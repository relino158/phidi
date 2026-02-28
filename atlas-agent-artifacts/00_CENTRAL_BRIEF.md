# Phidi AI-First Code Atlas - Central Brief

## Purpose
Build a local-first semantic map inside Phidi (Lapce fork) that helps humans and AI agents stay oriented while code changes scale quickly.

## Cleanroom Constraints
- Do not copy source code, identifiers, naming patterns, or non-standard vocabulary from the reference repository.
- Implement from this brief, RFCs, and public language/editor standards only.
- Keep implementation and API names independent from the studied project.

## v1 Product Scope
- Language support: Rust only.
- Workspace model: currently open workspace only.
- Agent features: concept discovery, entity briefing, blast radius estimation, working-tree impact scan, rename planning, advanced structural query.
- Visual layer: one default renderer (sunburst + bundled relationship arcs), but renderer architecture must be plugin-first.

## v1 Non-Goals
- No multi-language parity.
- No central hosted index service.
- No full runtime tracing.

## Core Terms
- Semantic Map: normalized graph-like project model for one workspace.
- Workspace Snapshot: persisted semantic map artifact used by UI and agent tools.
- Seed Snapshot: optional VCS-tracked bootstrap artifact.
- Route Trace: likely execution route derived from static evidence.
- Blast Radius: expected impact area from a proposed change.
- Confidence Score: confidence level for inferred links.

## Quality Principles
- Deterministic outputs for same input snapshot.
- Explicit uncertainty in all inferred results.
- Graceful partial results over hard failures.
- No UI-thread blocking for analysis work.
- Safety-first plugin execution.

## Agreed Decisions
1. Seed snapshot default is opt-out.
2. Rust toolchain policy:
   - MSRV: 1.87.0.
   - Default channel: stable.
   - CI gate: MSRV + latest stable required; beta optional non-blocking warning.
3. Unsigned renderer plugins:
   - Blocked by default.
   - Allow explicit user override with per-plugin, per-version hash pinning.
4. Initial performance gates target medium-sized Rust workspaces:
   - Cold build <= 120s.
   - Snapshot load <= 3s.
   - Single-file incremental refresh p95 <= 1.5s; p99 <= 3s.
   - Common query p95 <= 400ms; p99 <= 1.2s.
   - Peak memory during cold build <= 2.5GB.

## Workspace Crates (Target Integration)
- phidi-core
- phidi-rpc
- phidi-proxy
- phidi-app
