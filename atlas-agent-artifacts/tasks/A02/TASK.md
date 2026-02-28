# A02: Snapshot/freshness RPC contract + version negotiation

## Metadata
- Track: Track A
- Difficulty: [medium]
- Target crates: phidi-rpc
- Dependencies: A01

## Session Goal
Add typed protocol messages for snapshot status, freshness state, and schema negotiation.

## Acceptance Criteria
1. Request/response types are stable and documented
2. Compatibility tests cover serialization and unknown fields
3. RPC version mismatch yields explicit status.

## Validation Command

```bash
cargo test -p phidi-rpc
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
