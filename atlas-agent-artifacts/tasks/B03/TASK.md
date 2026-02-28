# B03: Blast radius estimator

## Metadata
- Track: Track B
- Difficulty: [hard]
- Target crates: phidi-proxy
- Dependencies: B02

## Session Goal
Estimate direct and indirect impact of proposed symbol changes.

## Acceptance Criteria
1. Output clearly separates direct vs indirect radius
2. Confidence scores are preserved in estimates
3. Edge cases with unresolved references are represented safely.

## Validation Command

```bash
cargo test -p phidi-proxy
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
