use std::{path::Path, process::Command};

use anyhow::{Context, Result, bail};

use crate::logging;

pub fn install_ca_cert(ca_cert_path: &Path) -> Result<()> {
    // Root store is required so browsers and HTTP clients trust issued certs.
    let output = Command::new("certutil")
        .arg("-addstore")
        .arg("-f")
        .arg("Root")
        .arg(ca_cert_path)
        .output()
        .context("failed to execute certutil")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "failed to install CA certificate using certutil: {}",
            stderr
        );
    }

    logging::info("TLS", "trust install target=windows:certutil status=ok");
    Ok(())
}
