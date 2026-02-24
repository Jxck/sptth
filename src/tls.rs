use std::{collections::HashMap, fs::File, io::BufReader, path::Path, sync::Arc};

use anyhow::{Context, Result, anyhow, bail};
use rustls::{
    ServerConfig,
    crypto::ring::sign::any_supported_type,
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
};

use crate::{ca::IssuedCert, config::normalize_domain, logging};

pub fn build_server_config(certs: &HashMap<String, IssuedCert>) -> Result<Arc<ServerConfig>> {
    if certs.is_empty() {
        bail!("no certificate available for proxy domains");
    }

    let mut map = HashMap::<String, Arc<CertifiedKey>>::new();
    let mut default = None::<Arc<CertifiedKey>>;

    for (domain, files) in certs {
        let certified = Arc::new(load_certified_key(&files.cert_path, &files.key_path)?);
        if default.is_none() {
            default = Some(Arc::clone(&certified));
        }
        map.insert(normalize_domain(domain), certified);
    }

    let resolver = DomainCertResolver {
        certs: map,
        default: default.context("missing default certificate")?,
    };

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(resolver));

    Ok(Arc::new(config))
}

fn load_certified_key(cert_path: &Path, key_path: &Path) -> Result<CertifiedKey> {
    let mut cert_reader = BufReader::new(
        File::open(cert_path)
            .with_context(|| format!("failed to open certificate: {}", cert_path.display()))?,
    );
    let cert_chain = rustls_pemfile::certs(&mut cert_reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to parse certificate: {}", cert_path.display()))?;
    if cert_chain.is_empty() {
        bail!("certificate chain is empty: {}", cert_path.display());
    }

    let mut key_reader = BufReader::new(
        File::open(key_path)
            .with_context(|| format!("failed to open key: {}", key_path.display()))?,
    );
    let key_der = rustls_pemfile::private_key(&mut key_reader)
        .with_context(|| format!("failed to parse key: {}", key_path.display()))?
        .ok_or_else(|| anyhow!("private key not found in {}", key_path.display()))?;

    let signing_key = any_supported_type(&key_der)
        .map_err(|e| anyhow!("unsupported private key {}: {}", key_path.display(), e))?;

    Ok(CertifiedKey::new(cert_chain, signing_key))
}

#[derive(Debug)]
struct DomainCertResolver {
    certs: HashMap<String, Arc<CertifiedKey>>,
    default: Arc<CertifiedKey>,
}

impl ResolvesServerCert for DomainCertResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let sni = client_hello.server_name().unwrap_or_default();
        let domain = normalize_domain(sni);

        if let Some(cert) = self.certs.get(&domain) {
            return Some(Arc::clone(cert));
        }

        if !domain.is_empty() {
            logging::debug(
                "TLS",
                &format!("SNI domain not found, fallback to default cert: {}", domain),
            );
        }

        Some(Arc::clone(&self.default))
    }
}
