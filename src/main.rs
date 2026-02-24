mod config;
mod dns;
mod logging;
mod server;

use std::{env, path::PathBuf};

use anyhow::{Result, bail};
use config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = parse_cli_args()?;
    let config = AppConfig::from_file(&config_path)?;

    logging::init(config.log_level);

    println!("sptth dns started");
    println!("  config  : {}", config_path.display());
    println!("  listen  : {}", config.listen);
    println!("  records : {}", config.joined_domains());
    println!("  upstream: {}", config.joined_upstream());
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
