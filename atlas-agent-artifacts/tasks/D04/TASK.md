# D04: Plugin trust enforcement (unsigned blocked, hash-pinned override)

## Metadata
- Track: Track D
- Difficulty: [hard]
- Target crates: phidi-app,phidi-proxy
- Dependencies: C01,C02

## Session Goal
Enforce safety-first plugin trust defaults with auditable overrides.

## Acceptance Criteria
1. Unsigned third-party plugins are blocked by default
2. User override requires per-version hash pin
3. Hash/version changes trigger re-approval.

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
