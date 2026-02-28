# A06: Foundation CI quality gates

## Metadata
- Track: Track A
- Difficulty: [medium]
- Target crates: phidi-core,phidi-rpc,phidi-proxy
- Dependencies: A02,A03,A04,A05

## Session Goal
Add mandatory checks for schema correctness, freshness behavior, and partial-output guarantees.

## Acceptance Criteria
1. CI blocks merges on foundation regressions
2. Test fixtures are stable and documented
3. Failure output identifies failing contract quickly.

## Validation Command

```bash
cargo test -p phidi-core -p phidi-rpc -p phidi-proxy
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
