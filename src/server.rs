use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result, bail};
use tokio::{net::UdpSocket, signal, task};

use crate::{
    ca,
    config::{AppConfig, DnsConfig, DomainAddrs},
    dns, logging, platform, proxy, tls,
};

pub async fn run(config: AppConfig) -> Result<()> {
    if !config.tls.enabled {
        bail!("tls.enabled must be true in this phase");
    }

    // Boot order matters: certificates must exist before the TLS listener starts.
    let assets = ca::provision_certificates(&config.tls, &config.proxies)?;
    if assets.ca_created {
        // Install trust only on first creation to avoid rewriting OS trust state
        // on every run.
        platform::install_ca_cert(&assets.ca_cert_path)?;
    } else {
        logging::info("TLS", "ca exists, trust install skipped");
    }
    let tls_config = tls::build_server_config(&assets.certs)?;

    let dns_fut = run_dns(config.dns, config.records);
    let proxy_fut = proxy::run(config.proxies, Arc::clone(&tls_config));

    tokio::select! {
        res = async {
            // DNS and proxy are a single service unit; if either fails, fail fast.
            tokio::try_join!(dns_fut, proxy_fut)?;
            Ok::<(), anyhow::Error>(())
        } => res,
        _ = signal::ctrl_c() => {
            logging::info("SERVER", "received Ctrl+C, shutting down");
            Ok(())
        }
    }
}

async fn run_dns(config: DnsConfig, records: HashMap<String, DomainAddrs>) -> Result<()> {
    let socket = Arc::new(
        UdpSocket::bind(config.listen)
            .await
            .with_context(|| format!("failed to bind dns socket {}", config.listen))?,
    );
    let records = Arc::new(records);
    let upstream = Arc::new(config.upstream);
    let ttl = config.ttl_seconds;

    logging::info("DNS", &format!("dns server listening on {}", config.listen));

    let mut buf = vec![0_u8; 4096];
    loop {
        let (size, peer) = socket
            .recv_from(&mut buf)
            .await
            .context("dns recv_from failed")?;
        let req_packet = buf[..size].to_vec();
        logging::debug("DNS", &format!("recv {} bytes from {}", size, peer));

        let socket = Arc::clone(&socket);
        let records = Arc::clone(&records);
        let upstream = Arc::clone(&upstream);

        // Each request is handled in its own task to keep UDP receive loop responsive.
        task::spawn(async move {
            match dns::handle_dns_packet(
                &req_packet,
                peer,
                records.as_ref(),
                upstream.as_ref(),
                ttl,
            )
            .await
            {
                Ok(resp) => match socket.send_to(&resp, peer).await {
                    Ok(sent) => logging::debug("DNS", &format!("sent {} bytes to {}", sent, peer)),
                    Err(err) => logging::error(
                        "DNS",
                        &format!("failed to send response to {}: {}", peer, err),
                    ),
                },
                Err(err) => logging::error(
                    "DNS",
                    &format!("request handling failed for {}: {}", peer, err),
                ),
            }
        });
    }
}
