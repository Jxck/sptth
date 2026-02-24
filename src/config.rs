use std::{
    collections::HashMap,
    fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::Path,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::logging::LogLevel;

#[derive(Debug, Deserialize)]
struct RawConfig {
    dns: RawDns,
    record: Vec<RawRecord>,
}

#[derive(Debug, Deserialize)]
struct RawDns {
    listen: String,
    upstream: Vec<String>,
    ttl_seconds: Option<u32>,
    log_level: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawRecord {
    domain: String,
    #[serde(rename = "A")]
    a: Option<Vec<String>>,
    #[serde(rename = "AAAA")]
    aaaa: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct DomainAddrs {
    pub ipv4: Vec<Ipv4Addr>,
    pub ipv6: Vec<Ipv6Addr>,
}

pub struct AppConfig {
    pub listen: SocketAddr,
    pub upstream: Vec<SocketAddr>,
    pub records: HashMap<String, DomainAddrs>,
    pub ttl_seconds: u32,
    pub log_level: LogLevel,
}

impl AppConfig {
    pub fn from_file(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let parsed: RawConfig = toml::from_str(&raw)
            .with_context(|| format!("failed to parse TOML: {}", path.display()))?;

        let listen = parsed
            .dns
            .listen
            .parse::<SocketAddr>()
            .with_context(|| format!("invalid listen address: {}", parsed.dns.listen))?;

        if parsed.dns.upstream.is_empty() {
            bail!("upstream must have at least one dns server");
        }

        let mut upstream = Vec::with_capacity(parsed.dns.upstream.len());
        for u in &parsed.dns.upstream {
            upstream.push(
                u.parse::<SocketAddr>()
                    .with_context(|| format!("invalid upstream address: {u}"))?,
            );
        }

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

        Ok(Self {
            listen,
            upstream,
            records,
            ttl_seconds: parsed.dns.ttl_seconds.unwrap_or(30),
            log_level: match parsed.dns.log_level.as_deref() {
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
