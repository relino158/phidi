# B02: Concept discovery + entity briefing service

## Metadata
- Track: Track B
- Difficulty: [hard]
- Target crates: phidi-proxy
- Dependencies: A03,A05

## Session Goal
Implement core query service for concept lookup and entity-oriented briefings.

## Acceptance Criteria
1. Queries return stable ordering for identical snapshot
2. Responses include inbound/outbound relationship context
3. Results expose certainty and provenance metadata.

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
