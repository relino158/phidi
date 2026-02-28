# D01: Benchmark harness and CI thresholds

## Metadata
- Track: Track D
- Difficulty: [hard]
- Target crates: phidi-core,phidi-rpc,phidi-proxy,phidi-app
- Dependencies: A05,B02,B03,B04,B05,C03

## Session Goal
Create benchmark suite and enforce initial performance gates in CI.

## Acceptance Criteria
1. CI fails when threshold regressions exceed budget
2. Benchmark fixtures represent medium workspace profile
3. Reports include trend-friendly metrics output.

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
