# B01: Agent capability RPC schemas

## Metadata
- Track: Track B
- Difficulty: [medium]
- Target crates: phidi-rpc
- Dependencies: A02

## Session Goal
Define typed contracts for six native agent operations.

## Acceptance Criteria
1. All six operation schemas are present
2. Error and timeout envelopes are standardized
3. Schema docs include certainty/provenance fields.

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
