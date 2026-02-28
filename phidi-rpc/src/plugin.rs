use core::fmt;
use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::counter::Counter;

#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PluginId(pub u64);

impl PluginId {
    pub fn next() -> Self {
        static PLUGIN_ID_COUNTER: Counter = Counter::new();
        Self(PLUGIN_ID_COUNTER.next())
    }
}

#[derive(Deserialize, Clone, Debug, Serialize)]
pub struct PluginConfiguration {
    #[serde(rename(deserialize = "type"))]
    pub kind: String,
    pub default: Value,
    pub description: String,
}

#[derive(Deserialize, Clone, Debug, Serialize, PartialEq, Eq)]
pub struct VoltInfo {
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub author: String,
    pub description: String,
    pub repository: Option<String>,
    pub wasm: bool,
    pub updated_at_ts: i64,
}

impl VoltInfo {
    pub fn id(&self) -> VoltID {
        VoltID::from(self)
    }
}

#[derive(Deserialize, Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct VoltActivation {
    pub language: Option<Vec<String>>,
    pub workspace_contains: Option<Vec<String>>,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum VoltCapability {
    Network,
    ProcessSpawn,
}

impl VoltCapability {
    pub const ALL: [Self; 2] = [Self::Network, Self::ProcessSpawn];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Network => "Network Access",
            Self::ProcessSpawn => "Process Spawn",
        }
    }

    pub const fn action_label(self, granted: bool) -> &'static str {
        match (self, granted) {
            (Self::Network, false) => "Allow Network Access",
            (Self::Network, true) => "Revoke Network Access",
            (Self::ProcessSpawn, false) => "Allow Process Spawn",
            (Self::ProcessSpawn, true) => "Revoke Process Spawn",
        }
    }

    pub const fn request_summary(self) -> &'static str {
        match self {
            Self::Network => "network access",
            Self::ProcessSpawn => "process spawn",
        }
    }
}

impl fmt::Display for VoltCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

#[derive(Deserialize, Clone, Debug, Serialize, PartialEq, Eq)]
pub struct VoltConfig {
    pub default: Value,
    pub description: String,
}

#[derive(Deserialize, Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct VoltMetadata {
    pub name: String,
    pub version: String,
    pub display_name: String,
    pub author: String,
    pub description: String,
    pub icon: Option<String>,
    pub repository: Option<String>,
    pub wasm: Option<String>,
    pub capabilities: Option<Vec<VoltCapability>>,
    pub color_themes: Option<Vec<String>>,
    pub icon_themes: Option<Vec<String>>,
    pub dir: Option<PathBuf>,
    pub activation: Option<VoltActivation>,
    pub config: Option<HashMap<String, VoltConfig>>,
}

impl VoltMetadata {
    pub fn id(&self) -> VoltID {
        VoltID::from(self)
    }

    pub fn info(&self) -> VoltInfo {
        VoltInfo {
            name: self.name.clone(),
            version: self.version.clone(),
            display_name: self.display_name.clone(),
            author: self.author.clone(),
            description: self.description.clone(),
            repository: self.repository.clone(),
            wasm: self.wasm.is_some(),
            updated_at_ts: 0,
        }
    }

    pub fn requests_capability(&self, capability: VoltCapability) -> bool {
        self.capabilities
            .as_ref()
            .is_some_and(|caps| caps.contains(&capability))
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoltID {
    pub author: String,
    pub name: String,
}

impl fmt::Display for VoltID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.author, self.name)
    }
}

impl From<VoltMetadata> for VoltID {
    fn from(volt: VoltMetadata) -> Self {
        Self {
            author: volt.author,
            name: volt.name,
        }
    }
}

impl From<&VoltMetadata> for VoltID {
    fn from(volt: &VoltMetadata) -> Self {
        Self {
            author: volt.author.clone(),
            name: volt.name.clone(),
        }
    }
}

impl From<VoltInfo> for VoltID {
    fn from(volt: VoltInfo) -> Self {
        Self {
            author: volt.author,
            name: volt.name,
        }
    }
}

