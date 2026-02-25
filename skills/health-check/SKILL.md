---
name: health-check
description: Run a repository-wide maintenance pass before merge/release. Covers cleanup, formatting, tests, docs, security, and CI checks.
---

# health-check Skill

Use this skill to run a repository-wide maintenance pass before merge/release.

## Goal

Keep the repository clean, reproducible, and aligned across code, tests, CI, and documentation.

## Scope

- Detect and remove unnecessary code/definitions/dependencies where safe.
- Ensure repository formatting is fully clean.
- Review dependency freshness and apply safe updates.
- Verify tests/build checks pass.
- Ensure markdown/docs reflect current behavior.
- Run security and release-readiness checks.

## Required Procedure

1. Baseline and inventory

- Check current git status and changed files.
- Identify generated/untracked artifacts that should not remain.

2. Code and dependency cleanup

- Remove unused code/imports/definitions that are clearly dead.
- Review dependencies for obvious removals or outdated versions.
- Separate risky major upgrades from safe upgrades in reporting.

3. Formatting and consistency

- Run repository formatting commands.
- Re-run format checks to confirm zero drift.

4. Validation gates

- Run:
  - `cargo run -p xtask -- fmt-check`
  - `cargo test -q`
  - `cargo check --workspace`

5. Documentation sync

- Verify README/docs match current code behavior and defaults.
- Check command examples, config examples, option names, and stated limitations.

6. Security pass

- Apply checklist from `skills/rust-security-checklist/SKILL.md`:
  - input/resource limits
  - trust boundary validation
  - key/file handling
  - command execution safety
  - logging hygiene

7. Compatibility and impact pass

- Confirm config compatibility implications are identified.
- Confirm `config.example.toml` and docs are aligned with runtime behavior.

8. CI reproducibility pass

- Ensure local validation commands match CI expectations.
- Flag workflow drift (missing checks, stale commands).

9. Release readiness pass

- Detect blockers for release quality:
  - leftover generated artifacts
  - stale docs
  - unresolved TODOs that affect users
  - broken or outdated command examples

10. Language consistency pass

- Check for unintended Japanese text in repository files.
- Keep user-facing and project policy docs consistent with the repository language policy.
- Report intentional exceptions explicitly if any exist.

11. Skill-policy conformance pass

- Verify that current code and workflow still conform to repository skills
  (`sptth-dev`, `rust-error-handling`, `rust-testing`, `rust-security-checklist`).
- Identify mismatches between implemented behavior and skill guidance.
- Propose concrete remediation when drift is detected.

## Output Format

Provide:

1. Changed files
2. What was auto-fixed
3. What needs maintainer decision
4. Validation results (fmt/test/check)
5. Residual risks or follow-ups

## Guardrails

- Prefer minimal safe edits; avoid broad refactors unless requested.
- Do not silently introduce breaking dependency upgrades.
- Keep commit scope coherent if committing.
- Follow commit format in `docs/commit-message.md` and keep commit text in English.
