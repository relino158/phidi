# A03: Snapshot persistence + migration guards

## Metadata
- Track: Track A
- Difficulty: [hard]
- Target crates: phidi-proxy
- Dependencies: A01

## Session Goal
Implement local snapshot load/save with schema compatibility checks and safe migration behavior.

## Acceptance Criteria
1. Load/save works for valid snapshots
2. Incompatible versions return structured recovery status
3. Corrupt snapshots do not crash service startup.

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
