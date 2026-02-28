use std::collections::HashMap;

use phidi_rpc::plugin::{VoltCapability, VoltMetadata};
use serde_json::{Value, json};

use super::{load_volt, unflatten_map};
use crate::plugin::capabilities::{
    requested_capability_prompt, sandbox_capabilities,
};

#[test]
fn test_unflatten_map() {
    let map: HashMap<String, Value> = serde_json::from_value(json!({
        "a.b.c": "d",
        "a.d": ["e"],
    }))
    .unwrap();
    assert_eq!(
        unflatten_map(&map),
        json!({
            "a": {
                "b": {
                    "c": "d",
                },
                "d": ["e"],
            }
        })
    );
}

#[test]
fn test_load_volt() {
    let phidi_proxy_dir = std::env::current_dir()
        .expect("Can't get \"phidi-proxy\" directory")
        .join("src")
        .join("plugin")
        .join("wasi")
        .join("plugins");

    // Invalid path (file does not exist)
    let path = phidi_proxy_dir.join("some-path");
    match path.canonicalize() {
        Ok(path) => panic!("{path:?} file must not exast, but it is"),
        Err(err) => assert_eq!(err.kind(), std::io::ErrorKind::NotFound),
    };
    // This should return Err since the file does not exist
    if let Ok(volt_metadata) = load_volt(&phidi_proxy_dir) {
        panic!(
            "Unexpected result from `phidi_proxy::plugin::wasi::load_volt` function: {volt_metadata:?}"
        );
    }

    // Invalid file (not readable into a string)
    // Making sure the file exists
    let path = phidi_proxy_dir.join("smiley.png");
    let path = match path.canonicalize() {
        Ok(path) => path,
        Err(err) => panic!("{path:?} file must exast, but: {err:?}"),
    };
    // Making sure the data in the file is invalid utf-8
    match std::fs::read_to_string(path.clone()) {
        Ok(str) => panic!(
            "{path:?} file must be invalid utf-8, but it is valid utf-8: {str:?}",
        ),
        Err(err) => assert_eq!(err.kind(), std::io::ErrorKind::InvalidData),
    }
    // This should return Err since the `*.png` file cannot be read as a String
    if let Ok(volt_metadata) = load_volt(&path) {
        panic!(
            "Unexpected result from `phidi_proxy::plugin::wasi::load_volt` function: {volt_metadata:?}",
        );
    }

    // Invalid data in file (cannot be read as VoltMetadata)
    // Making sure the file exists
    let path = phidi_proxy_dir
        .join("some_author.test-plugin-one")
        .join("Light.svg");
    let path = match path.canonicalize() {
        Ok(path) => path,
        Err(err) => panic!("{path:?} file must exast, but: {err:?}"),
    };
    // Making sure the data in the file is valid utf-8 (*.svg file is must be a valid utf-8)
    match std::fs::read_to_string(path.clone()) {
        Ok(_) => {}
        Err(err) => panic!("{path:?} file must be valid utf-8, but {err:?}"),
    }
    // This should return Err since the data in the file cannot be interpreted as VoltMetadata
    if let Ok(volt_metadata) = load_volt(&path) {
        panic!(
            "Unexpected result from `phidi_proxy::plugin::wasi::load_volt` function: {volt_metadata:?}",
        );
    }

    let parent_path = phidi_proxy_dir.join("some_author.test-plugin-one");

    let volt_metadata = match load_volt(&parent_path) {
        Ok(volt_metadata) => volt_metadata,
        Err(error) => panic!("{}", error),
    };

    let wasm_path = parent_path
        .join("phidi.wasm")
        .canonicalize()
        .ok()
        .as_ref()
        .and_then(|path| path.to_str())
        .map(ToOwned::to_owned);

    let color_themes_pathes = ["Dark.toml", "Light.toml"]
        .into_iter()
        .filter_map(|theme| {
            parent_path
                .join(theme)
                .canonicalize()
                .ok()
                .as_ref()
                .and_then(|path| path.to_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();

    let icon_themes_pathes = ["Dark.svg", "Light.svg"]
        .into_iter()
        .filter_map(|theme| {
            parent_path
                .join(theme)
                .canonicalize()
                .ok()
                .as_ref()
                .and_then(|path| path.to_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();

    assert_eq!(
        volt_metadata,
        VoltMetadata {
            name: "some-useful-plugin".to_string(),
            version: "0.1.56".to_string(),
            display_name: "Some Useful Plugin Name".to_string(),
            author: "some_author".to_string(),
            description: "very useful plugin".to_string(),
            icon: Some("icon.svg".to_string()),
            repository: Some("https://github.com/phidi".to_string()),
            wasm: wasm_path,
            capabilities: Some(vec![
                VoltCapability::Network,
                VoltCapability::ProcessSpawn,
            ]),
            color_themes: Some(color_themes_pathes),
            icon_themes: Some(icon_themes_pathes),
            dir: parent_path.canonicalize().ok(),
            activation: None,
            config: None
        }
    );

    let parent_path = phidi_proxy_dir.join("some_author.test-plugin-two");

    let volt_metadata = match load_volt(&parent_path) {
        Ok(volt_metadata) => volt_metadata,
        Err(error) => panic!("{}", error),
    };

    let wasm_path = parent_path
        .join("phidi.wasm")
        .canonicalize()
        .ok()
        .as_ref()
        .and_then(|path| path.to_str())
        .map(ToOwned::to_owned);

    let color_themes_pathes = ["Light.toml"]
        .into_iter()
        .filter_map(|theme| {
            parent_path
                .join(theme)
                .canonicalize()
                .ok()
                .as_ref()
                .and_then(|path| path.to_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();

    let icon_themes_pathes = ["Light.svg"]
        .into_iter()
        .filter_map(|theme| {
            parent_path
                .join(theme)
                .canonicalize()
                .ok()
                .as_ref()
                .and_then(|path| path.to_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();

    assert_eq!(
        volt_metadata,
        VoltMetadata {
            name: "some-useful-plugin".to_string(),
            version: "0.1.56".to_string(),
            display_name: "Some Useful Plugin Name".to_string(),
            author: "some_author.".to_string(),
            description: "very useful plugin".to_string(),
            icon: Some("icon.svg".to_string()),
            repository: Some("https://github.com/phidi".to_string()),
            wasm: wasm_path,
            capabilities: None,
            color_themes: Some(color_themes_pathes),
            icon_themes: Some(icon_themes_pathes),
            dir: parent_path.canonicalize().ok(),
            activation: None,
            config: None
        }
    );

    let parent_path = phidi_proxy_dir.join("some_author.test-plugin-three");

    let volt_metadata = match load_volt(&parent_path) {
        Ok(volt_metadata) => volt_metadata,
        Err(error) => panic!("{}", error),
    };

    assert_eq!(
        volt_metadata,
        VoltMetadata {
            name: "some-useful-plugin".to_string(),
            version: "0.1.56".to_string(),
            display_name: "Some Useful Plugin Name".to_string(),
            author: "some_author".to_string(),
            description: "very useful plugin".to_string(),
            icon: Some("icon.svg".to_string()),
            repository: Some("https://github.com/phidi".to_string()),
            wasm: None,
            capabilities: None,
            color_themes: Some(Vec::new()),
            icon_themes: Some(Vec::new()),
            dir: parent_path.canonicalize().ok(),
            activation: None,
            config: None
        }
    );
}

#[test]
fn sandbox_defaults_to_denied_for_ungranted_capabilities() {
    let meta = VoltMetadata {
        name: "sandboxed-plugin".to_string(),
        version: "0.1.0".to_string(),
        display_name: "Sandboxed Plugin".to_string(),
        author: "someone".to_string(),
        description: "needs extra powers".to_string(),
        icon: None,
        repository: None,
        wasm: Some("/tmp/plugin.wasm".to_string()),
        capabilities: Some(vec![
            VoltCapability::Network,
            VoltCapability::ProcessSpawn,
        ]),
        color_themes: None,
        icon_themes: None,
        dir: None,
        activation: None,
        config: None,
    };

    let denied = sandbox_capabilities(&meta, &[]);
    assert_eq!(denied, Vec::<VoltCapability>::new());

    let network_only = sandbox_capabilities(&meta, &[VoltCapability::Network]);
    assert_eq!(network_only, vec![VoltCapability::Network]);
}

#[test]
fn capability_prompts_are_explicit_and_revocable() {
    let meta = VoltMetadata {
        name: "sandboxed-plugin".to_string(),
        version: "0.1.0".to_string(),
        display_name: "Sandboxed Plugin".to_string(),
        author: "someone".to_string(),
        description: "needs extra powers".to_string(),
        icon: None,
        repository: None,
        wasm: Some("/tmp/plugin.wasm".to_string()),
        capabilities: Some(vec![VoltCapability::ProcessSpawn]),
        color_themes: None,
        icon_themes: None,
        dir: None,
        activation: None,
        config: None,
    };

    let prompt = requested_capability_prompt(&meta, VoltCapability::ProcessSpawn);
    assert!(prompt.contains("requests process spawn"));
    assert!(prompt.contains("Allow Process Spawn"));
    assert!(prompt.contains("revoke"));
}
