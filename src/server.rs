use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::{net::UdpSocket, signal, task};

use crate::{config::AppConfig, dns, logging};

pub async fn run(config: AppConfig) -> Result<()> {
    let socket = Arc::new(
        UdpSocket::bind(config.listen)
            .await
            .with_context(|| format!("failed to bind dns socket {}", config.listen))?,
    );
    let records = Arc::new(config.records);
    let upstream = Arc::new(config.upstream);
    let ttl = config.ttl_seconds;

    logging::info("DNS", "dns server loop starting");

    let mut buf = vec![0_u8; 4096];
    loop {
        tokio::select! {
            _ = signal::ctrl_c() => break,
            recv = socket.recv_from(&mut buf) => {
                let (size, peer) = recv.context("dns recv_from failed")?;
                let req_packet = buf[..size].to_vec();
                logging::debug("DNS", &format!("recv {} bytes from {}", size, peer));

                let socket = Arc::clone(&socket);
                let records = Arc::clone(&records);
                let upstream = Arc::clone(&upstream);

                task::spawn(async move {
                    match dns::handle_dns_packet(&req_packet, peer, records.as_ref(), upstream.as_ref(), ttl).await {
                        Ok(resp) => {
                            match socket.send_to(&resp, peer).await {
                                Ok(sent) => logging::debug("DNS", &format!("sent {} bytes to {}", sent, peer)),
                                Err(err) => logging::error("DNS", &format!("failed to send response to {}: {}", peer, err)),
                            }
                        }
                        Err(err) => logging::error("DNS", &format!("request handling failed for {}: {}", peer, err)),
                    }
                });
            }
        }
    }

    Ok(())
}
