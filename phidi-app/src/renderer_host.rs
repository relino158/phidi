use std::{ffi::CStr, os::raw::c_char, path::Path};

use libloading::Library;
use phidi_rpc::renderer::{
    CURRENT_RENDERER_ABI_VERSION, RENDERER_ENTRY_SYMBOL_V1,
    RendererAbiCompatibility, RendererHostSupport, RendererLoadStatus,
    RendererPluginDescriptorV1, RendererPluginEntryV1, RendererPluginMetadata,
};
use semver::{Version, VersionReq};

pub const CURRENT_RENDERER_HOST_API_VERSION: &str = env!("CARGO_PKG_VERSION");

const BUILTIN_RENDERER_NAME: &[u8] = b"phidi-default-renderer\0";
const BUILTIN_RENDERER_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
const BUILTIN_RENDERER_HOST_API_REQUIREMENT: &str =
    concat!("=", env!("CARGO_PKG_VERSION"), "\0");

static BUILTIN_DEFAULT_RENDERER_DESCRIPTOR: RendererPluginDescriptorV1 =
    RendererPluginDescriptorV1 {
        struct_size: RendererPluginDescriptorV1::expected_size(),
        abi_version: CURRENT_RENDERER_ABI_VERSION,
        plugin_name: BUILTIN_RENDERER_NAME.as_ptr() as *const c_char,
        plugin_version: BUILTIN_RENDERER_VERSION.as_ptr() as *const c_char,
        host_api_requirement: BUILTIN_RENDERER_HOST_API_REQUIREMENT.as_ptr()
            as *const c_char,
    };

pub fn probe_renderer_plugin(
    plugin_library: &Path,
    host_api_version: &str,
) -> RendererLoadStatus {
    let (host_support, parsed_host_api_version) =
        match parse_host_support(host_api_version) {
            Ok(host_support) => host_support,
            Err(status) => return *status,
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
    validate_renderer_descriptor(descriptor, &host_support, &parsed_host_api_version)
}

pub fn probe_renderer_descriptor(
    descriptor: &RendererPluginDescriptorV1,
    host_api_version: &str,
) -> RendererLoadStatus {
    let (host_support, parsed_host_api_version) =
        match parse_host_support(host_api_version) {
            Ok(host_support) => host_support,
            Err(status) => return *status,
        };

    validate_renderer_descriptor(descriptor, &host_support, &parsed_host_api_version)
}

pub fn builtin_default_renderer_descriptor() -> &'static RendererPluginDescriptorV1 {
    &BUILTIN_DEFAULT_RENDERER_DESCRIPTOR
}

pub fn probe_builtin_default_renderer() -> RendererLoadStatus {
    probe_renderer_descriptor(
        builtin_default_renderer_descriptor(),
        CURRENT_RENDERER_HOST_API_VERSION,
    )
}

fn parse_host_support(
    host_api_version: &str,
) -> Result<(RendererHostSupport, Version), Box<RendererLoadStatus>> {
    let parsed_host_api_version = match Version::parse(host_api_version) {
        Ok(version) => version,
        Err(err) => {
            return Err(Box::new(RendererLoadStatus::InvalidHostApiVersion {
                host_api_version: host_api_version.to_string(),
                message: err.to_string(),
            }));
        }
    };

    Ok((
        RendererHostSupport::current_build(host_api_version),
        parsed_host_api_version,
    ))
}

fn validate_renderer_descriptor(
    descriptor: &RendererPluginDescriptorV1,
    host_support: &RendererHostSupport,
    parsed_host_api_version: &Version,
) -> RendererLoadStatus {
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
            host_support: host_support.clone(),
            compatibility,
        };
    }

    let requirement = match VersionReq::parse(&plugin.host_api_requirement) {
        Ok(requirement) => requirement,
        Err(err) => {
            return RendererLoadStatus::InvalidHostApiRequirement {
                plugin,
                host_support: host_support.clone(),
                message: err.to_string(),
            };
        }
    };

    if !requirement.matches(parsed_host_api_version) {
        return RendererLoadStatus::HostApiMismatch {
            plugin,
            host_support: host_support.clone(),
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
        RendererAbiCompatibility, RendererHostSupport, RendererLoadStatus,
    };

    use super::{
        CURRENT_RENDERER_HOST_API_VERSION, builtin_default_renderer_descriptor,
        probe_builtin_default_renderer, probe_renderer_descriptor,
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
                host_support: RendererHostSupport::current_build("0.1.3"),
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
                host_support: RendererHostSupport::current_build("0.1.3"),
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

    #[test]
    fn builtin_default_renderer_uses_same_descriptor_contract_path() {
        let status = probe_builtin_default_renderer();

        assert_eq!(
            status,
            RendererLoadStatus::Ready {
                plugin: phidi_rpc::renderer::RendererPluginMetadata {
                    plugin_name: "phidi-default-renderer".to_string(),
                    plugin_version: CURRENT_RENDERER_HOST_API_VERSION.to_string(),
                    abi_version: CURRENT_RENDERER_ABI_VERSION,
                    host_api_requirement: format!(
                        "={CURRENT_RENDERER_HOST_API_VERSION}"
                    ),
                },
            }
        );
    }

    #[test]
    fn builtin_descriptor_reports_invalid_host_version_actionably() {
        let status = probe_renderer_descriptor(
            builtin_default_renderer_descriptor(),
            "not-semver",
        );

        assert_eq!(
            status,
            RendererLoadStatus::InvalidHostApiVersion {
                host_api_version: "not-semver".to_string(),
                message:
                    "unexpected character 'n' while parsing major version number"
                        .to_string(),
            }
        );
        assert!(
            status
                .actionable_message()
                .contains("Fix the host version string")
        );
    }
}
