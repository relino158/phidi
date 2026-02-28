# Decision Log

## Locked Decisions

### D-001: Seed Snapshot Default
- Status: Accepted
- Decision: Seed snapshot is opt-out by default.
- Rationale: Keep onboarding fast while avoiding forced VCS artifact growth.

### D-002: Rust Toolchain Support Policy
- Status: Accepted
- Decision:
  - MSRV is 1.87.0.
  - Default developer channel is stable.
  - CI requires MSRV and latest stable; beta is optional warning lane.
- Rationale: Aligns with common toolchains while preserving predictability.

### D-003: Unsigned Renderer Plugin Trust Policy
- Status: Accepted
- Decision:
  - Unsigned third-party renderer plugins blocked by default.
  - User override allowed per plugin/version with hash pinning.
  - Elevated capabilities require explicit consent.
- Rationale: Safety-first defaults with explicit escape hatch.

### D-004: Initial Performance Gates
- Status: Accepted
- Decision (target profile: medium Rust workspace):
  - cold build <= 120s
  - snapshot load <= 3s
  - single-file incremental refresh p95 <= 1.5s, p99 <= 3s
  - common query p95 <= 400ms, p99 <= 1.2s
  - peak memory during cold build <= 2.5GB
- Rationale: Keeps user feedback loop responsive without unrealistic targets.

## Open Decisions (Keep Updated)
- None at this stage.