impl From<&VoltInfo> for VoltID {
    fn from(volt: &VoltInfo) -> Self {
        Self {
            author: volt.author.clone(),
            name: volt.name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{VoltCapability, VoltID, VoltInfo, VoltMetadata};

    #[test]
    fn test_volt_metadata_id() {
        let volt_metadata = VoltMetadata {
            name: "plugin".to_string(),
            version: "0.1".to_string(),
            display_name: "Plugin".to_string(),
            author: "Author".to_string(),
            description: "Useful plugin".to_string(),
            icon: None,
            repository: None,
            wasm: None,
            capabilities: None,
            color_themes: None,
            icon_themes: None,
            dir: std::env::current_dir().unwrap().canonicalize().ok(),
            activation: None,
            config: None,
        };
        let volt_id = VoltID {
            author: "Author".to_string(),
            name: "plugin".to_string(),
        };

        assert_eq!(volt_metadata.id(), volt_id);
        assert_eq!(
            <VoltID as From<&VoltMetadata>>::from(&volt_metadata),
            volt_id
        );
        assert_eq!(
            <VoltID as From<VoltMetadata>>::from(volt_metadata.clone()),
            volt_id
        );
        assert_eq!(
            <&VoltMetadata as Into<VoltID>>::into(&volt_metadata),
            volt_id
        );
        assert_eq!(<VoltMetadata as Into<VoltID>>::into(volt_metadata), volt_id);
    }

    #[test]
    fn test_volt_metadata_info() {
        let volt_metadata = VoltMetadata {
            name: "plugin".to_string(),
            version: "0.1".to_string(),
            display_name: "Plugin".to_string(),
            author: "Author".to_string(),
            description: "Useful plugin".to_string(),
            icon: None,
            repository: None,
            wasm: None,
            capabilities: None,
            color_themes: None,
            icon_themes: None,
            dir: std::env::current_dir().unwrap().canonicalize().ok(),
            activation: None,
            config: None,
        };
        let volt_info = VoltInfo {
            name: "plugin".to_string(),
            version: "0.1".to_string(),
            display_name: "Plugin".to_string(),
            author: "Author".to_string(),
            description: "Useful plugin".to_string(),
            repository: None,
            wasm: false,
            updated_at_ts: 0,
        };
        assert_eq!(volt_metadata.info(), volt_info);
    }

    #[test]
    fn test_volt_info_id() {
        let volt_info = VoltInfo {
            name: "plugin".to_string(),
            version: "0.1".to_string(),
            display_name: "Plugin".to_string(),
            author: "Author".to_string(),
            description: "Useful plugin".to_string(),
            repository: None,
            wasm: false,
            updated_at_ts: 0,
        };
        let volt_id = VoltID {
            author: "Author".to_string(),
            name: "plugin".to_string(),
        };
        assert_eq!(volt_info.id(), volt_id);
        assert_eq!(<VoltID as From<&VoltInfo>>::from(&volt_info), volt_id);
        assert_eq!(<VoltID as From<VoltInfo>>::from(volt_info.clone()), volt_id);
        assert_eq!(<&VoltInfo as Into<VoltID>>::into(&volt_info), volt_id);
        assert_eq!(<VoltInfo as Into<VoltID>>::into(volt_info), volt_id);
    }

    #[test]
    fn test_volt_metadata_requested_capabilities() {
        let volt_metadata = VoltMetadata {
            name: "plugin".to_string(),
            version: "0.1".to_string(),
            display_name: "Plugin".to_string(),
            author: "Author".to_string(),
            description: "Useful plugin".to_string(),
            icon: None,
            repository: None,
            wasm: Some("/tmp/plugin.wasm".to_string()),
            capabilities: Some(vec![VoltCapability::Network]),
            color_themes: None,
            icon_themes: None,
            dir: None,
            activation: None,
            config: None,
        };

        assert!(volt_metadata.requests_capability(VoltCapability::Network));
        assert!(!volt_metadata.requests_capability(VoltCapability::ProcessSpawn));
    }
}
