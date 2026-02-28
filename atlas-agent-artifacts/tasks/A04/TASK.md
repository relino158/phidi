# A04: Freshness evaluator using commit + dirty state

## Metadata
- Track: Track A
- Difficulty: [medium]
- Target crates: phidi-proxy
- Dependencies: A03

## Session Goal
Map git state to freshness states used by UI and tools.

## Acceptance Criteria
1. Freshness mapping covers exact/drifted/outdated/incompatible
2. State transitions are deterministic under test fixtures
3. Status includes actionable rebuild guidance.

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
