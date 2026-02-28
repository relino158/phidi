# C-SP1 Report: Renderer ABI boundary spike with throwaway plugin

## Scope

- Branch under test: `experiments/atlas-c-sp1`
- Probe code:
  - `phidi-rpc/src/renderer.rs`
  - `phidi-app/src/renderer_host.rs`
  - `phidi-app/tests/fixtures/renderers/`
- Probe method: load a throwaway `cdylib`, resolve one exported symbol, copy a static descriptor, then validate descriptor layout, ABI version, and host API semver requirement.

This spike stays intentionally narrow. It proves discovery and compatibility behavior without committing the project to a render callback surface yet.

## Findings

1. One metadata-only entry symbol is enough to prove the host can distinguish load failure, missing symbol, ABI mismatch, and host API mismatch before any renderer logic executes.
2. A shared `#[repr(C)]` descriptor in `phidi-rpc` is a practical cleanroom boundary for both the host and a throwaway plugin fixture.
3. The ABI line and the host API line should be versioned separately:
   - ABI: numeric major/minor layout compatibility
   - host API: semver requirement string evaluated by the host
4. Exact `struct_size` validation is a cheap way to catch descriptor layout drift at the boundary before dereferencing any optional future fields.
5. The host can safely unload immediately after probing as long as the plugin only exposes immutable metadata and the host copies it before the library is dropped.

## Chosen boundary assumptions

- Required symbol name: `phidi_renderer_descriptor_v1`
- Symbol type: `extern "C"` function returning `*const RendererPluginDescriptorV1`
- Returned descriptor points at immutable static plugin-owned data
- String fields are NUL-terminated UTF-8 and valid for the lifetime of the loaded library
- The v1 descriptor is metadata-only; it does not expose render callbacks yet
- Current host support window is exact ABI `1.0`; older/newer revisions are rejected explicitly in this spike

## Constraints For C01

1. Route the built-in default renderer through the same descriptor and compatibility path. Do not special-case the default renderer after this spike.
2. Keep the metadata probe separate from any callable render surface. If C01 adds function pointers or handles, add them as a clearly versioned next step instead of overloading the probe contract.
3. Preserve actionable failure categories in the host:
   - load failure
   - missing entry symbol
   - invalid descriptor
   - ABI mismatch
   - host API mismatch
   - invalid host API requirement
4. Decide callback ownership explicitly before C01 adds execution:
   - either keep the dynamic library alive behind a renderer handle
   - or require an explicit destroy/unload hook for any plugin-owned state
5. If the descriptor layout changes, prefer a new symbol/version pair over silently widening the old struct unless prefix-compatibility rules are designed and tested on purpose.
6. Apply unsigned-plugin policy before activation, not just before discovery. The probe path is a compatibility check, not a trust decision.

## Test Note

- Updated tests: yes
- Validation target: `cargo test -p phidi-app -p phidi-rpc`
- The spike uses throwaway `cdylib` fixtures under `phidi-app/tests/fixtures/renderers/`

## Handoff

`C01` is now unblocked to implement the real renderer host contract on top of a proven boundary:

- stable one-symbol probe path
- explicit ABI compatibility result
- explicit host API compatibility result
- test-backed throwaway plugin fixtures for success and failure cases
