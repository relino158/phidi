# C03: Default renderer plugin (sunburst + bundled links)

## Metadata
- Track: Track C
- Difficulty: [hard]
- Target crates: phidi-app
- Dependencies: C02,A05

## Session Goal
Ship default visualization plugin using semantic map view models.

## Acceptance Criteria
1. Hierarchy and cross-link visuals render from normalized input
2. Certainty filters are supported
3. Rendering remains responsive on medium workspaces.

## Validation Command

```bash
cargo test -p phidi-app
```

## Expected Deliverables
1. One PR scoped to this task ID.
2. Tests updated or an explicit test-gap note.
3. Handoff note listing what is now unblocked.

## Guardrails
- Keep scope within this packet.
- If blocked by a missing dependency, mark blocked and stop.
- Do not include copied code/text from external reference implementations.
