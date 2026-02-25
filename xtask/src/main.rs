use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

const DPRINT_VERSION: &str = "0.51.1";

const MARKDOWN_PLUGIN_URL: &str = "https://plugins.dprint.dev/markdown-0.21.1.wasm";
const MARKDOWN_PLUGIN_FILE: &str = "markdown-0.21.1.wasm";
const MARKDOWN_PLUGIN_SHA256: &str =
    "064467750514c9ce5192b375582d762ec64cb3ba99673413fa86645d50406279";

const TOML_PLUGIN_URL: &str = "https://plugins.dprint.dev/toml-0.7.0.wasm";
const TOML_PLUGIN_FILE: &str = "toml-0.7.0.wasm";
const TOML_PLUGIN_SHA256: &str = "0126c8112691542d30b52a639076ecc83e07bace877638cee7c6915fd36b8629";

const JSON_PLUGIN_URL: &str = "https://plugins.dprint.dev/json-0.21.0.wasm";
const JSON_PLUGIN_FILE: &str = "json-0.21.0.wasm";
const JSON_PLUGIN_SHA256: &str = "188a08916eeccf2414e06c8b51d8f44d3695f055a0d63cef39eace0a11e247bc";

struct PluginSpec {
    url: &'static str,
    file: &'static str,
    sha256: &'static str,
}

const PLUGINS: &[PluginSpec] = &[
    PluginSpec {
        url: MARKDOWN_PLUGIN_URL,
        file: MARKDOWN_PLUGIN_FILE,
        sha256: MARKDOWN_PLUGIN_SHA256,
    },
    PluginSpec {
        url: TOML_PLUGIN_URL,
        file: TOML_PLUGIN_FILE,
        sha256: TOML_PLUGIN_SHA256,
    },
    PluginSpec {
        url: JSON_PLUGIN_URL,
        file: JSON_PLUGIN_FILE,
        sha256: JSON_PLUGIN_SHA256,
    },
];

fn main() -> Result<()> {
    let root = workspace_root();
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("setup") => cmd_setup(&root),
        Some("fmt") => {
            ensure_tools(&root)?;
            run_dprint(&root, &["fmt"])?;
            run_rustfmt(&root, false)
        }
        Some("fmt-check") => {
            ensure_tools(&root)?;
            run_dprint(&root, &["check"])?;
            run_rustfmt(&root, true)
        }
        _ => {
            eprintln!("usage: cargo run -p xtask -- <setup|fmt|fmt-check>");
            bail!("unknown xtask command")
        }
    }
}

fn cmd_setup(root: &Path) -> Result<()> {
    ensure_tools(root)?;
    ensure_git_hooks(root)?;
    Ok(())
}

fn ensure_tools(root: &Path) -> Result<()> {
    fs::create_dir_all(tools_root(&root).join("dprint").join("plugins"))
        .context("failed to create plugin directory")?;
    fs::create_dir_all(tools_root(&root).join("dprint").join("cache"))
        .context("failed to create dprint cache directory")?;

    ensure_dprint_binary(&root)?;
    let client = reqwest::blocking::Client::builder()
        .user_agent("sptth-xtask")
        .build()
        .context("failed to build HTTP client")?;

    for plugin in PLUGINS {
        ensure_plugin(&root, &client, plugin)?;
    }

    Ok(())
}

fn run_dprint(root: &Path, args: &[&str]) -> Result<()> {
    let dprint = dprint_bin(&root);
    if !dprint.exists() {
        bail!("dprint binary is missing: run `cargo run -p xtask -- setup`");
    }

    let cache_dir = tools_root(&root).join("dprint").join("cache");
    fs::create_dir_all(&cache_dir).context("failed to create dprint cache directory")?;

    let status = Command::new(&dprint)
        .args(args)
        .current_dir(&root)
        .env("DPRINT_CACHE_DIR", cache_dir)
        .status()
        .with_context(|| format!("failed to execute {}", dprint.display()))?;

    if !status.success() {
        bail!("dprint exited with status {}", status);
    }

    Ok(())
}

