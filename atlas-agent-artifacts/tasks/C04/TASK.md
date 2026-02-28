# C04: Renderer interaction bridge

## Metadata
- Track: Track C
- Difficulty: [medium]
- Target crates: phidi-app,phidi-rpc,phidi-proxy
- Dependencies: C02,B02,B06

## Session Goal
Bridge hover/click/filter interactions to typed backend queries and updates.

## Acceptance Criteria
1. Interaction events are typed end-to-end
2. Drill-down actions open matching entity/context views
3. Error states are surfaced without UI lockup.

## Validation Command

```bash
cargo test -p phidi-app -p phidi-rpc -p phidi-proxy
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
