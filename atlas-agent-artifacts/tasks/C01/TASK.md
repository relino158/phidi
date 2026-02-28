# C01: Renderer ABI/API and version checks

## Metadata
- Track: Track C
- Difficulty: [hard]
- Target crates: phidi-app,phidi-rpc
- Dependencies: A02,C-SP1

## Session Goal
Implement renderer interface contract and strict compatibility checks.

## Acceptance Criteria
1. Incompatible plugin versions are rejected safely
2. Compatibility errors are actionable
3. Built-in default renderer follows same contract path.

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
