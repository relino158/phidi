# A05: Rust adapter baseline extraction with partial output

## Metadata
- Track: Track A
- Difficulty: [hard]
- Target crates: phidi-proxy
- Dependencies: A01,A-SP1

## Session Goal
Emit baseline Rust entities and relationships while preserving service availability on parse failures.

## Acceptance Criteria
1. Representative Rust constructs are emitted
2. File-level failures degrade to partial output only
3. Inferred links carry confidence scores.

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
