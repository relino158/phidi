use std::{ffi::CStr, path::Path};

use libloading::Library;
use phidi_rpc::renderer::{
    CURRENT_RENDERER_ABI_VERSION, RENDERER_ENTRY_SYMBOL_V1,
    RendererAbiCompatibility, RendererLoadStatus, RendererPluginDescriptorV1,
    RendererPluginEntryV1, RendererPluginMetadata,
};
use semver::{Version, VersionReq};

pub fn probe_renderer_plugin(
    plugin_library: &Path,
    host_api_version: &str,
) -> RendererLoadStatus {
    let host_api_version = match Version::parse(host_api_version) {
        Ok(version) => version,
        Err(err) => {
            return RendererLoadStatus::InvalidHostApiVersion {
                host_api_version: host_api_version.to_string(),
                message: err.to_string(),
            };
        }
    };

    let library = match unsafe { Library::new(plugin_library) } {
        Ok(library) => library,
        Err(err) => {
            return RendererLoadStatus::LoadFailure {
                message: format!(
                    "failed to load '{}': {err}",
                    plugin_library.display()
                ),
            };
        }
    };

    let entry = unsafe {
        match library
            .get::<RendererPluginEntryV1>(RENDERER_ENTRY_SYMBOL_V1.as_bytes())
        {
            Ok(symbol) => symbol,
            Err(_) => {
                return RendererLoadStatus::MissingEntry {
                    symbol: RENDERER_ENTRY_SYMBOL_V1.to_string(),
                };
            }
        }
    };

    let descriptor = unsafe { entry() };
    if descriptor.is_null() {
        return RendererLoadStatus::NullDescriptor {
            symbol: RENDERER_ENTRY_SYMBOL_V1.to_string(),
        };
    }

    let descriptor = unsafe { &*descriptor };
    if descriptor.struct_size != RendererPluginDescriptorV1::expected_size() {
        return RendererLoadStatus::InvalidDescriptor {
            message: format!(
                "descriptor size {} did not match expected {}",
                descriptor.struct_size,
                RendererPluginDescriptorV1::expected_size()
            ),
        };
    }

    let plugin = match read_metadata(descriptor) {
        Ok(plugin) => plugin,
        Err(message) => {
            return RendererLoadStatus::InvalidDescriptor { message };
        }
    };

    let compatibility = plugin.abi_version.compatibility_with_current();
    if !matches!(
        compatibility,
        RendererAbiCompatibility::Current | RendererAbiCompatibility::Compatible
    ) {
        return RendererLoadStatus::AbiMismatch {
            plugin,
            expected_abi_version: CURRENT_RENDERER_ABI_VERSION,
            compatibility,
        };
    }

    let requirement = match VersionReq::parse(&plugin.host_api_requirement) {
        Ok(requirement) => requirement,
        Err(err) => {
            return RendererLoadStatus::InvalidHostApiRequirement {
                plugin,
                message: err.to_string(),
            };
        }
    };

    if !requirement.matches(&host_api_version) {
        return RendererLoadStatus::HostApiMismatch {
            plugin,
            host_api_version: host_api_version.to_string(),
        };
    }

    RendererLoadStatus::Ready { plugin }
}

fn read_metadata(
    descriptor: &RendererPluginDescriptorV1,
) -> Result<RendererPluginMetadata, String> {
    Ok(RendererPluginMetadata {
        plugin_name: read_c_string(descriptor.plugin_name, "plugin_name")?,
        plugin_version: read_c_string(descriptor.plugin_version, "plugin_version")?,
        abi_version: descriptor.abi_version,
        host_api_requirement: read_c_string(
            descriptor.host_api_requirement,
            "host_api_requirement",
        )?,
    })
}

