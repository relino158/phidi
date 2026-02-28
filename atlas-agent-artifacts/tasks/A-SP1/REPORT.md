# A-SP1 Report: Rust macro certainty feasibility spike

## Scope

- Branch under test: `experiments/atlas-a0`
- Throwaway probe location: `phidi-proxy/tests/a_sp1_macro_certainty_spike.rs`
- Probe method: parse representative Rust snippets with `syn::parse_file` plus `syn::visit::Visit`, then classify only syntax-visible facts against what `A05` needs to emit.

The probe is isolated to an integration test on purpose. It does not ship in the proxy runtime and it keeps the spike logic out of the baseline adapter implementation path.

## Findings

1. Item macro invocations are visible with high certainty at the callsite, but their expanded items are not visible in a parser-only AST.
2. `#[derive(...)]` lists are visible with high certainty, but the resulting trait impls and generated methods are not.
3. Attribute macro attachments are visible with high certainty, but any wrappers, sibling items, or rewritten bodies require compiler-backed expansion to confirm.
4. A parser-only baseline can still preserve useful provenance by recording macro invocations and macro-related attachments without over-claiming generated entities or edges.

## Chosen certainty policy

- `Certain`:
  - explicit items parsed from source
  - explicit impl blocks parsed from source
  - item macro invocation sites
  - derive macro names attached to an item
  - attribute macro attachments when the adapter recognizes the attribute as macro-relevant input
- `Omit from A05 baseline`:
  - macro-expanded items
  - derive-generated impl edges
  - attribute-macro-generated wrappers, sibling items, and call edges
  - any symbol/link inferred only from macro token bodies without compiler expansion

This keeps `A05` deterministic and honest: direct syntax can be emitted with top certainty, while macro-driven expansion stays out of the baseline snapshot until a compiler-backed or expansion-aware adapter exists.

## Constraints For A05

1. Record macro provenance separately from ordinary item/link extraction. `A05` should be able to say “this item invokes `make_helper!`” or “this struct uses `derive(Debug)`” without claiming generated output exists.
2. Do not synthesize generated item names or impl edges from macro tokens. Token streams are too unconstrained to support deterministic, high-certainty structural extraction.
3. Keep certainty scoring asymmetric:
   - direct syntax facts can be emitted as the highest certainty tier
   - macro attachments can be emitted as certain provenance facts
   - expansion-derived entities/edges should be omitted, not downgraded into a misleading low-confidence structural graph
4. Preserve partial-output behavior around macros. If a file parses, macro-heavy regions should still yield direct syntax facts plus macro provenance. If parsing fails, degrade at file scope without crashing the adapter.

## Test Note

- Updated tests: yes
- Validation target: `cargo test -p phidi-proxy`
- The spike harness is intentionally test-only; there is no production adapter code in this packet.

## Handoff

`A05` is now unblocked to build a parser-first Rust baseline extractor with a clear boundary:

- emit direct syntax entities and explicit impls
- emit macro provenance facts
- omit expansion-derived structure until the project chooses an expansion-aware backend
