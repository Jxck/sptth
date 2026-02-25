mod ca;
mod config;
mod dns;
mod logging;
mod platform;
mod proxy;
mod server;
mod tls;

use std::{env, path::PathBuf};

use anyhow::{Result, bail};
use config::AppConfig;
use rustls::crypto::ring::default_provider;

#[tokio::main]
async fn main() -> Result<()> {
    // rustls 0.23 requires an explicit process-level crypto provider.
    // We install ring here once at startup to avoid runtime panics later.
    let _ = default_provider().install_default();

    let config_path = parse_cli_args()?;
    let config = AppConfig::from_file(&config_path)?;

    logging::init(config.log_level);

    println!("sptth started");
    println!("  config   : {}", config_path.display());
    println!("  dns      : {}", config.dns.listen);
    println!("  records  : {}", config.joined_domains());
    println!("  upstream : {}", config.dns.joined_upstream());
    println!("  proxies  : {}", config.joined_proxies());
    println!(
        "  tls      : {}",
        if config.tls.enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("  log_level: {}", config.log_level.as_str());
    println!("press Ctrl+C to stop");

    server::run(config).await
}

fn parse_cli_args() -> Result<PathBuf> {
    let mut args = env::args();
    let bin = args.next().unwrap_or_else(|| "sptth".to_string());
    let config_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config.toml"));

    if args.next().is_some() {
        bail!("usage: {} [config.toml]", bin);
    }

    Ok(config_path)
}
