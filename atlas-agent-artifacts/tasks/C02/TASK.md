# C02: Renderer host lifecycle + sandbox policy

## Metadata
- Track: Track C
- Difficulty: [hard]
- Target crates: phidi-app,phidi-proxy
- Dependencies: C01

## Session Goal
Implement plugin lifecycle controls and capability-restricted execution policy.

## Acceptance Criteria
1. Start/stop/reload lifecycle is deterministic
2. Sandbox denies network and process spawn by default
3. Capability prompts are explicit and revocable.

## Validation Command

```bash
cargo test -p phidi-app -p phidi-proxy
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
