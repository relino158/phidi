# C02 Handoff

## Completed
- Added a stateful renderer host in `phidi-app` so start, stop, and reload operate through one deterministic lifecycle owner instead of metadata-only probing.
- Kept renderer activation and probing on the same compatibility path, while preserving the loaded dynamic library handle for active renderer sessions.
- Added declarative plugin capabilities in shared metadata, with `network` and `process-spawn` modeled explicitly.
- Added persisted capability grants in `phidi-app` and exposed allow/revoke actions in the plugin menu so grants are revocable without manual file edits.
- Threaded capability grants through proxy initialization and live updates so `phidi-proxy` enforces them centrally.
- Changed the WASI sandbox default to deny outbound HTTP unless the plugin has an explicit granted `network` capability.
- Gated host-side process escape hatches (`ExecuteProcess`, debugger registration, and `StartLspServer`) behind the explicit `process-spawn` capability and emit revocable user-facing guidance when denied.

## Validation
- `cargo fmt --all`
- `cargo test -p phidi-app -p phidi-proxy`

## Unblocked
- `C03` can activate the built-in default renderer through a real lifecycle owner instead of adding a second activation path.
- Future renderer work can reload renderer implementations deterministically after trust/policy changes without rebuilding the host contract again.
- `C04` can rely on explicit plugin capability metadata and persisted grants when wiring renderer or plugin-side interactions that need elevated access.
