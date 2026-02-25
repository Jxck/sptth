# AGENTS.md

Project-wide guidance for AI coding agents and contributors.

## Scope

- This file applies to the entire repository.
- If a deeper directory adds its own `AGENTS.md`, the deeper file overrides this one for that subtree.

## Primary Goals

- Keep local-dev reliability high (`DNS + TLS proxy + local CA`).
- Prefer safe defaults for local security.
- Preserve maintainable structure over quick one-off fixes.

## Required Workflow

1. Format and checks before commit:
   - `cargo run -p xtask -- fmt-check`
   - `cargo test -q`
   - `cargo check --workspace`
2. Do not bypass failing checks unless explicitly requested by the maintainer.
3. Keep changes scoped to one concern per commit.

## Formatting

- Use project tooling only:
  - `cargo run -p xtask -- fmt`
  - `cargo run -p xtask -- fmt-check`
- `pre-commit` is check-only. It blocks commits when formatting is dirty.

## Commit Message Policy

- Follow `/Users/jxck/develop/sptth/docs/commit-message.md`.
- Required structure:
  1. short subject
  2. 3-bullet compressed summary
  3. sections per summary item with:
     - Background
     - Review
     - Decision
     - Impact

## Change Design Rules

- Prefer small, explicit changes.
- Avoid broad refactors unless requested.
- Keep behavior changes documented in `README.md` when user-facing.
- Add/adjust tests for changed logic when practical.

## Security-Sensitive Areas

- `src/ca.rs` (private keys, trust install)
- `src/dns.rs` and `src/server.rs` (resolver forwarding, concurrency)
- `src/proxy.rs` and `src/tls.rs` (routing, body limits, TLS behavior)

When touching these paths:

- explain the threat/risk being addressed,
- keep failure modes explicit in logs,
- avoid weakening existing safeguards.

## Communication Style

- Keep responses concise and factual.
- If uncertain, state assumptions clearly before changing behavior.
