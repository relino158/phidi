# C-SP1: Renderer ABI boundary spike with throwaway plugin

## Metadata
- Track: Track C
- Difficulty: [spike]
- Target crates: phidi-app,phidi-rpc
- Dependencies: A02

## Session Goal
Validate ABI/API boundaries and failure modes before committing host design.

## Acceptance Criteria
1. Throwaway plugin proves load/version/error pathways
2. ABI boundary assumptions are written down
3. Follow-up constraints are clear for C01.

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
