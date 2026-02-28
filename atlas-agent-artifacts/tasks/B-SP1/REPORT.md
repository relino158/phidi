# B-SP1 Report: Rename ambiguity strategy spike

## Scope

- Branch under test: `experiments/atlas-b-sp1`
- Throwaway probe location: `phidi-proxy/tests/b_sp1_rename_ambiguity_spike.rs`
- Probe method: parse representative Rust call expressions with `syn::parse_str` plus `syn::visit::Visit`, then rank same-name rename candidates using only syntax-visible evidence already aligned with the current extractor model:
  - target name
  - optional call path text
  - caller file path
  - caller qualified name
  - callable kind (`function` vs `method`)

The probe is intentionally test-only. It does not introduce production rename-planning code and it keeps the experiment scoped to the ambiguity policy that `B05` needs.

## Findings

1. Explicit call paths are the only syntax-only signal strong enough to promote a same-name candidate directly into the high-confidence bucket.
2. Unqualified function calls can be ranked deterministically with locality signals such as same-file and shared module-prefix depth, but those signals are heuristic and should stay in the low-confidence bucket.
3. Same-name method calls are qualitatively different from free-function calls: `syn::ExprMethodCall` exposes the receiver expression and method name, but not a resolved receiver type, so multiple method candidates should surface as conflicts instead of guessed edits.
4. Additive numeric weights are unnecessary for the baseline. A lexicographic evidence tuple is simpler and more stable:
   - explicit qualified-path match
   - same file
   - shared module-prefix depth
5. The current extractor behavior in `resolve_inferred_calls` proves the same core risk: ambiguity is common enough that the baseline already drops multi-candidate matches. `B05` must not silently drop them; it needs to surface them as preview conflicts.

## Chosen ranking strategy

- Rank candidates with a deterministic evidence tuple, not a blended score:
  - `exact_path_match`
  - `same_file`
  - `shared_module_prefix`
- Classify the top result as:
  - `high-confidence` when an explicit qualified path uniquely identifies one candidate
  - `low-confidence` when one candidate wins only through locality heuristics
  - `conflict` when the top evidence ties or when multiple same-name method candidates exist without stronger syntax evidence

This keeps the preview deterministic and explainable. Every bucket boundary maps to a concrete kind of evidence instead of an opaque threshold.

## Constraints For B05

1. Preserve ambiguous hits as first-class output. Do not discard them just because they cannot be auto-promoted into a safe edit.
2. Keep method-call handling conservative in the syntax-only baseline:
   - one same-name method candidate can be previewed as low confidence
   - multiple same-name method candidates should become conflicts unless `B05` adds stronger evidence than this spike used
3. Prefer evidence tiers over hand-tuned numeric weights for the first implementation. If `B05` later needs more signals, add them as new ordered fields rather than rebalancing a weighted sum.
4. Attach a human-readable reason to every emitted edit or conflict so the UI can explain why something is high confidence, low confidence, or blocked.
5. Treat bare identifier calls as unqualified syntax even though the parsed path text is present. A single-segment path like `render()` is not the same signal as `graphics::render()`.
6. If `B05` scans text beyond existing snapshot edges, keep the default failure mode conservative:
   - exact path winner: emit editable preview
   - heuristic winner: emit low-confidence preview
   - unresolved tie: emit conflict and require manual review

## Test Note

- Updated tests: yes
- Validation target: `cargo test -p phidi-proxy`
- The spike harness is intentionally isolated to one integration test file; no production rename-planning code ships in this packet.

## Handoff

`B05` is now unblocked to implement rename preview planning with a concrete ambiguity policy:

- deterministic lexicographic candidate ranking
- explicit separation between path-backed and heuristic-backed edits
- conflict-first handling for unresolved ties
- conservative treatment of same-name method calls under syntax-only analysis
