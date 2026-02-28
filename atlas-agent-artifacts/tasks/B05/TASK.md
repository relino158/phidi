# B05: Rename planner preview with confidence markers

## Metadata
- Track: Track B
- Difficulty: [hard]
- Target crates: phidi-proxy
- Dependencies: B02,B-SP1

## Session Goal
Generate multi-file rename preview plans with confidence annotations.

## Acceptance Criteria
1. Preview includes high-confidence and low-confidence buckets
2. Conflicts and unsafe edits are surfaced, not hidden
3. No in-place mutation occurs in preview mode.

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
