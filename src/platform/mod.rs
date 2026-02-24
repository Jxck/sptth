use std::path::Path;

use anyhow::Result;

#[cfg(target_os = "macos")]
mod macos;

pub fn install_ca_cert(ca_cert_path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        return macos::install_ca_cert(ca_cert_path);
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = ca_cert_path;
        anyhow::bail!("unsupported platform: trust-store auto-install currently supports only macOS");
    }
}
