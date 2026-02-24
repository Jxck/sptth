# sptth

`sptth` currently provides:

1. DNS server with local overrides (`[[record]]`)
2. Plain HTTP reverse proxy on port `443` (`[[proxy]]`)

This phase does not support TLS yet. Access is plain HTTP, for example:
`http://example.com:443`.

## Config (TOML)

Create `config.toml` (or pass a custom path):

```toml
[dns]
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53", "8.8.8.8:53"]
ttl_seconds = 1
log_level = "info"

[[record]]
domain = "example.com"
A = ["127.0.0.1"]

[[record]]
domain = "example.net"
A = ["127.0.0.2"]

[[proxy]]
domain = "example.com"
listen = "127.0.0.1:443"
upstream = "localhost:3000"

[[proxy]]
domain = "example.net"
listen = "127.0.0.1:443"
upstream = "localhost:3001"
```

## Config Notes

- `[[record]]`: each entry can define `A` and/or `AAAA`.
- `[[proxy]].upstream`: must be `host:port` only (no `http://` or `https://`).
- In this MVP, all `[[proxy]].listen` values must be identical.
- `[[proxy]].domain` must be unique.

## Build

```sh
cargo build
```

## Run

```sh
sudo env RUSTUP_TOOLCHAIN=1.92.0-aarch64-apple-darwin cargo run -- config.toml
```

If omitted, `config.toml` in the current directory is used.

## Verify DNS

```sh
dig @127.0.0.1 example.com A
dig @127.0.0.1 example.net A
```

## Verify Proxy (Plain HTTP on 443)

Start an upstream app first:

```sh
python3 -m http.server 3000
```

Then call the proxy:

```sh
curl --resolve example.com:443:127.0.0.1 http://example.com:443/
```

Expected behavior:

- `Host: example.com` is routed to `localhost:3000`
- unknown host returns `502 Bad Gateway`
