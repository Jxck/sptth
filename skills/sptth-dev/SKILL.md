# sptth-dev Skill

Use this skill when making code/documentation changes in this repository.

## Intent

Provide a consistent, low-risk workflow for implementing and validating changes in `sptth`.

## Steps

1. Read context quickly:
   - `README.md`
   - `docs/commit-message.md`
   - relevant files under `src/`
2. Implement minimal, focused changes.
3. Run required checks:
   - `cargo run -p xtask -- fmt-check`
   - `cargo test -q`
   - `cargo check --workspace`
4. Update docs when behavior/options changed.
5. Prepare commit message using the project template.

## Commands

```sh
cargo run -p xtask -- setup
cargo run -p xtask -- fmt
cargo run -p xtask -- fmt-check
cargo test -q
cargo check --workspace
```

## Guardrails

- Do not commit unrelated diffs.
- Do not disable security checks silently.
- Keep DNS/Proxy/TLS changes covered by at least one test update when practical.
- Preserve local-only assumptions in docs (not for public edge deployment).
