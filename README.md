# sptth

Step 1: DNS only.

This implementation does only name resolution:

- returns configured local addresses for configured domains
- forwards all other queries to upstream DNS servers

## Config (TOML)

Create `config.toml` (or pass a custom path):

```toml
listen = "127.0.0.1:53"
upstream = ["1.1.1.1:53", "8.8.8.8:53"]
ttl_seconds = 30
log_level = "info"

[[record]]
domain = "jxck.io"
A = ["127.0.0.1"]
AAAA = ["::1"]

[[record]]
domain = "api.jxck.io"
A = ["127.0.0.2"]
AAAA = ["::1"]
```

Each `[[record]]` can define `A` and/or `AAAA`.

`log_level` values:

- `error`
- `info` (default)
- `debug`

## Build

```sh
cargo build
```

## Run

```sh
sudo env RUSTUP_TOOLCHAIN=1.92.0-aarch64-apple-darwin cargo run -- config.toml
```

If omitted, `config.toml` in the current directory is used.

## Verify

```sh
dig @127.0.0.1 jxck.io A
```

```sh
dig @127.0.0.1 jxck.io AAAA
```

```sh
dig @127.0.0.1 example.com A
```

Expected:

- configured domains resolve to addresses from `[[record]]` (`A` / `AAAA`)
- other domains are resolved by upstream DNS