fn read_c_string(
    value: *const std::ffi::c_char,
    field_name: &str,
) -> Result<String, String> {
    if value.is_null() {
        return Err(format!("{field_name} pointer was null"));
    }

    unsafe { CStr::from_ptr(value) }
        .to_str()
        .map(|value| value.to_string())
        .map_err(|err| format!("{field_name} was not valid UTF-8: {err}"))
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        process::Command,
        sync::{Mutex, OnceLock},
    };

    use super::probe_renderer_plugin;
    use phidi_rpc::renderer::{
        CURRENT_RENDERER_ABI_VERSION, RENDERER_ENTRY_SYMBOL_V1,
        RendererAbiCompatibility, RendererLoadStatus,
    };

    fn ready_fixture() -> &'static PathBuf {
        static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
        FIXTURE.get_or_init(|| build_fixture("renderer_probe_ready"))
    }

    fn abi_mismatch_fixture() -> &'static PathBuf {
        static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
        FIXTURE.get_or_init(|| build_fixture("renderer_probe_abi_mismatch"))
    }

    fn host_api_mismatch_fixture() -> &'static PathBuf {
        static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
        FIXTURE.get_or_init(|| build_fixture("renderer_probe_host_api_mismatch"))
    }

    fn missing_entry_fixture() -> &'static PathBuf {
        static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
        FIXTURE.get_or_init(|| build_fixture("renderer_probe_missing_entry"))
    }

    fn build_fixture(package_name: &str) -> PathBuf {
        static BUILD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

        let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/renderers")
            .join(package_name);
        let target_dir = std::env::temp_dir().join("phidi-renderer-fixtures");
        let _build_guard = BUILD_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let output = Command::new("cargo")
            .arg("build")
            .arg("--manifest-path")
            .arg(fixture_root.join("Cargo.toml"))
            .arg("--target-dir")
            .arg(&target_dir)
            .arg("--quiet")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "failed to build fixture {package_name}: stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        target_dir.join("debug").join(format!(
            "{}{}.{}",
            dynamic_library_prefix(),
            package_name,
            std::env::consts::DLL_EXTENSION
        ))
    }

    fn dynamic_library_prefix() -> &'static str {
        if cfg!(target_os = "windows") {
            ""
        } else {
            "lib"
        }
    }

    #[test]
    fn probe_renderer_plugin_reports_ready_for_compatible_descriptor() {
        let status = probe_renderer_plugin(ready_fixture(), "0.1.3");

        assert_eq!(
            status,
            RendererLoadStatus::Ready {
                plugin: phidi_rpc::renderer::RendererPluginMetadata {
                    plugin_name: "throwaway-ready".to_string(),
                    plugin_version: "0.1.0".to_string(),
                    abi_version: CURRENT_RENDERER_ABI_VERSION,
                    host_api_requirement: ">=0.1.0, <0.2.0".to_string(),
                },
            }
        );
    }

    #[test]
    fn probe_renderer_plugin_rejects_newer_abi_versions() {
        let status = probe_renderer_plugin(abi_mismatch_fixture(), "0.1.3");

        assert_eq!(
            status,
            RendererLoadStatus::AbiMismatch {
                plugin: phidi_rpc::renderer::RendererPluginMetadata {
                    plugin_name: "throwaway-abi-mismatch".to_string(),
                    plugin_version: "0.1.0".to_string(),
                    abi_version: phidi_rpc::renderer::RendererAbiVersion::new(2, 0,),
                    host_api_requirement: ">=0.1.0, <0.2.0".to_string(),
                },
                expected_abi_version: CURRENT_RENDERER_ABI_VERSION,
                compatibility: RendererAbiCompatibility::TooNew,
            }
        );
    }

    #[test]
    fn probe_renderer_plugin_rejects_incompatible_host_api_requirements() {
        let status = probe_renderer_plugin(host_api_mismatch_fixture(), "0.1.3");

        assert_eq!(
            status,
            RendererLoadStatus::HostApiMismatch {
                plugin: phidi_rpc::renderer::RendererPluginMetadata {
                    plugin_name: "throwaway-host-api-mismatch".to_string(),
                    plugin_version: "0.1.0".to_string(),
                    abi_version: CURRENT_RENDERER_ABI_VERSION,
                    host_api_requirement: ">=0.2.0, <0.3.0".to_string(),
                },
                host_api_version: "0.1.3".to_string(),
            }
        );
    }

    #[test]
    fn probe_renderer_plugin_reports_missing_entry_symbol() {
        let status = probe_renderer_plugin(missing_entry_fixture(), "0.1.3");

        assert_eq!(
            status,
            RendererLoadStatus::MissingEntry {
                symbol: RENDERER_ENTRY_SYMBOL_V1.to_string(),
            }
        );
    }
}
