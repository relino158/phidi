# A01: Semantic map domain contract

## Metadata
- Track: Track A
- Difficulty: [hard]
- Target crates: phidi-core
- Dependencies: none

## Session Goal
Define the core semantic-map domain records and enums for freshness and certainty.

## Acceptance Criteria
1. Core types compile and are documented
2. Version field strategy is present
3. No crate outside phidi-core is required to compile this packet.

## Validation Command

```bash
cargo test -p phidi-core
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
