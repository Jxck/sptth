use std::path::Path;

use anyhow::Result;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub fn install_ca_cert(ca_cert_path: &Path) -> Result<()> {
    // Dispatch by target OS to keep platform-specific command logic isolated.
    #[cfg(target_os = "macos")]
    {
        return macos::install_ca_cert(ca_cert_path);
    }

    #[cfg(target_os = "linux")]
    {
        return linux::install_ca_cert(ca_cert_path);
    }

    #[cfg(target_os = "windows")]
    {
        return windows::install_ca_cert(ca_cert_path);
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = ca_cert_path;
        anyhow::bail!(
            "unsupported platform: trust-store auto-install supports only macOS, Linux, and Windows"
        );
    }
}
