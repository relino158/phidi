# C01 Handoff

## Completed
- Promoted the renderer ABI/API contract into live crate exports in both `phidi-rpc` and `phidi-app` so the probe path is compiled and test-covered.
- Added shared host-support metadata to renderer load statuses, including the concrete host API version and ABI support window used for compatibility decisions.
- Added stable actionable guidance strings for renderer compatibility failures so ABI/API mismatches can tell callers what to rebuild or replace.
- Refactored `phidi-app` renderer probing so dynamic libraries and the built-in default renderer descriptor both go through the same descriptor validation and semver/ABI checks.
- Added built-in renderer tests plus updated probe tests for ABI mismatch, host API mismatch, missing entry, and invalid host version handling.

## Validation
- `cargo fmt --check`
- `cargo test -p phidi-app -p phidi-rpc`

## Unblocked
- Follow-on renderer activation work can consume one shared compatibility path for both bundled and external renderers instead of special-casing the default renderer.
- UI/status work can surface actionable renderer load failures directly from `RendererLoadStatus::actionable_message()`.
- Future renderer plugin tasks can negotiate against explicit host ABI/API support metadata already carried in the shared RPC contract.
