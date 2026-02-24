# sptth

Step 1: DNS only.

This implementation does only name resolution:

- returns `A=127.0.0.1` for configured domains
- forwards all other queries to upstream DNS servers

## Config (TOML)

Create `config.toml` (or pass a custom path):

```toml
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53", "8.8.8.8:53"]
domains = ["jxck.io", "api.jxck.io"]
ttl_seconds = 30
```

## Build

```sh
cargo build
```

## Run

```sh
sudo env RUSTUP_TOOLCHAIN=1.92.0-aarch64-apple-darwin cargo run -- config.toml --log-level info
```

If omitted, `config.toml` in the current directory is used.

## Log Levels

- `--log-level error`: errors only
- `--log-level info`: errors + local resolve events (default)
- `--log-level debug`: include query receive/forward details

## Verify

```sh
dig @127.0.0.1 jxck.io A
```

```sh
dig @127.0.0.1 example.com A
```

Expected:

- `jxck.io` resolves to `127.0.0.1`
- `example.com` is resolved by upstream DNS
