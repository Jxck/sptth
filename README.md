# sptth

<p align="center">
  <img src="docs/assets/logo-horizontal.svg" alt="sptth logo" width="900" />
</p>

All-in-one tool for local server development with **valid HTTPS**.

## Concept

Do you usually develop with URLs like `http://localhost:3000`?
`localhost` is a special hostname and behaves differently from production, so things that work locally can fail after deployment on `https://${your-production-domain}`.

Previously, setting up local real-domain HTTPS usually required three separate privileged steps:

1. Map domain -> `127.0.0.1` (`/etc/hosts` or DNS settings)
2. Create/install a local CA and issue certs (for example with `mkcert`)
3. Bind a server on port `443`

That meant dealing with `sudo` across multiple tools and steps.

`sptth` now bundles everything needed to develop locally on real domains such as `https://${your-production-domain}`.

- Local DNS
- Local CA
- Local reverse proxy

You can start all of them with one `sudo` command and test locally in conditions closer to production.

`sptth` consolidates this into one flow:

```sh
sudo sptth config.toml
```

One command, one `sudo`, one process for all local development.

## Caution

> [!CAUTION]
> `sptth` is in **Early Preview** during `v0.x`.
> It changes some of your local settings.
> Use it at your own risk.

## Security Notes

`sptth` is designed for **local development only** and should not be exposed to untrusted networks.

- The reverse proxy enforces a **10 MiB request body limit** (request and response) to prevent memory exhaustion.
- Concurrent proxy connections are capped at **512** and DNS handler tasks at **256**.
- Upstream HTTP requests have a **5 s connect timeout** and **30 s overall timeout**; TLS handshakes time out after **10 s**.
- DNS forwarding **validates the source address** of upstream responses to reject spoofed packets.
- Private key files (CA and leaf) are created with **mode 0600** (owner-only read/write) on Unix.
- CA and certificate directories are created with **mode 0700** (owner-only access) on Unix.

## Quick Start

Change your OS DNS server to `127.0.0.1`.

### macOS

`System Settings > Network > Wi-Fi Details > DNS > DNS Servers`

![DNS Settings on macOS](docs/assets/dns-settings-on-macos.png)

### Windows

Open `Control Panel > Network and Internet > Network Connections`, open your active network adapter, then set DNS server `127.0.0.1` in `Internet Protocol Version 4 (TCP/IPv4)`.

### Ubuntu

Set DNS manually to `127.0.0.1` in `Settings > Network`, or configure `/etc/resolv.conf` / `systemd-resolved` to use `127.0.0.1`.

## Install

See `docs/release.md` for release and artifact verification policy.

### Verify Release Artifacts

After downloading release files, verify them before execution.

```sh
VERSION=v0.0.2
FILE=sptth-${VERSION}-aarch64-apple-darwin.tar.gz
```

1. Verify checksum:

```sh
sha256sum -c SHA256SUMS --ignore-missing
```

2. Verify keyless signature:

```sh
cosign verify-blob \
  --certificate "${FILE}.crt" \
  --signature "${FILE}.sig" \
  --certificate-identity-regexp "https://github.com/jxck/sptth/.github/workflows/release.yml@refs/tags/${VERSION}" \
  --certificate-oidc-issuer "https://token.actions.githubusercontent.com" \
  "${FILE}"
```

3. Verify provenance attestation:

```sh
gh attestation verify "${FILE}" --repo jxck/sptth
```

## Development Setup

After cloning, run setup once to install local development tools into `.tools/`.

```sh
cargo run -p xtask -- setup
```

Formatting commands:

```sh
cargo run -p xtask -- fmt
cargo run -p xtask -- fmt-check
```

Use `fmt` locally to apply formatting. Use `fmt-check` in CI to fail on unformatted files.

`xtask setup` installs pinned versions of:

- `dprint` binary
- Markdown plugin
- TOML plugin
- JSON plugin

Artifacts are stored in `.tools/` and are not committed to git.

`xtask setup` also enables repository hooks via `core.hooksPath=.githooks`.
The `pre-commit` hook runs format checks for `*.rs`, `*.json`, `*.md`, and `*.toml`, and blocks commit when formatting is not clean.

## Run

```sh
sudo sptth config.toml
```

It requires installing a local CA into the OS trust store. Please accept it (at your own risk).

## `config.toml`

