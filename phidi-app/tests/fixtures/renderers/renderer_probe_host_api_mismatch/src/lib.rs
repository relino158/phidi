use std::ffi::c_char;

use phidi_rpc::renderer::{
    CURRENT_RENDERER_ABI_VERSION, RendererPluginDescriptorV1,
};

static PLUGIN_NAME: &[u8] = b"throwaway-host-api-mismatch\0";
static PLUGIN_VERSION: &[u8] = b"0.1.0\0";
static HOST_API_REQUIREMENT: &[u8] = b">=0.2.0, <0.3.0\0";

static DESCRIPTOR: RendererPluginDescriptorV1 = RendererPluginDescriptorV1 {
    struct_size: RendererPluginDescriptorV1::expected_size(),
    abi_version: CURRENT_RENDERER_ABI_VERSION,
    plugin_name: PLUGIN_NAME.as_ptr() as *const c_char,
    plugin_version: PLUGIN_VERSION.as_ptr() as *const c_char,
    host_api_requirement: HOST_API_REQUIREMENT.as_ptr() as *const c_char,
};

#[unsafe(no_mangle)]
pub extern "C" fn phidi_renderer_descriptor_v1() -> *const RendererPluginDescriptorV1 {
    &DESCRIPTOR
}
