# A-SP1: Rust macro certainty feasibility spike

## Metadata
- Track: Track A
- Difficulty: [spike]
- Target crates: phidi-proxy
- Dependencies: none

## Session Goal
Build a throwaway adapter branch to measure how macro-heavy code affects link certainty.

## Acceptance Criteria
1. Document findings and chosen certainty policy
2. Throwaway implementation is isolated and not merged directly
3. Follow-up notes provide concrete constraints for A05.

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
