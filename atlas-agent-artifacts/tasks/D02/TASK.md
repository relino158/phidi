# D02: Fault-injection and recovery suite

## Metadata
- Track: Track D
- Difficulty: [hard]
- Target crates: phidi-proxy,phidi-app
- Dependencies: A03,A05

## Session Goal
Validate resilience for parser/storage failures and degraded-mode behavior.

## Acceptance Criteria
1. Injected parser/storage faults do not crash session
2. Recovery paths are deterministic and testable
3. User-visible status clearly indicates degraded mode.

## Validation Command

```bash
cargo test -p phidi-proxy -p phidi-app
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
