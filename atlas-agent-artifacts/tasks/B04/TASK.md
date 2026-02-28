# B04: Working-tree delta impact scanner

## Metadata
- Track: Track B
- Difficulty: [medium]
- Target crates: phidi-proxy
- Dependencies: A04,B02

## Session Goal
Map uncommitted edits to likely affected areas of the semantic map.

## Acceptance Criteria
1. Scanner handles staged and unstaged edits
2. Output links changed files to impacted entities
3. When analysis is partial, response includes explicit completeness flags.

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
