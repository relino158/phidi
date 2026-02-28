# B-SP1: Rename ambiguity strategy spike

## Metadata
- Track: Track B
- Difficulty: [spike]
- Target crates: phidi-proxy
- Dependencies: B02

## Session Goal
Prototype rename candidate ranking and conflict handling before implementation.

## Acceptance Criteria
1. Throwaway prototype demonstrates ambiguous cases
2. Ranking strategy is selected with justification
3. Risks and guardrails documented for B05.

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
