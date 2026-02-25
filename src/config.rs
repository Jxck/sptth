use std::{
    collections::{HashMap, HashSet},
    fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use http::uri::Authority;
use serde::Deserialize;

use crate::logging::LogLevel;

#[derive(Debug, Deserialize)]
struct RawConfig {
    dns: RawDns,
    tls: RawTls,
    record: Vec<RawRecord>,
    proxy: Vec<RawProxy>,
    log_level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawDns {
    listen: String,
    upstream: Vec<String>,
    ttl_seconds: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawTls {
    enabled: Option<bool>,
    ca_dir: Option<String>,
    cert_dir: Option<String>,
    ca_common_name: Option<String>,
    valid_days: Option<u32>,
    renew_before_days: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct RawRecord {
    domain: String,
    #[serde(rename = "A")]
    a: Option<Vec<String>>,
    #[serde(rename = "AAAA")]
    aaaa: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RawProxy {
    domain: String,
    listen: String,
    upstream: String,
}

#[derive(Debug, Clone)]
pub struct DomainAddrs {
    pub ipv4: Vec<Ipv4Addr>,
    pub ipv6: Vec<Ipv6Addr>,
}

#[derive(Debug)]
pub struct DnsConfig {
    pub listen: SocketAddr,
    pub upstream: Vec<SocketAddr>,
    pub ttl_seconds: u32,
}

#[derive(Debug)]
pub struct TlsConfig {
    pub enabled: bool,
    pub ca_dir: PathBuf,
    pub cert_dir: PathBuf,
    pub ca_common_name: String,
    pub valid_days: u32,
    pub renew_before_days: u32,
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub domain: String,
    pub listen: SocketAddr,
    pub upstream_host_port: String,
}

impl ProxyConfig {
    // Keep config syntax compact ("host:port"), but always expose an HTTP base URL
    // to the proxy layer.
    pub fn base_url(&self) -> String {
        format!("http://{}", self.upstream_host_port)
    }
}

#[derive(Debug)]
pub struct AppConfig {
    pub dns: DnsConfig,
    pub tls: TlsConfig,
    pub records: HashMap<String, DomainAddrs>,
    pub proxies: Vec<ProxyConfig>,
    pub log_level: LogLevel,
}

impl AppConfig {
    pub fn from_file(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        Self::from_toml_str(&raw, &path.display().to_string())
    }

    fn from_toml_str(raw: &str, source: &str) -> Result<Self> {
        let parsed: RawConfig =
            toml::from_str(raw).with_context(|| format!("failed to parse TOML: {}", source))?;

        let dns_listen = parsed
            .dns
            .listen
            .parse::<SocketAddr>()
            .with_context(|| format!("invalid dns.listen address: {}", parsed.dns.listen))?;

        if parsed.dns.upstream.is_empty() {
            bail!("dns.upstream must have at least one dns server");
        }

        let mut dns_upstream = Vec::with_capacity(parsed.dns.upstream.len());
        for u in &parsed.dns.upstream {
            dns_upstream.push(
                u.parse::<SocketAddr>()
                    .with_context(|| format!("invalid dns.upstream address: {u}"))?,
            );
        }

        let tls_enabled = parsed.tls.enabled.unwrap_or(true);
        let tls_valid_days = parsed.tls.valid_days.unwrap_or(90);
        let tls_renew_before_days = parsed.tls.renew_before_days.unwrap_or(30);
        if tls_valid_days == 0 {
            bail!("tls.valid_days must be greater than 0");
        }
        if tls_renew_before_days >= tls_valid_days {
            bail!("tls.renew_before_days must be smaller than tls.valid_days");
        }

        let ca_common_name = parsed
            .tls
            .ca_common_name
            .unwrap_or_else(|| "sptth local ca".to_string());
        if ca_common_name.trim().is_empty() {
            bail!("tls.ca_common_name must not be empty");
        }

        // Use a stable per-user location when paths are omitted, so running from
        // different working directories does not create different CAs.
        let default_base = default_state_base_dir();
        let ca_dir = parsed
            .tls
            .ca_dir
            .as_deref()
            .map(expand_tilde)
            .unwrap_or_else(|| default_base.join("ca"));
        let cert_dir = parsed
            .tls
            .cert_dir
            .as_deref()
            .map(expand_tilde)
            .unwrap_or_else(|| default_base.join("certs"));

        if parsed.record.is_empty() {
            bail!("at least one [[record]] is required");
        }

        let mut records = HashMap::<String, DomainAddrs>::new();
        for row in &parsed.record {
            let domain = normalize_domain(&row.domain);
            if domain.is_empty() {
                bail!("record.domain contains empty value");
            }

            let a_values = row.a.as_deref().unwrap_or(&[]);
            let aaaa_values = row.aaaa.as_deref().unwrap_or(&[]);
            if a_values.is_empty() && aaaa_values.is_empty() {
                bail!("record requires A and/or AAAA values: {}", domain);
            }

            let mut ipv4 = Vec::<Ipv4Addr>::new();
            let mut ipv6 = Vec::<Ipv6Addr>::new();

            for value in a_values {
                let ip = value.parse::<IpAddr>().with_context(|| {
                    format!("invalid A address in record {}: {}", domain, value)
                })?;
                match ip {
                    IpAddr::V4(v4) => ipv4.push(v4),
                    IpAddr::V6(_) => bail!("A must be IPv4 in record {}: {}", domain, value),
                }
            }

            for value in aaaa_values {
                let ip = value.parse::<IpAddr>().with_context(|| {
                    format!("invalid AAAA address in record {}: {}", domain, value)
                })?;
                match ip {
                    IpAddr::V6(v6) => ipv6.push(v6),
                    IpAddr::V4(_) => bail!("AAAA must be IPv6 in record {}: {}", domain, value),
                }
            }

            let prev = records.insert(domain.clone(), DomainAddrs { ipv4, ipv6 });
            if prev.is_some() {
                bail!("duplicate record.domain: {}", domain);
            }
        }

        if parsed.proxy.is_empty() {
            bail!("at least one [[proxy]] is required");
        }

        let mut proxies = Vec::<ProxyConfig>::with_capacity(parsed.proxy.len());
        let mut domain_seen = HashSet::<String>::new();
        let mut listen_seen = None::<SocketAddr>;

        for row in &parsed.proxy {
            let domain = normalize_domain(&row.domain);
            if domain.is_empty() {
                bail!("proxy.domain contains empty value");
            }
            if !domain_seen.insert(domain.clone()) {
                bail!("duplicate proxy.domain: {}", domain);
            }

            let listen = row
                .listen
                .parse::<SocketAddr>()
                .with_context(|| format!("invalid proxy.listen address: {}", row.listen))?;

            // MVP constraint: a single listener simplifies lifecycle and avoids
            // accidental partial routing differences across proxy entries.
            match listen_seen {
                None => listen_seen = Some(listen),
                Some(v) if v == listen => {}
                Some(_) => {
                    bail!("all proxy.listen values must be identical in the current version")
                }
            }

            if row.upstream.contains("://") {
                bail!(
                    "proxy.upstream must be host:port (no scheme): {}",
                    row.upstream
                );
            }

            validate_upstream_host_port(&row.upstream)?;

            proxies.push(ProxyConfig {
                domain,
                listen,
                upstream_host_port: row.upstream.clone(),
            });
        }

        Ok(Self {
            dns: DnsConfig {
                listen: dns_listen,
                upstream: dns_upstream,
                ttl_seconds: parsed.dns.ttl_seconds.unwrap_or(1),
            },
            tls: TlsConfig {
                enabled: tls_enabled,
                ca_dir,
                cert_dir,
                ca_common_name,
                valid_days: tls_valid_days,
                renew_before_days: tls_renew_before_days,
            },
            records,
            proxies,
            log_level: match parsed.log_level.as_deref() {
                None => LogLevel::Info,
                Some(v) => LogLevel::parse(v)?,
            },
        })
    }

    pub fn joined_domains(&self) -> String {
        let mut v = self.records.keys().cloned().collect::<Vec<_>>();
        v.sort();
        v.join(", ")
    }

    pub fn joined_proxies(&self) -> String {
        self.proxies
            .iter()
            .map(|p| format!("{}:{}->{}", p.domain, p.listen.port(), p.upstream_host_port))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl DnsConfig {
    pub fn joined_upstream(&self) -> String {
        self.upstream
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub fn normalize_domain(input: &str) -> String {
    input.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn validate_upstream_host_port(input: &str) -> Result<()> {
    let authority: Authority = input
        .parse()
        .with_context(|| format!("invalid proxy.upstream host:port: {}", input))?;

    if authority.host().is_empty() {
        bail!("proxy.upstream host is empty: {}", input);
    }

    if authority.port_u16().is_none() {
        bail!("proxy.upstream must include port: {}", input);
    }

    Ok(())
}

fn default_state_base_dir() -> PathBuf {
    // When run with sudo, we still want state files to belong to the invoking user
    // rather than root's HOME to keep behavior predictable across restarts.
    if let Ok(sudo_user) = std::env::var("SUDO_USER") {
        if !sudo_user.trim().is_empty() && sudo_user != "root" {
            if let Ok(Some(home)) = homedir::home(&sudo_user) {
                return home.join(".config").join("sptth");
            }
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home).join(".config").join("sptth");
        }
    }

    PathBuf::from(".sptth")
}

fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    }
    if let Some(suffix) = input.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(suffix);
        }
    }
    PathBuf::from(input)
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, expand_tilde, normalize_domain};

    fn base_toml(proxy_block: &str) -> String {
        format!(
            r#"
log_level = "info"

[dns]
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53"]
ttl_seconds = 1

[tls]
enabled = true
ca_dir = "/tmp/sptth-ca"
cert_dir = "/tmp/sptth-certs"
ca_common_name = "sptth local ca"
valid_days = 90
renew_before_days = 30

[[record]]
domain = "example.com"
A = ["127.0.0.1"]

{proxy_block}
"#
        )
    }

    #[test]
    fn reject_proxy_upstream_with_scheme() {
        let toml = base_toml(
            r#"
[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "http://localhost:3000"
"#,
        );

        let err = AppConfig::from_toml_str(&toml, "test")
            .expect_err("config should fail for scheme in upstream");
        assert!(err.to_string().contains("no scheme"));
    }

    #[test]
    fn reject_proxy_upstream_without_port() {
        let toml = base_toml(
            r#"
[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost"
"#,
        );

        let err =
            AppConfig::from_toml_str(&toml, "test").expect_err("config should fail without port");
        assert!(err.to_string().contains("must include port"));
    }

    #[test]
    fn reject_duplicate_proxy_domain() {
        let toml = base_toml(
            r#"
[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"

[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3001"
"#,
        );

        let err = AppConfig::from_toml_str(&toml, "test")
            .expect_err("config should fail for duplicate domain");
        assert!(err.to_string().contains("duplicate proxy.domain"));
    }

    #[test]
    fn reject_invalid_proxy_listen() {
        let toml = base_toml(
            r#"
[[proxy]]
domain = "example.com"
listen = "127.0.0.1"
upstream = "localhost:3000"
"#,
        );

        let err = AppConfig::from_toml_str(&toml, "test")
            .expect_err("config should fail for invalid listen");
        assert!(err.to_string().contains("invalid proxy.listen"));
    }

    #[test]
    fn reject_invalid_tls_renew_window() {
        let toml = base_toml(
            r#"
[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"
"#,
        )
        .replace("renew_before_days = 30", "renew_before_days = 90");

        let err = AppConfig::from_toml_str(&toml, "test")
            .expect_err("config should fail for invalid renew window");
        assert!(err.to_string().contains("renew_before_days"));
    }

    #[test]
    fn accept_valid_config() {
        let toml = base_toml(
            r#"
[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"
"#,
        );

        let config = AppConfig::from_toml_str(&toml, "test").expect("valid config should parse");
        assert_eq!(config.dns.listen.port(), 53);
        assert_eq!(config.proxies.len(), 1);
        assert_eq!(config.proxies[0].domain, "example.com");
    }

    #[test]
    fn normalize_domain_trims_and_lowercases() {
        assert_eq!(normalize_domain("Example.COM"), "example.com");
        assert_eq!(normalize_domain("example.com."), "example.com");
        assert_eq!(normalize_domain("  Example.COM.  "), "example.com");
        assert_eq!(normalize_domain(""), "");
    }

    #[test]
    fn expand_tilde_with_home() {
        let result = expand_tilde("/absolute/path");
        assert_eq!(result.to_str().unwrap(), "/absolute/path");

        let result = expand_tilde("relative/path");
        assert_eq!(result.to_str().unwrap(), "relative/path");
    }

    #[test]
    fn reject_duplicate_record_domain() {
        let toml = r#"
log_level = "info"

[dns]
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53"]

[tls]
enabled = true
ca_dir = "/tmp/sptth-ca"
cert_dir = "/tmp/sptth-certs"

[[record]]
domain = "example.com"
A = ["127.0.0.1"]

[[record]]
domain = "example.com"
A = ["127.0.0.2"]

[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"
"#;

        let err = AppConfig::from_toml_str(toml, "test")
            .expect_err("config should fail for duplicate record domain");
        assert!(err.to_string().contains("duplicate record.domain"));
    }

    #[test]
    fn reject_a_record_with_ipv6() {
        let toml = base_toml(
            r#"
[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"
"#,
        )
        .replace(r#"A = ["127.0.0.1"]"#, r#"A = ["::1"]"#);

        let err = AppConfig::from_toml_str(&toml, "test")
            .expect_err("config should fail for IPv6 in A record");
        assert!(err.to_string().contains("A must be IPv4"));
    }

    #[test]
    fn reject_aaaa_record_with_ipv4() {
        let toml = r#"
log_level = "info"

[dns]
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53"]

[tls]
enabled = true
ca_dir = "/tmp/sptth-ca"
cert_dir = "/tmp/sptth-certs"

[[record]]
domain = "example.com"
AAAA = ["127.0.0.1"]

[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"
"#;

        let err = AppConfig::from_toml_str(toml, "test")
            .expect_err("config should fail for IPv4 in AAAA record");
        assert!(err.to_string().contains("AAAA must be IPv6"));
    }

    #[test]
    fn reject_different_proxy_listen() {
        let toml = base_toml(
            r#"
[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"

[[proxy]]
domain = "example.net"
listen = "127.0.0.1:8443"
upstream = "localhost:4000"
"#,
        );

        let err = AppConfig::from_toml_str(&toml, "test")
            .expect_err("config should fail for different listen addresses");
        assert!(err.to_string().contains("must be identical"));
    }
}
