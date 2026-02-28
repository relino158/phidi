# Agent Session Model

## Unit of Assignment
Assign one PR-sized task packet per agent session.

## Session Size Rules
- One concrete outcome tied to 1 to 3 acceptance criteria.
- Prefer one crate scope, maximum two crates.
- Should finish in one session (roughly 60 to 120 minutes).
- Includes code, tests, and contract/docs updates for that slice.
- Produces exactly one branch and one PR.

## Split Rules
Split a task packet if any of these is true:
- More than two crates are modified.
- More than ~300 net lines expected.
- More than one unresolved technical risk.
- No clear validation command exists.

## Session Roles
- Builder: implementation packet owner.
- Integrator: cross-crate wiring packet owner.
- Verifier: regression/perf/security validation and review packet owner.

## Required Session Deliverables
- PR with linked task ID.
- Tests or explicit test-gap rationale.
- Short risk note: what could regress.
- Handoff note: next dependency now unblocked.

## Failure Handling
- If blocked by missing dependency, stop and mark packet as blocked.
- If blocked by uncertainty, create/update a spike packet.
- Do not expand scope to absorb blocked dependencies.
