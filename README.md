# sptth

`sptth` runs a local DNS server and an HTTPS reverse proxy in one process.

## Preview Status (v0)

- `sptth` is in preview during `v0.x`.
> [!CAUTION]
> Please use it at your own risk.

Current capabilities:

1. DNS override for configured domains (`[[record]]`)
2. HTTPS reverse proxy on `127.0.0.1:443` (`[[proxy]]`)
3. Local CA creation and OS trust-store installation (automatic at startup)

## Current Platform Support

- macOS: supported
- Linux: supported (`update-ca-certificates` or `update-ca-trust`)
- Windows: supported (`certutil`)

## Config (TOML)

```toml
[dns]
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53", "8.8.8.8:53"]

[[record]]
domain = "example.com"
A = ["127.0.0.1"]

[tls]
ca_dir = "~/.config/sptth/ca"
cert_dir = "~/.config/sptth/certs"

[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"
```

## Options

- `[dns].ttl_seconds`: `30`
- `[dns].log_level`: `info`
- `[tls].enabled`: `true`
- `[tls].ca_common_name`: `sptth local ca`
- `[tls].valid_days`: `90`
- `[tls].renew_before_days`: `30`
- `[tls].ca_dir`: `~/.config/sptth/ca`
- `[tls].cert_dir`: `~/.config/sptth/certs`
- when started via `sudo`, default `tls.ca_dir` / `tls.cert_dir` use `SUDO_USER` home.
- trust-store installation runs when CA is created. If CA already exists, it is skipped.
- `[[proxy]].upstream` must be `host:port` only.
- `[[proxy]].domain` must be unique.
- all `[[proxy]].listen` values must be identical in this phase.

## Notes

- startup fails if CA trust installation fails.
- Linux requires either `update-ca-certificates` or `update-ca-trust`.
- Windows requires `certutil`.

## Run

```sh
sudo env RUSTUP_TOOLCHAIN=1.92.0-aarch64-apple-darwin cargo run -- config.toml
```

## Verify

Start local upstream app:

```sh
python3 -m http.server 3000
```

1. Point your OS DNS to `127.0.0.1` and verify resolution:

```sh
dig example.com A
```

2. Verify HTTPS proxy with normal name resolution:

```sh
curl https://example.com/
```

Expected behavior:

- `example.com` resolves to `127.0.0.1` (from `[[record]]`)
- `https://example.com` routes to `localhost:3000`
- unknown host returns `502 Bad Gateway`
