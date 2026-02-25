# Commit Message Format

This project uses a structured commit message format to preserve decision context.

## Required Structure (Scale by Change Size)

1. Small change (for example: 1 file, tiny fix/docs tweak)
   - Subject line
   - Optional 1-2 short bullets
2. Medium change (for example: a few files, one coherent behavior change)
   - Subject line
   - Up to 3 summary bullets
3. Large change (for example: multi-module feature/refactor)
   - Subject line
   - 3 summary bullets
   - Per-bullet detailed sections with:
     - Background
     - Review
     - Decision
     - Impact

## Template (Large Change)

```text
<subject>

- <summary 1>
- <summary 2>
- <summary 3>

## <summary 1>
- Background:
- Review:
- Decision:
- Impact:

## <summary 2>
- Background:
- Review:
- Decision:
- Impact:

## <summary 3>
- Background:
- Review:
- Decision:
- Impact:
```

## Example Command

```sh
git commit -m "Harden DNS source validation" \
  -m "- Validate upstream response source by full socket address
- Keep DNS forward retry behavior within timeout window
- Add tests for expected/invalid source combinations" \
  -m "## Validate upstream response source by full socket address
- Background: Upstream response source checks were too permissive.
- Review: IP-only checks allow packets from unexpected source ports.
- Decision: Compare full SocketAddr (IP + port).
- Impact: Better spoofing resistance for forwarded DNS responses.

## Keep DNS forward retry behavior within timeout window
- Background: Source validation should not degrade failover behavior.
- Review: Discard unexpected packets and continue until timeout.
- Decision: Preserve timeout window loop and fallback order.
- Impact: Security is improved without changing expected retry semantics.

## Add tests for expected/invalid source combinations
- Background: Validation logic should be regression-safe.
- Review: Added tests for exact match, port mismatch, and IP mismatch.
- Decision: Keep tests focused on source validation helper behavior.
- Impact: Future changes are less likely to weaken the check."
```

## Notes

- Keep each section concise and factual.
- Do not include unrelated changes in the same commit.
- If pre-commit checks are bypassed, state the reason in the commit body.
- Do not use the large-change template for tiny commits.
