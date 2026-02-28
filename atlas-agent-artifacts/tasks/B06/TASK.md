# B06: Workbench commands + panel wiring for six capabilities

## Metadata
- Track: Track B
- Difficulty: [medium]
- Target crates: phidi-app,phidi-rpc
- Dependencies: A02,B01

## Session Goal
Expose all six native operations through UI commands and a dedicated panel flow.

## Acceptance Criteria
1. Each operation can be invoked from the workbench command path
2. Panel renders typed success and error states
3. UI does not block during long-running operations.

## Validation Command

```bash
cargo test -p phidi-app -p phidi-rpc
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
