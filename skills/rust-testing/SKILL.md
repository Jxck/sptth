# rust-testing Skill

Use this skill when adding or modifying tests in this repository.

## Goal

Ensure behavior changes are covered with focused tests and fast feedback loops.

## Test Strategy

1. Unit tests for pure logic and validation helpers.
2. Integration-like checks for command-level behavior when practical.
3. Regression tests for security-sensitive fixes.

## What to Test

- Config parsing/validation (`src/config.rs`)
- DNS response routing/forwarding decisions (`src/dns.rs`)
- Proxy routing/header/body constraints (`src/proxy.rs`)
- Key/cert file handling constraints (`src/ca.rs`)

## Rules

- Keep test scopes tight (one behavior per test).
- Name tests by behavior, not implementation details.
- Prefer deterministic fixtures and fixed inputs.
- Avoid network dependency in unit tests.

## Commands

```sh
cargo test -q
cargo check --workspace
cargo run -p xtask -- fmt-check
```

## Done Criteria

- New behavior is covered.
- Existing tests still pass.
- Tests remain readable and maintainable.
