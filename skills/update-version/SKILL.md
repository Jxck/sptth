---
name: update-version
description: Update project versions and release tag in one consistent flow (Cargo.toml, xtask, docs, and git tag).
---

# update-version Skill

Use this skill when preparing a release version update so crate versions and git tags do not drift.

## Goal

Keep release identifiers consistent across:

1. `Cargo.toml` (`[package].version`)
2. `xtask/Cargo.toml` (`[package].version`)
3. docs/examples that embed a fixed release version
4. git annotated tag (`vX.Y.Z`)

## Required Inputs

1. Optional bump kind argument:
   - default: `patch` (when no argument is given)
   - explicit: `minor` or `major`

## Required Procedure

1. Baseline checks

- Ensure working tree is clean (or changes are intentionally scoped).
- Read current version from `Cargo.toml`.
- Decide target version by bump kind:
  - no argument -> patch bump
  - `minor` -> minor bump with patch reset to `0`
  - `major` -> major bump with minor/patch reset to `0`
- Confirm target tag `vX.Y.Z` does not already exist.

2. Update versioned files

- Update `/Users/jxck/develop/sptth/Cargo.toml` to `version = "X.Y.Z"`.
- Update `/Users/jxck/develop/sptth/xtask/Cargo.toml` to `version = "X.Y.Z"`.
- Update fixed release examples in docs (for example `README.md` `VERSION=vX.Y.Z`).
- Do **not** edit `Cargo.lock` manually.
- Refresh lockfile metadata via Cargo commands after manifest updates (for example `cargo check --workspace`).

3. Validate

- Run:
  - `cargo run -p xtask -- fmt-check`
  - `cargo test -q`
  - `cargo check --workspace`

4. Commit

- Commit only files changed for version update.
- Follow `docs/commit-message.md`.
- Keep commit message in English.

5. Tag

- Create annotated tag `vX.Y.Z` on the update-version commit.
- Verify tag points to expected commit.

6. Push (if requested)

- Push branch and tag:
  - `git push origin main`
  - `git push origin vX.Y.Z`

## Command Examples

```sh
# bump examples
# /update-version          -> patch bump (default)
# /update-version minor    -> minor bump
# /update-version major    -> major bump

# example computed target: 0.0.4
git tag --list "v0.0.4"
cargo run -p xtask -- fmt-check
cargo test -q
cargo check --workspace
git tag -a v0.0.4 -m "v0.0.4"
git show --no-patch --oneline v0.0.4
```

## Output Format

Provide:

1. Updated files
2. Validation results (`fmt-check`/`test`/`check`)
3. Commit hash
4. Tag name and tagged commit
5. Push status (done or pending)

## Guardrails

- Never retag an existing version; bump instead.
- Do not include unrelated refactors in the update-version commit.
- If docs mention a fixed version string, keep it aligned with the new tag.
