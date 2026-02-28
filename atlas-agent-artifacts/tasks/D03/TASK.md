# D03: Snapshot compatibility matrix tests

## Metadata
- Track: Track D
- Difficulty: [medium]
- Target crates: phidi-core,phidi-proxy
- Dependencies: A01,A03

## Session Goal
Guarantee read behavior for current and previous schema minor versions.

## Acceptance Criteria
1. Compatibility matrix includes required version pairs
2. Unsupported versions return explicit migration guidance
3. Backward-read regressions block CI.

## Validation Command

```bash
cargo test -p phidi-core -p phidi-proxy
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
