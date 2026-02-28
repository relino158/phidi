//! Typed renderer-plugin ABI records for host probing and compatibility checks.

use std::ffi::c_char;

use serde::{Deserialize, Serialize};

/// Exported symbol used by v1 renderer plugins.
pub const RENDERER_ENTRY_SYMBOL_V1: &str = "phidi_renderer_descriptor_v1";

/// The newest renderer ABI revision this build knows how to load.
pub const CURRENT_RENDERER_ABI_VERSION: RendererAbiVersion =
    RendererAbiVersion::new(1, 0);

/// The oldest renderer ABI revision this build promises to read.
pub const MINIMUM_RENDERER_ABI_VERSION: RendererAbiVersion =
    CURRENT_RENDERER_ABI_VERSION;

/// Renderer ABI revision encoded as major/minor components.
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct RendererAbiVersion {
    /// Breaking ABI line. Hosts require the same major version.
    pub major: u16,
    /// Backward-compatible revision within one ABI line.
    pub minor: u16,
}

impl RendererAbiVersion {
    /// Creates a renderer ABI version.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Compares this version against the supported range for a host.
    pub const fn compatibility(
        self,
        current: RendererAbiVersion,
        minimum: RendererAbiVersion,
    ) -> RendererAbiCompatibility {
        if self.major != current.major {
            return if self.major < current.major {
                RendererAbiCompatibility::TooOld
            } else {
                RendererAbiCompatibility::TooNew
            };
        }

        if self.minor > current.minor {
            RendererAbiCompatibility::TooNew
        } else if self.minor < minimum.minor {
            RendererAbiCompatibility::TooOld
        } else if self.minor == current.minor {
            RendererAbiCompatibility::Current
        } else {
            RendererAbiCompatibility::Compatible
        }
    }

    /// Compares this version against the support policy compiled into the current build.
    pub const fn compatibility_with_current(self) -> RendererAbiCompatibility {
        self.compatibility(
            CURRENT_RENDERER_ABI_VERSION,
            MINIMUM_RENDERER_ABI_VERSION,
        )
    }
}

/// Result of comparing a plugin ABI version with the current host.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RendererAbiCompatibility {
    /// Matches the ABI revision emitted by this build.
    Current,
    /// Older but still readable without a breaking ABI change.
    Compatible,
    /// Older than the minimum ABI revision the host accepts.
    TooOld,
    /// Newer than the host understands.
    TooNew,
}

/// Raw v1 descriptor returned by a renderer plugin entrypoint.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RendererPluginDescriptorV1 {
    /// The plugin's view of this descriptor size, used to detect layout drift.
    pub struct_size: u32,
    /// Renderer ABI revision compiled into the plugin.
    pub abi_version: RendererAbiVersion,
    /// NUL-terminated plugin identifier for logs and diagnostics.
    pub plugin_name: *const c_char,
    /// NUL-terminated plugin version string.
    pub plugin_version: *const c_char,
    /// NUL-terminated semver requirement for the host-side renderer API.
    pub host_api_requirement: *const c_char,
}

// Safety: the descriptor is immutable metadata. Sharing it is sound as long as
// producers only point at immutable, process-valid strings, which is the v1 ABI
// contract this spike is validating.
unsafe impl Sync for RendererPluginDescriptorV1 {}

impl RendererPluginDescriptorV1 {
    /// Descriptor size the current host expects for v1.
    pub const fn expected_size() -> u32 {
        size_of_descriptor()
    }
}

/// Export type for the v1 renderer descriptor symbol.
pub type RendererPluginEntryV1 =
    unsafe extern "C" fn() -> *const RendererPluginDescriptorV1;

/// Safe metadata copied out of a renderer plugin descriptor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RendererPluginMetadata {
    pub plugin_name: String,
    pub plugin_version: String,
    pub abi_version: RendererAbiVersion,
    pub host_api_requirement: String,
}

/// High-level outcome from probing one renderer plugin library.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum RendererLoadStatus {
    /// Library loaded and the descriptor matched the current host contract.
    Ready { plugin: RendererPluginMetadata },
    /// Library loaded, but the expected entry symbol was absent.
    MissingEntry { symbol: String },
    /// Library could not be loaded at all.
    LoadFailure { message: String },
    /// The plugin returned a null descriptor pointer.
    NullDescriptor { symbol: String },
    /// The plugin descriptor failed layout or string validation.
    InvalidDescriptor { message: String },
    /// The plugin ABI revision is outside the host's readable window.
    AbiMismatch {
        plugin: RendererPluginMetadata,
        expected_abi_version: RendererAbiVersion,
        compatibility: RendererAbiCompatibility,
    },
    /// The plugin requires a newer or otherwise incompatible host API version.
    HostApiMismatch {
        plugin: RendererPluginMetadata,
        host_api_version: String,
    },
    /// The plugin declared an invalid semver requirement for the host API.
    InvalidHostApiRequirement {
        plugin: RendererPluginMetadata,
        message: String,
    },
    /// The host API version string was invalid.
    InvalidHostApiVersion {
        host_api_version: String,
        message: String,
    },
}

const fn size_of_descriptor() -> u32 {
    std::mem::size_of::<RendererPluginDescriptorV1>() as u32
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        CURRENT_RENDERER_ABI_VERSION, RENDERER_ENTRY_SYMBOL_V1,
        RendererAbiCompatibility, RendererAbiVersion, RendererLoadStatus,
        RendererPluginDescriptorV1, RendererPluginMetadata,
    };

    #[test]
    fn renderer_abi_version_reports_too_new_major_versions() {
        let compatibility =
            RendererAbiVersion::new(2, 0).compatibility_with_current();

        assert_eq!(compatibility, RendererAbiCompatibility::TooNew);
    }

    #[test]
    fn renderer_load_status_serializes_stably() {
        let response = RendererLoadStatus::AbiMismatch {
            plugin: RendererPluginMetadata {
                plugin_name: "throwaway".to_string(),
                plugin_version: "0.1.0".to_string(),
                abi_version: RendererAbiVersion::new(2, 0),
                host_api_requirement: ">=0.1.0, <0.2.0".to_string(),
            },
            expected_abi_version: CURRENT_RENDERER_ABI_VERSION,
            compatibility: RendererAbiCompatibility::TooNew,
        };

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["status"], json!("abi-mismatch"));
        assert_eq!(
            value["plugin"],
            json!({
                "plugin_name": "throwaway",
                "plugin_version": "0.1.0",
                "abi_version": {"major": 2, "minor": 0},
                "host_api_requirement": ">=0.1.0, <0.2.0"
            })
        );
        assert_eq!(
            value["expected_abi_version"],
            json!({"major": 1, "minor": 0})
        );
        assert_eq!(value["compatibility"], json!("too-new"));
    }

    #[test]
    fn renderer_load_status_ignores_unknown_future_fields() {
        let response: RendererLoadStatus = serde_json::from_value(json!({
            "status": "missing-entry",
            "symbol": RENDERER_ENTRY_SYMBOL_V1,
            "future_field": "ignored"
        }))
        .unwrap();

        assert_eq!(
            response,
            RendererLoadStatus::MissingEntry {
                symbol: RENDERER_ENTRY_SYMBOL_V1.to_string(),
            }
        );
    }

    #[test]
    fn descriptor_expected_size_matches_layout() {
        assert_eq!(
            RendererPluginDescriptorV1::expected_size() as usize,
            std::mem::size_of::<RendererPluginDescriptorV1>()
        );
    }
}
