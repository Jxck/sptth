---
name: rust-error-handling
description: Use when changing Rust code paths that return errors or log failures. Ensures errors are actionable, consistent, and safe.
---

# rust-error-handling Skill

Use this skill when changing Rust code paths that return errors or log failures.

## Goal

Keep errors actionable, consistent, and easy to debug without leaking sensitive data.

## Principles

- Prefer `anyhow::Context` to add operation + resource context.
- Keep top-level error messages short and user-facing.
- Preserve root causes in chained errors.
- Avoid swallowing errors silently.
- Do not include secrets/keys/tokens in error text.

## Patterns

1. Add context at I/O and boundary calls:
   - file open/read/write
   - socket bind/send/recv
   - process execution
2. Use `bail!` for validation failures with explicit reason.
3. Keep logs structured and component-scoped (`DNS`, `PROXY`, `TLS`, etc.).
4. Return typed status at boundaries (for HTTP/DNS behavior) and retain internal error cause.

## Checklist

- Error path explains what failed and where.
- Caller receives enough info to act.
- Logs are useful at `info`/`debug`/`error` levels.
- No sensitive material is printed.
