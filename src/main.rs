use std::{
    collections::HashMap,
    env, fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use hickory_proto::{
    op::{Message, MessageType, Query, ResponseCode},
    rr::{
        Name, RData, Record, RecordType,
        rdata::{A, AAAA},
    },
};
use serde::Deserialize;
use tokio::{net::UdpSocket, signal, task, time};

#[derive(Debug, Deserialize)]
struct RawConfig {
    listen: String,
    upstream: Vec<String>,
    ttl_seconds: Option<u32>,
    record: Vec<RawRecord>,
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
struct DomainAddrs {
    ipv4: Vec<Ipv4Addr>,
    ipv6: Vec<Ipv6Addr>,
}

struct Config {
    listen: SocketAddr,
    upstream: Vec<SocketAddr>,
    records: HashMap<String, DomainAddrs>,
    ttl_seconds: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum LogLevel {
    Error,
    Info,
    Debug,
}

static LOG_LEVEL: OnceLock<LogLevel> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    let (config_path, log_level) = parse_cli_args()?;
    let _ = LOG_LEVEL.set(log_level);
    let config = load_config(&config_path)?;

    println!("sptth dns started");
    println!("  config  : {}", config_path.display());
    println!("  listen  : {}", config.listen);
    println!("  records : {}", join_domains(config.records.keys()));
    println!("  upstream: {}", join_upstream(&config.upstream));
    println!("  log_level: {}", log_level.as_str());
    println!("press Ctrl+C to stop");
    log_info("dns server loop starting");

    run_dns_server(config).await
}

fn parse_cli_args() -> Result<(PathBuf, LogLevel)> {
    let mut args = env::args();
    let bin = args.next().unwrap_or_else(|| "sptth".to_string());
    let mut config_path = PathBuf::from("config.toml");
    let mut config_set = false;
    let mut log_level = LogLevel::Info;

    let argv = args.collect::<Vec<_>>();
    let mut i = 0_usize;
    while i < argv.len() {
        match argv[i].as_str() {
            "--help" | "-h" => {
                print_usage(&bin);
                std::process::exit(0);
            }
            "--log-level" => {
                i += 1;
                let value = argv
                    .get(i)
                    .ok_or_else(|| anyhow!("--log-level requires a value"))?;
                log_level = LogLevel::parse(value)?;
            }
            arg if arg.starts_with('-') => bail!("unknown option: {}", arg),
            path => {
                if config_set {
                    bail!(
                        "usage: {} [config.toml] [--log-level <error|info|debug>]",
                        bin
                    );
                }
                config_path = PathBuf::from(path);
                config_set = true;
            }
        }
        i += 1;
    }

    Ok((config_path, log_level))
}

fn load_config(path: &PathBuf) -> Result<Config> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    let parsed: RawConfig = toml::from_str(&raw)
        .with_context(|| format!("failed to parse TOML: {}", path.display()))?;

    let listen = parsed
        .listen
        .parse::<SocketAddr>()
        .with_context(|| format!("invalid listen address: {}", parsed.listen))?;

    if parsed.upstream.is_empty() {
        bail!("upstream must have at least one dns server");
    }

    let mut upstream = Vec::with_capacity(parsed.upstream.len());
    for u in &parsed.upstream {
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
            let ip = value
                .parse::<IpAddr>()
                .with_context(|| format!("invalid A address in record {}: {}", domain, value))?;
            match ip {
                IpAddr::V4(v4) => ipv4.push(v4),
                IpAddr::V6(_) => bail!("A must be IPv4 in record {}: {}", domain, value),
            }
        }

        for value in aaaa_values {
            let ip = value
                .parse::<IpAddr>()
                .with_context(|| format!("invalid AAAA address in record {}: {}", domain, value))?;
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

    Ok(Config {
        listen,
        upstream,
        records,
        ttl_seconds: parsed.ttl_seconds.unwrap_or(30),
    })
}

async fn run_dns_server(config: Config) -> Result<()> {
    let socket = Arc::new(
        UdpSocket::bind(config.listen)
            .await
            .with_context(|| format!("failed to bind dns socket {}", config.listen))?,
    );
    let records = Arc::new(config.records);
    let upstream = Arc::new(config.upstream);
    let ttl = config.ttl_seconds;

    let mut buf = vec![0_u8; 4096];
    loop {
        tokio::select! {
            _ = signal::ctrl_c() => break,
            recv = socket.recv_from(&mut buf) => {
                let (size, peer) = recv.context("dns recv_from failed")?;
                let req_packet = buf[..size].to_vec();
                log_debug(&format!("recv {} bytes from {}", size, peer));

                let socket = Arc::clone(&socket);
                let records = Arc::clone(&records);
                let upstream = Arc::clone(&upstream);

                task::spawn(async move {
                    match handle_dns_packet(&req_packet, peer, records.as_ref(), upstream.as_ref(), ttl).await {
                        Ok(resp) => {
                            match socket.send_to(&resp, peer).await {
                                Ok(sent) => log_debug(&format!("sent {} bytes to {}", sent, peer)),
                                Err(err) => log_error(&format!("failed to send response to {}: {}", peer, err)),
                            }
                        }
                        Err(err) => log_error(&format!("request handling failed for {}: {}", peer, err)),
                    }
                });
            }
        }
    }

    Ok(())
}

async fn handle_dns_packet(
    packet: &[u8],
    peer: SocketAddr,
    records: &HashMap<String, DomainAddrs>,
    upstream: &[SocketAddr],
    ttl: u32,
) -> Result<Vec<u8>> {
    let req = Message::from_vec(packet).context("invalid dns request packet")?;
    let query = req
        .queries()
        .first()
        .ok_or_else(|| anyhow!("dns query is empty"))?
        .clone();

    let qname = normalize_domain(&query.name().to_ascii());
    let qtype = query.query_type();
    log_debug(&format!(
        "query id={} from={} name={} type={}",
        req.id(),
        peer,
        qname,
        qtype
    ));

    if let Some(addrs) = records.get(&qname) {
        if qtype == RecordType::A || qtype == RecordType::AAAA || qtype.is_any() {
            log_info(&format!(
                "local resolve id={} name={} type={}",
                req.id(),
                qname,
                qtype
            ));
            return local_response(&req, &query, &qname, qtype, ttl, addrs);
        }
    }

    log_debug(&format!(
        "forward id={} name={} to upstream",
        req.id(),
        qname
    ));
    forward_dns_packet(packet, req.id(), &qname, qtype, upstream).await
}

fn local_response(
    req: &Message,
    query: &Query,
    qname: &str,
    qtype: RecordType,
    ttl: u32,
    addrs: &DomainAddrs,
) -> Result<Vec<u8>> {
    let mut resp = Message::new();
    resp.set_id(req.id());
    resp.set_message_type(MessageType::Response);
    resp.set_op_code(req.op_code());
    resp.set_recursion_desired(req.recursion_desired());
    resp.set_recursion_available(true);
    resp.set_authoritative(true);
    resp.set_response_code(ResponseCode::NoError);
    resp.add_query(query.clone());

    let name = Name::from_ascii(qname).with_context(|| format!("invalid query name: {qname}"))?;

    match qtype {
        RecordType::A => {
            for v4 in &addrs.ipv4 {
                resp.add_answer(Record::from_rdata(name.clone(), ttl, RData::A(A(*v4))));
            }
        }
        RecordType::AAAA => {
            for v6 in &addrs.ipv6 {
                resp.add_answer(Record::from_rdata(
                    name.clone(),
                    ttl,
                    RData::AAAA(AAAA(*v6)),
                ));
            }
        }
        RecordType::ANY => {
            for v4 in &addrs.ipv4 {
                resp.add_answer(Record::from_rdata(name.clone(), ttl, RData::A(A(*v4))));
            }
            for v6 in &addrs.ipv6 {
                resp.add_answer(Record::from_rdata(
                    name.clone(),
                    ttl,
                    RData::AAAA(AAAA(*v6)),
                ));
            }
        }
        _ => {}
    }

    resp.to_vec().context("failed to encode dns response")
}

async fn forward_dns_packet(
    packet: &[u8],
    query_id: u16,
    qname: &str,
    qtype: RecordType,
    upstream: &[SocketAddr],
) -> Result<Vec<u8>> {
    for server in upstream {
        log_debug(&format!(
            "forward try id={} name={} type={} upstream={}",
            query_id, qname, qtype, server
        ));
        let resolver = UdpSocket::bind("0.0.0.0:0")
            .await
            .context("failed to bind temporary dns socket")?;
        resolver
            .send_to(packet, server)
            .await
            .with_context(|| format!("failed to forward dns query to {server}"))?;

        let mut buf = vec![0_u8; 4096];
        let recv = time::timeout(time::Duration::from_secs(2), resolver.recv_from(&mut buf)).await;
        match recv {
            Ok(Ok((n, from))) => {
                log_debug(&format!(
                    "forward success id={} upstream={} bytes={}",
                    query_id, from, n
                ));
                return Ok(buf[..n].to_vec());
            }
            Ok(Err(err)) => {
                log_error(&format!(
                    "forward recv error id={} upstream={} err={}",
                    query_id, server, err
                ));
            }
            Err(_) => {
                log_error(&format!(
                    "forward timeout id={} upstream={}",
                    query_id, server
                ));
            }
        }
    }

    bail!("all upstream dns servers failed")
}

fn normalize_domain(input: &str) -> String {
    input.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn join_domains<'a, I>(domains: I) -> String
where
    I: Iterator<Item = &'a String>,
{
    let mut v = domains.cloned().collect::<Vec<_>>();
    v.sort();
    v.join(", ")
}

fn join_upstream(upstream: &[SocketAddr]) -> String {
    upstream
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn log_info(msg: &str) {
    if should_log(LogLevel::Info) {
        eprintln!("[{}] INFO  {}", unix_ts(), msg);
    }
}

fn log_debug(msg: &str) {
    if should_log(LogLevel::Debug) {
        eprintln!("[{}] DEBUG {}", unix_ts(), msg);
    }
}

fn log_error(msg: &str) {
    if should_log(LogLevel::Error) {
        eprintln!("[{}] ERROR {}", unix_ts(), msg);
    }
}

fn should_log(level: LogLevel) -> bool {
    let configured = LOG_LEVEL.get().copied().unwrap_or(LogLevel::Info);
    level <= configured
}

fn unix_ts() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(dur) => dur.as_secs(),
        Err(_) => 0,
    }
}

impl LogLevel {
    fn parse(v: &str) -> Result<Self> {
        match v {
            "error" => Ok(Self::Error),
            "info" => Ok(Self::Info),
            "debug" => Ok(Self::Debug),
            _ => bail!(
                "invalid --log-level value: {} (expected: error|info|debug)",
                v
            ),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Info => "info",
            Self::Debug => "debug",
        }
    }
}

fn print_usage(bin: &str) {
    println!(
        "usage: {} [config.toml] [--log-level <error|info|debug>]",
        bin
    );
}
