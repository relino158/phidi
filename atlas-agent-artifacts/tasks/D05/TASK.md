# D05: Release checklist and operator documentation

## Metadata
- Track: Track D
- Difficulty: [trivial]
- Target crates: phidi-app,phidi-proxy,phidi-rpc,phidi-core
- Dependencies: D01,D02,D03,D04

## Session Goal
Document operational playbooks and release sign-off criteria.

## Acceptance Criteria
1. Rebuild, recovery, and troubleshooting flows are documented
2. Checklist maps directly to release gates
3. Docs are linked from main project contributor guidance.

## Validation Command

```bash
cargo test -p phidi-core -p phidi-rpc -p phidi-proxy -p phidi-app
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