```toml
[dns]
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53", "8.8.8.8:53"]

[[record]]
domain = "example.com"
A = ["127.0.0.1"]
AAAA = ["::1"]

[[record]]
domain = "example.net"
A = ["127.0.0.1"]
AAAA = ["::1"]

[tls]
ca_dir = "~/.config/sptth/ca"
cert_dir = "~/.config/sptth/certs"

[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"

[[proxy]]
domain = "example.net"
listen = "127.0.0.1:443"
upstream = "localhost:4000"
```

In this example, access to these domains is proxied to local servers.

- `https://example.com` -> `http://localhost:3000`
- `https://example.net` -> `http://localhost:4000`

This works because the local DNS resolves `example.com` and `example.net` to `127.0.0.1`, and the proxy forwards HTTPS on port 443 to the upstream HTTP servers.

The browser should not show certificate errors, because valid TLS certificates are issued by a local CA trusted by the OS.

## Current Platform Support

- macOS: supported
- Linux: supported (`update-ca-certificates` or `update-ca-trust`)
- Windows: supported (`certutil`)

## Options

### Top-level options

- `log_level`
  - Application-wide log level (default: `info`, values: `error` | `info` | `debug`)

### `[dns]` section

```toml
[dns]
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53", "8.8.8.8:53"]

[[record]]
domain = "example.com"
A = ["127.0.0.1"]
AAAA = ["::1"]
```

This starts a DNS server on `127.0.0.1:53`.
Queries for domains in `[[record]]` return local `A`/`AAAA` values; all others are forwarded to upstream DNS.

- `[dns].listen`
  - DNS listen IP/port
- `[dns].upstream`
  - Upstream DNS servers
- `[dns].ttl_seconds`
  - Record TTL (default: `1`)

### `[[record]]` section

```toml
[[record]]
domain = "example.com"
A = ["127.0.0.1"]
AAAA = ["::1"]
```

Each `[[record]]` entry defines local DNS overrides for a domain.

- `[[record]].domain`
  - Domain name to override
- `[[record]].A`
  - IPv4 addresses (optional)
- `[[record]].AAAA`
  - IPv6 addresses (optional)

### `[tls]` section

```toml
[tls]
ca_dir = "~/.config/sptth/ca"
cert_dir = "~/.config/sptth/certs"
```

The `[tls]` section is required in the config file.

`sptth` creates a local CA and installs it into the OS trust store.
On first run, the generated CA certificate and key are saved in `ca_dir`.
If files already exist in `ca_dir`, they are reused and trust-store modification is skipped.

At startup, certificates for all domains in `[[proxy]]` are generated and signed by the local CA.
Files are stored in `cert_dir` and used by the proxy to provide valid TLS in browsers.

- `[tls].valid_days`
  - Server certificate validity in days (default: `90`)
- `[tls].renew_before_days`
  - Renewal threshold before expiry in days (default: `30`)
- `[tls].ca_dir`
  - Root CA certificate/key directory (default: `~/.config/sptth/ca`)
- `[tls].cert_dir`
  - Domain certificate/key directory (default: `~/.config/sptth/certs`)

(When running with `sudo`, default `tls.ca_dir` / `tls.cert_dir` are resolved under the `SUDO_USER` home directory.)

> [!WARNING]
>
> - Startup fails if trust-store installation fails.
> - Linux requires either `update-ca-certificates` or `update-ca-trust`.
> - Windows requires `certutil`.

### `[[proxy]]` section

```toml
[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"
```

This forwards `https://example.com` to `http://localhost:3000`.

- `[[proxy]].domain`
  - Routing domain (must be unique)
- `[[proxy]].listen`
  - Proxy listen address; all entries must share the same value in the current version
- `[[proxy]].upstream`
  - Upstream in `host:port` format

## Build and Run from Source

```sh
git clone https://github.com/jxck/sptth.git
cd sptth
cargo run -p xtask -- setup
cargo build --release
sudo ./target/release/sptth config.toml
```

## Verification

Start a local upstream server first:

```sh
echo "<title>hello</title>" > index.html
python3 -m http.server 3000
```

Verify DNS resolution (with OS DNS set to `127.0.0.1`):

```sh
dig example.com A

;; ANSWER SECTION:
example.com.        1    IN    A    127.0.0.1
```

Verify HTTPS proxy:

```sh
curl https://example.com/
<title>hello</title>
```

## ACK

All source code was written in Rust by Codex.
