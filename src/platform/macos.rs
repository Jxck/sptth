use std::{path::Path, process::Command};

use anyhow::{Context, Result, bail};

use crate::logging;

pub fn install_ca_cert(ca_cert_path: &Path) -> Result<()> {
    // Use System keychain so all local user processes can trust sptth-issued certs.
    let output = Command::new("security")
        .arg("add-trusted-cert")
        .arg("-d")
        .arg("-r")
        .arg("trustRoot")
        .arg("-k")
        .arg("/Library/Keychains/System.keychain")
        .arg(ca_cert_path)
        .output()
        .context("failed to execute security command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "failed to install CA certificate to trust store: {}",
            stderr
        );
    }

    logging::info("TLS", "trust install target=system status=ok");
    Ok(())
}
