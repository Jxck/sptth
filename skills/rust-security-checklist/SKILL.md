---
name: rust-security-checklist
description: Use before merging security-relevant Rust changes. Catches common local-service security regressions.
---

# rust-security-checklist Skill

Use this skill before merging security-relevant Rust changes.

## Goal

Catch common local-service security regressions early.

## Checklist

1. Input and size limits

- Are request/response/body sizes bounded?
- Are parsing paths resistant to oversized input?

2. Network trust boundaries

- Are remote response sources validated (address/port where required)?
- Are timeouts and retry limits explicit?

3. Resource exhaustion

- Is task/thread concurrency bounded?
- Are unbounded queues/spawns avoided?

4. File and key safety

- Are private keys/certs written with restrictive permissions where supported?
- Are secret file paths stable and predictable under `sudo`?

5. Command execution safety

- Avoid `sh -c` with interpolated strings.
- Prefer direct `Command` APIs and explicit args.

6. Logging hygiene

- No secrets in logs.
- Errors are specific but not sensitive.

## Required Validation

```sh
cargo run -p xtask -- fmt-check
cargo test -q
cargo check --workspace
```

## Reporting Template

- Threat addressed:
- Change summary:
- Residual risk:
- Follow-up (if any):
