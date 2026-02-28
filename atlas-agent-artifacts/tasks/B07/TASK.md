# B07: Determinism, timeout, and partial-result contract tests

## Metadata
- Track: Track B
- Difficulty: [medium]
- Target crates: phidi-rpc,phidi-proxy
- Dependencies: B01,B02,B03,B04,B05

## Session Goal
Add test suite for deterministic outputs and bounded execution behavior.

## Acceptance Criteria
1. Same input snapshot yields stable output ordering
2. Timeout path returns structured partial responses
3. Contract snapshots prevent accidental response-shape drift.

## Validation Command

```bash
cargo test -p phidi-rpc -p phidi-proxy
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
