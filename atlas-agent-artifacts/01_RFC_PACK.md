# RFC Pack

## RFC-001: Boundaries and Cleanroom Rules
### Objective
Define legal and engineering boundaries for an MIT-compatible implementation.

### Requirements
- Behavior-first design and contract-first interfaces.
- New naming system for all public APIs.
- Workspace-local operation first.
- No borrowed proprietary text, identifiers, or snippets.

### Exit Criteria
- Cleanroom checklist in PR template.
- Public glossary for all new API terms.
- Architecture docs contain no copied reference content.

## RFC-002: Semantic Map Contract and Snapshot Lifecycle
### Objective
Standardize the intermediate contract and lifecycle of persisted map artifacts.

### Requirements
- Versioned, deterministic schema.
- Two artifact modes: local working snapshot + optional seed snapshot.
- Freshness states: exact, drifted, outdated, incompatible.
- Certainty metadata on inferred links.

### Exit Criteria
- Version compatibility checks enforced on read/write.
- Freshness transitions covered by tests.
- Safe behavior for incompatible snapshots.

## RFC-003: Rust Adapter Contract
### Objective
Define Rust v1 extraction behavior and incremental update rules.

### Requirements
- Coverage: modules, imports, types, functions, methods, traits, impl blocks, tests, macro-aware best effort.
- Incremental recomputation by affected scope.
- Partial-output mode on parser/analysis failures.

### Exit Criteria
- Corpus tests for representative Rust patterns.
- Partial-output behavior validated.
- Confidence scores present for non-direct links.

## RFC-004: Native Agent Capability Contract
### Objective
Provide built-in agent operations over semantic data with stable machine-readable outputs.

### Requirements
- Capability set:
  - concept discovery
  - entity briefing
  - blast radius estimation
  - uncommitted delta impact scan
  - coordinated rename planning
  - advanced structural query
- Time-bounded execution with partial-result fallback.
- Deterministic ordering/content for same snapshot.

### Exit Criteria
- Typed request/response schemas in phidi-rpc.
- Determinism and timeout behavior tests pass.

## RFC-005: Visualization Plugin Host Contract
### Objective
Make renderer plugins fully swappable, including default renderer.

### Requirements
- Stable renderer ABI/API with explicit version checks.
- Sandbox policy and capability gating.
- Default renderer plugin implements sunburst + bundled links.

### Exit Criteria
- Default renderer can be disabled and replaced.
- Typed UI interaction events round-trip through RPC.
- Large-workspace responsiveness target is met.

## RFC-006: Reliability, Performance, and Release Gates
### Objective
Set measurable release-quality bars.

### Requirements
- Background-only analysis operations.
- Concurrency-safe snapshot access.
- Resource ceilings and timeout controls.
- CI benchmark/fault-injection/compatibility gates.

### Exit Criteria
- Performance thresholds enforced in CI.
- Recovery from parser/store failures validated.
- Snapshot compatibility matrix for current and previous minor schema versions.
