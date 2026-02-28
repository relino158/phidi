use phidi_rpc::plugin::{VoltCapability, VoltMetadata};

pub fn sandbox_capabilities(
    meta: &VoltMetadata,
    granted_capabilities: &[VoltCapability],
) -> Vec<VoltCapability> {
    let mut allowed = Vec::new();
    for capability in VoltCapability::ALL {
        if meta.requests_capability(capability)
            && granted_capabilities.contains(&capability)
        {
            allowed.push(capability);
        }
    }
    allowed
}

pub fn requested_capability_prompt(
    meta: &VoltMetadata,
    capability: VoltCapability,
) -> String {
    format!(
        "Plugin '{}' requests {}. Use '{}' from the plugin menu to grant it, and you can revoke it later.",
        meta.display_name,
        capability.request_summary(),
        capability.action_label(false),
    )
}
