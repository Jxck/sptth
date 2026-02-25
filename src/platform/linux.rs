use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

use crate::logging;

pub fn install_ca_cert(ca_cert_path: &Path) -> Result<()> {
    // Distros differ; support both Debian-style and RHEL-style trust commands.
    if has_command("update-ca-certificates") {
        install_with_update_ca_certificates(ca_cert_path)?;
        logging::info(
            "TLS",
            "trust install target=linux:update-ca-certificates status=ok",
        );
        return Ok(());
    }

    if has_command("update-ca-trust") {
        install_with_update_ca_trust(ca_cert_path)?;
        logging::info(
            "TLS",
            "trust install target=linux:update-ca-trust status=ok",
        );
        return Ok(());
    }

    bail!(
        "failed to install CA certificate on Linux: neither update-ca-certificates nor update-ca-trust is available"
    )
}

fn install_with_update_ca_certificates(ca_cert_path: &Path) -> Result<()> {
    let target = PathBuf::from("/usr/local/share/ca-certificates/sptth-rootCA.crt");
    fs::copy(ca_cert_path, &target)
        .with_context(|| format!("failed to copy CA certificate to {}", target.display()))?;

    let output = Command::new("update-ca-certificates")
        .output()
        .context("failed to execute update-ca-certificates")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("update-ca-certificates failed: {}", stderr);
    }

    Ok(())
}

fn install_with_update_ca_trust(ca_cert_path: &Path) -> Result<()> {
    let target = PathBuf::from("/etc/pki/ca-trust/source/anchors/sptth-rootCA.crt");
    fs::copy(ca_cert_path, &target)
        .with_context(|| format!("failed to copy CA certificate to {}", target.display()))?;

    let output = Command::new("update-ca-trust")
        .arg("extract")
        .output()
        .context("failed to execute update-ca-trust extract")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("update-ca-trust extract failed: {}", stderr);
    }

    Ok(())
}

/// Search PATH for an executable without invoking a shell, so the name is
/// never interpreted as a shell meta-character sequence.
fn has_command(name: &str) -> bool {
    let path_var = env::var_os("PATH").unwrap_or_default();
    env::split_paths(&path_var).any(|dir| {
        let candidate = dir.join(name);
        candidate.is_file()
    })
}
