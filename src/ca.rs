use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration as StdDuration, SystemTime},
};

use anyhow::{Context, Result};
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    KeyPair, KeyUsagePurpose,
};
use time::{Duration, OffsetDateTime};

use crate::{
    config::{ProxyConfig, TlsConfig},
    logging,
};

#[derive(Debug, Clone)]
pub struct IssuedCert {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Debug)]
pub struct TlsAssets {
    pub ca_cert_path: PathBuf,
    pub ca_created: bool,
    pub certs: HashMap<String, IssuedCert>,
}

pub fn provision_certificates(tls: &TlsConfig, proxies: &[ProxyConfig]) -> Result<TlsAssets> {
    fs::create_dir_all(&tls.ca_dir)
        .with_context(|| format!("failed to create ca_dir: {}", tls.ca_dir.display()))?;
    fs::create_dir_all(&tls.cert_dir)
        .with_context(|| format!("failed to create cert_dir: {}", tls.cert_dir.display()))?;

    let signer = load_or_create_ca(tls)?;
    let mut certs = HashMap::<String, IssuedCert>::new();

    for proxy in proxies {
        let domain = proxy.domain.clone();
        let cert_path = tls.cert_dir.join(format!("{}.pem", domain));
        let key_path = tls.cert_dir.join(format!("{}.key", domain));

        // Reissue by age threshold instead of parsing X.509 on every run.
        // Why: this keeps startup logic simple and fast for MVP.
        let reissue = should_reissue(&cert_path, tls.valid_days, tls.renew_before_days);
        if reissue {
            issue_domain_cert(
                &domain,
                &cert_path,
                &key_path,
                tls.valid_days,
                &signer.ca_cert,
                &signer.ca_key,
            )?;
            logging::info("TLS", &format!("cert issued domain={}", domain));
        } else {
            logging::info("TLS", &format!("cert reused domain={}", domain));
        }

        certs.insert(
            domain,
            IssuedCert {
                cert_path,
                key_path,
            },
        );
    }

    Ok(TlsAssets {
        ca_cert_path: signer.ca_cert_path,
        ca_created: signer.created,
        certs,
    })
}

struct CaSigner {
    ca_cert: Certificate,
    ca_key: KeyPair,
    ca_cert_path: PathBuf,
    created: bool,
}

fn load_or_create_ca(tls: &TlsConfig) -> Result<CaSigner> {
    let ca_cert_path = tls.ca_dir.join("rootCA.pem");
    let ca_key_path = tls.ca_dir.join("rootCA-key.pem");

    let key_exists = ca_key_path.exists();
    let cert_exists = ca_cert_path.exists();

    let (ca_key, created) = if key_exists {
        let pem = fs::read_to_string(&ca_key_path)
            .with_context(|| format!("failed to read CA key: {}", ca_key_path.display()))?;
        let key = KeyPair::from_pem(&pem)
            .with_context(|| format!("failed to parse CA key: {}", ca_key_path.display()))?;
        // If key exists but cert is missing, recover by regenerating cert from key.
        (key, !cert_exists)
    } else {
        let key = KeyPair::generate().context("failed to generate CA key")?;
        fs::write(&ca_key_path, key.serialize_pem())
            .with_context(|| format!("failed to write CA key: {}", ca_key_path.display()))?;
        (key, true)
    };

    let mut params = CertificateParams::default();
    params
        .distinguished_name
        .push(DnType::CommonName, tls.ca_common_name.clone());
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::CrlSign,
    ];

    let ca_cert = params
        .self_signed(&ca_key)
        .context("failed to create CA certificate")?;

    if created {
        fs::write(&ca_cert_path, ca_cert.pem()).with_context(|| {
            format!("failed to write CA certificate: {}", ca_cert_path.display())
        })?;
    }

    if created {
        logging::info(
            "TLS",
            &format!("ca created path={}", ca_cert_path.display()),
        );
    } else {
        logging::info("TLS", &format!("ca reused path={}", ca_cert_path.display()));
    }

    Ok(CaSigner {
        ca_cert,
        ca_key,
        ca_cert_path,
        created,
    })
}

fn issue_domain_cert(
    domain: &str,
    cert_path: &Path,
    key_path: &Path,
    valid_days: u32,
    ca_cert: &Certificate,
    ca_key: &KeyPair,
) -> Result<()> {
    let leaf_key = KeyPair::generate().context("failed to generate leaf key")?;

    let mut params = CertificateParams::new(vec![domain.to_string()])
        .context("failed to initialize certificate parameters")?;
    params.distinguished_name.push(DnType::CommonName, domain);
    params.is_ca = IsCa::NoCa;
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];

    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(i64::from(valid_days));

    let cert = params
        .signed_by(&leaf_key, ca_cert, ca_key)
        .with_context(|| format!("failed to issue certificate for domain: {}", domain))?;

    fs::write(cert_path, cert.pem())
        .with_context(|| format!("failed to write certificate: {}", cert_path.display()))?;
    fs::write(key_path, leaf_key.serialize_pem())
        .with_context(|| format!("failed to write key: {}", key_path.display()))?;

    Ok(())
}

fn should_reissue(cert_path: &Path, valid_days: u32, renew_before_days: u32) -> bool {
    if !cert_path.exists() {
        return true;
    }

    let renew_after_days = valid_days.saturating_sub(renew_before_days);
    if renew_after_days == 0 {
        return true;
    }

    let metadata = match fs::metadata(cert_path) {
        Ok(v) => v,
        Err(_) => return true,
    };
    let modified = match metadata.modified() {
        Ok(v) => v,
        Err(_) => return true,
    };

    let age = match SystemTime::now().duration_since(modified) {
        Ok(v) => v,
        Err(_) => return true,
    };

    age >= StdDuration::from_secs(u64::from(renew_after_days) * 24 * 60 * 60)
}
