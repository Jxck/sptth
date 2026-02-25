use std::{collections::HashMap, net::SocketAddr};

use anyhow::{Context, Result, anyhow, bail};
use hickory_proto::{
    op::{Message, MessageType, Query, ResponseCode},
    rr::{
        Name, RData, Record, RecordType,
        rdata::{A, AAAA},
    },
};

use crate::{
    config::{DomainAddrs, normalize_domain},
    logging,
};

pub async fn handle_dns_packet(
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
    logging::debug(
        "DNS",
        &format!(
            "query id={} from={} name={} type={}",
            req.id(),
            peer,
            qname,
            qtype
        ),
    );

    if let Some(addrs) = records.get(&qname) {
        // Local records have priority over upstream to guarantee deterministic
        // dev-domain routing.
        if qtype == RecordType::A || qtype == RecordType::AAAA || qtype.is_any() {
            return local_response(&req, &query, &qname, qtype, ttl, addrs);
        }
    }

    logging::debug(
        "DNS",
        &format!("forward id={} name={} to upstream", req.id(), qname),
    );
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
                logging::info("DNS", &format!("resolve name={} address={}", qname, v4));
                resp.add_answer(Record::from_rdata(name.clone(), ttl, RData::A(A(*v4))));
            }
        }
        RecordType::AAAA => {
            for v6 in &addrs.ipv6 {
                logging::info("DNS", &format!("resolve name={} address={}", qname, v6));
                resp.add_answer(Record::from_rdata(
                    name.clone(),
                    ttl,
                    RData::AAAA(AAAA(*v6)),
                ));
            }
        }
        RecordType::ANY => {
            for v4 in &addrs.ipv4 {
                logging::info("DNS", &format!("resolve name={} address={}", qname, v4));
                resp.add_answer(Record::from_rdata(name.clone(), ttl, RData::A(A(*v4))));
            }
            for v6 in &addrs.ipv6 {
                logging::info("DNS", &format!("resolve name={} address={}", qname, v6));
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

/// Check whether the response source exactly matches the expected upstream server.
fn is_valid_source(from: SocketAddr, expected: SocketAddr) -> bool {
    from == expected
}

async fn forward_dns_packet(
    packet: &[u8],
    query_id: u16,
    qname: &str,
    qtype: RecordType,
    upstream: &[SocketAddr],
) -> Result<Vec<u8>> {
    // Try upstream servers in order. This gives simple failover behavior
    // without adding extra retry state.
    for server in upstream {
        logging::debug(
            "DNS",
            &format!(
                "forward try id={} name={} type={} upstream={}",
                query_id, qname, qtype, server
            ),
        );

        let resolver = tokio::net::UdpSocket::bind("0.0.0.0:0")
            .await
            .context("failed to bind temporary dns socket")?;
        resolver
            .send_to(packet, server)
            .await
            .with_context(|| format!("failed to forward dns query to {server}"))?;

        let mut buf = vec![0_u8; 4096];
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(2);

        // Loop within the timeout window to discard spoofed packets from
        // unexpected sources and accept only the real upstream response.
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                logging::error(
                    "DNS",
                    &format!("forward timeout id={} upstream={}", query_id, server),
                );
                break;
            }

            let recv = tokio::time::timeout(remaining, resolver.recv_from(&mut buf)).await;

            match recv {
                Ok(Ok((n, from))) => {
                    if is_valid_source(from, *server) {
                        logging::debug(
                            "DNS",
                            &format!(
                                "forward success id={} upstream={} bytes={}",
                                query_id, from, n
                            ),
                        );
                        return Ok(buf[..n].to_vec());
                    }
                    // Discard packets from unexpected sources to prevent
                    // DNS spoofing via forged response injection.
                    logging::debug(
                        "DNS",
                        &format!(
                            "forward ignored id={} from={} expected={}",
                            query_id, from, server
                        ),
                    );
                }
                Ok(Err(err)) => {
                    logging::error(
                        "DNS",
                        &format!(
                            "forward recv error id={} upstream={} err={}",
                            query_id, server, err
                        ),
                    );
                    break;
                }
                Err(_) => {
                    logging::error(
                        "DNS",
                        &format!("forward timeout id={} upstream={}", query_id, server),
                    );
                    break;
                }
            }
        }
    }

    bail!("all upstream dns servers failed")
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::is_valid_source;

    #[test]
    fn valid_source_same_ip_and_port() {
        let server: SocketAddr = "1.1.1.1:53".parse().unwrap();
        let from: SocketAddr = "1.1.1.1:53".parse().unwrap();
        assert!(is_valid_source(from, server));
    }

    #[test]
    fn invalid_source_different_port() {
        let server: SocketAddr = "1.1.1.1:53".parse().unwrap();
        let from: SocketAddr = "1.1.1.1:5353".parse().unwrap();
        assert!(!is_valid_source(from, server));
    }

    #[test]
    fn invalid_source_different_ip() {
        let server: SocketAddr = "1.1.1.1:53".parse().unwrap();
        let from: SocketAddr = "9.9.9.9:53".parse().unwrap();
        assert!(!is_valid_source(from, server));
    }

    #[test]
    fn valid_source_ipv6() {
        let server: SocketAddr = "[2606:4700::1111]:53".parse().unwrap();
        let from: SocketAddr = "[2606:4700::1111]:53".parse().unwrap();
        assert!(is_valid_source(from, server));
    }
}
