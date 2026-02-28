use std::{fs, path::Path};

use phidi_core::semantic_map::{
    CertaintyKind, EntityKind, RelationshipKind, SnapshotCompleteness,
};
use phidi_proxy::rust_snapshot::RustSnapshotExtractor;
use tempfile::tempdir;

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

#[test]
fn extracts_representative_rust_entities_and_relationships() {
    let tempdir = tempdir().unwrap();
    write_file(
        &tempdir.path().join("src/lib.rs"),
        r#"
use crate::nested::Widget;
mod nested;

#[derive(Debug)]
struct Container;

trait Runner {
    fn run(&self);
}

impl Runner for Container {
    fn run(&self) {
        helper();
    }
}

fn helper() {}

#[instrument]
fn traced() {}

#[test]
fn smoke_test() {
    helper();
}

make_item!(generated);
"#,
    );
    write_file(
        &tempdir.path().join("src/nested.rs"),
        r#"
pub struct Widget;

impl Widget {
    pub fn build() -> Self {
        Self
    }
}
"#,
    );

    let snapshot = RustSnapshotExtractor::new()
        .extract_workspace(tempdir.path())
        .unwrap();

    assert_eq!(snapshot.completeness, SnapshotCompleteness::Complete);
    assert!(snapshot.diagnostics.is_empty());

    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::Module && entity.name == "nested"
    }));
    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::Import && entity.name == "crate::nested::Widget"
    }));
    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::Struct && entity.name == "Container"
    }));
    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::Trait && entity.name == "Runner"
    }));
    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::ImplBlock
            && entity.qualified_name.as_deref() == Some("Container as Runner")
    }));
    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::Method && entity.name == "run"
    }));
    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::Function && entity.name == "helper"
    }));
    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::Test && entity.name == "smoke_test"
    }));
    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::Macro && entity.name == "make_item"
    }));

    let inferred_call = snapshot
        .relationships
        .iter()
        .find(|relationship| {
            relationship.kind == RelationshipKind::Calls
                && relationship.certainty.kind == CertaintyKind::Inferred
        })
        .expect("expected at least one inferred call relationship");
    assert!(u8::from(inferred_call.certainty.confidence) < 100);

    assert!(snapshot.relationships.iter().any(|relationship| {
        relationship.kind == RelationshipKind::Implements
            && relationship.certainty.kind == CertaintyKind::Observed
    }));
}

#[test]
fn parse_failures_degrade_to_partial_output_per_file() {
    let tempdir = tempdir().unwrap();
    write_file(
        &tempdir.path().join("src/good.rs"),
        r#"
fn helper() {}
"#,
    );
    write_file(
        &tempdir.path().join("src/bad.rs"),
        r#"
fn broken( {
"#,
    );

    let snapshot = RustSnapshotExtractor::new()
        .extract_workspace(tempdir.path())
        .unwrap();

    assert_eq!(snapshot.completeness, SnapshotCompleteness::Partial);
    assert!(snapshot.entities.iter().any(|entity| {
        entity.kind == EntityKind::Function && entity.name == "helper"
    }));
    assert_eq!(snapshot.diagnostics.len(), 1);
    assert!(snapshot.diagnostics[0].message.contains("failed to parse"));
    assert_eq!(
        snapshot.diagnostics[0]
            .location
            .as_ref()
            .map(|location| location.path.as_str()),
        Some("src/bad.rs")
    );
}