fn ensure_dprint_binary(root: &Path) -> Result<()> {
    let bin = dprint_bin(root);

    if bin.exists() {
        let version = dprint_version(&bin)?;
        if version == DPRINT_VERSION {
            return Ok(());
        }
    }

    let tools_root = tools_root(root);
    fs::create_dir_all(&tools_root).context("failed to create tools directory")?;

    let status = Command::new("cargo")
        .arg("install")
        .arg("dprint")
        .arg("--version")
        .arg(DPRINT_VERSION)
        .arg("--locked")
        .arg("--root")
        .arg(&tools_root)
        .arg("--force")
        .current_dir(root)
        .status()
        .context("failed to run `cargo install dprint`")?;

    if !status.success() {
        bail!("cargo install dprint failed with status {}", status);
    }

    Ok(())
}

fn ensure_plugin(root: &Path, client: &reqwest::blocking::Client, spec: &PluginSpec) -> Result<()> {
    let path = tools_root(root)
        .join("dprint")
        .join("plugins")
        .join(spec.file);

    if path.exists() {
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let digest = sha256_hex(&bytes);
        if digest == spec.sha256 {
            return Ok(());
        }
    }

    let response = client
        .get(spec.url)
        .send()
        .with_context(|| format!("failed to download {}", spec.url))?
        .error_for_status()
        .with_context(|| format!("unexpected HTTP status for {}", spec.url))?;

    let body = response
        .bytes()
        .with_context(|| format!("failed to read response body for {}", spec.url))?;
    let digest = sha256_hex(body.as_ref());
    if digest != spec.sha256 {
        bail!(
            "sha256 mismatch for {}: expected {}, got {}",
            spec.file,
            spec.sha256,
            digest
        );
    }

    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &body).with_context(|| format!("failed to write {}", tmp.display()))?;
    // Windows does not allow rename over an existing destination.
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("failed to remove {}", path.display()))?;
    }
    fs::rename(&tmp, &path)
        .with_context(|| format!("failed to place plugin at {}", path.display()))?;

    Ok(())
}

fn ensure_git_hooks(root: &Path) -> Result<()> {
    let hooks_dir = root.join(".githooks");
    fs::create_dir_all(&hooks_dir).context("failed to create .githooks directory")?;

    let hook_path = hooks_dir.join("pre-commit");
    if !hook_path.exists() {
        bail!(
            "missing git hook file: {}. restore repository files and re-run setup",
            hook_path.display()
        );
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_path)
            .with_context(|| format!("failed to read {}", hook_path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)
            .with_context(|| format!("failed to set executable bit on {}", hook_path.display()))?;
    }

    let status = Command::new("git")
        .arg("config")
        .arg("--local")
        .arg("core.hooksPath")
        .arg(".githooks")
        .current_dir(root)
        .status()
        .context("failed to configure git hooksPath")?;
    if !status.success() {
        bail!("git config core.hooksPath failed with status {}", status);
    }

    Ok(())
}

fn run_rustfmt(root: &Path, check: bool) -> Result<()> {
    let mut command = Command::new("cargo");
    command.arg("fmt").arg("--all");
    if check {
        command.arg("--").arg("--check");
    }
    let status = command
        .current_dir(root)
        .status()
        .context("failed to run cargo fmt")?;
    if !status.success() {
        bail!("cargo fmt failed with status {}", status);
    }
    Ok(())
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn dprint_version(path: &Path) -> Result<String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .with_context(|| format!("failed to run {}", path.display()))?;
    if !output.status.success() {
        bail!(
            "{} --version failed with status {}",
            path.display(),
            output.status
        );
    }
    let stdout = String::from_utf8(output.stdout).context("dprint output was not UTF-8")?;
    let version = stdout
        .split_whitespace()
        .nth(1)
        .context("failed to parse dprint version")?;
    Ok(version.to_string())
}

fn dprint_bin(root: &Path) -> PathBuf {
    let exe = if cfg!(windows) {
        "dprint.exe"
    } else {
        "dprint"
    };
    tools_root(root).join("bin").join(exe)
}

fn tools_root(root: &Path) -> PathBuf {
    root.join(".tools")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be in workspace root/xtask")
        .to_path_buf()
}
