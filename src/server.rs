use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use tokio::{net::UdpSocket, signal, task};

use crate::{
    config::{AppConfig, DnsConfig, DomainAddrs},
    dns, logging, proxy,
};

pub async fn run(config: AppConfig) -> Result<()> {
    let dns_fut = run_dns(config.dns, config.records);
    let proxy_fut = proxy::run(config.proxies);

    tokio::select! {
        res = async {
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
