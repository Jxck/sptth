# Release Playbook

This document defines the release process for `sptth`, including signing, attestations, and pre-release risk checks.

## Security Model

Releases must provide:

1. Source integrity (signed commits/tags)
2. Artifact integrity (checksums + signature)
3. Build provenance (attestation from CI)

`sptth` should be treated as a local-development tool with privileged behavior (`sudo`, local CA, DNS/proxy changes). Release quality and trust signals must be strict.

## Required GitHub Protection

Before publishing releases, enforce:

1. Branch protection on `main`
2. Required reviews for `.github/workflows/*` changes
3. Restricted tag creation for `v*`
4. Signed tag policy for release tags

## Release Workflow Requirements

The release workflow must include:

1. Matrix build for supported OS/arch targets
2. `SHA256SUMS` generation for all artifacts
3. Keyless artifact signing via Sigstore `cosign`
4. GitHub artifact attestations (build provenance)

### Minimum Workflow Permissions

```yaml
permissions:
  contents: write
  id-token: write
  attestations: write
```

## Pre-Release Checklist

Run these before tagging:

1. `cargo run -p xtask -- fmt-check`
2. `cargo test -q`
3. `cargo check --workspace`
4. Run `health-check` skill (`skills/health-check/SKILL.md`)
5. Confirm docs and `config.example.toml` reflect current behavior
6. Confirm no unintended language drift in repository docs

## Tag and Publish

1. Update version/changelog
2. Create signed release tag
3. Push tag
4. Wait for release workflow to finish
5. Verify generated artifacts and metadata

## Keyless Signing (No Long-Lived Private Key)

With keyless signing:

- You do **not** manage a long-lived artifact signing private key.
- CI obtains a short-lived certificate via OIDC.
- Signature and certificate are generated per release in CI.

## Artifact Verification (Consumer Side)

For each downloaded file:

1. Verify checksum (`SHA256SUMS`)
2. Verify signature/certificate (`cosign verify-blob`)
3. Verify provenance attestation

## Known Risks to Watch

1. Workflow tampering
2. Unprotected release tags
3. Missing verification instructions for users
4. Non-reproducible build environment drift
5. Dependency supply-chain issues
6. Platform trust warnings (macOS Gatekeeper / Windows SmartScreen)
7. Privileged runtime behavior misuse

## Failure Policy

Do **not** publish a release when:

- Required checks fail
- Sign/attestation steps fail
- Provenance cannot be verified
- Critical documentation mismatch is detected
