# sptth

`sptth` runs a local DNS server and an HTTPS reverse proxy in one process.

Current capabilities:

1. DNS override for configured domains (`[[record]]`)
2. HTTPS reverse proxy on `127.0.0.1:443` (`[[proxy]]`)
3. Local CA creation and macOS trust-store installation (automatic at startup)

## Current Platform Support

- macOS: supported
- Linux / Windows: planned (not implemented yet)

## Config (TOML)

```toml
[dns]
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53", "8.8.8.8:53"]
ttl_seconds = 1
log_level = "info"

[tls]
enabled = true
ca_common_name = "sptth local ca"
valid_days = 90
renew_before_days = 30

[[record]]
domain = "example.com"
A = ["127.0.0.1"]

[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"
```

## Notes

- `[[proxy]].upstream` must be `host:port` only.
- `[[proxy]].domain` must be unique.
- all `[[proxy]].listen` values must be identical in this phase.
- startup fails if CA trust installation fails.
- `tls.ca_dir` and `tls.cert_dir` are optional.
- default paths are `~/.config/sptth/ca` and `~/.config/sptth/certs`.
- when started via `sudo`, the default uses `SUDO_USER` home.
- trust-store installation runs when CA is created. If CA already exists, it is skipped.

## Run

```sh
sudo env RUSTUP_TOOLCHAIN=1.92.0-aarch64-apple-darwin cargo run -- config.toml
```

## Verify

Start local upstream app:

```sh
python3 -m http.server 3000
```

DNS:

```sh
dig @127.0.0.1 example.com A
```

HTTPS proxy:

```sh
curl --resolve example.com:443:127.0.0.1 https://example.com/
```

Expected behavior:

- `https://example.com` routes to `localhost:3000`
- unknown host returns `502 Bad Gateway`
