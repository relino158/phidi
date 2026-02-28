use std::{fs, path::Path};

use phidi_core::semantic_map::{
    EntityId, EntityKind, EntityLocation, SnapshotKind, SnapshotProvenance,
    TextPoint, TextSpan, WorkspaceSnapshot,
};
use phidi_proxy::query::SnapshotQueryService;
use phidi_rpc::agent::{CapabilityResponse, EntitySelector, RenamePlanningRequest};
use tempfile::tempdir;

#[test]
fn rename_planning_splits_preview_into_confidence_buckets_and_keeps_files_unchanged()
{
    let fixture = WorkspaceFixture::new();
    fixture.write("src/ui/render.rs", "pub fn render() {}\n");
    fixture.write("src/graphics/render.rs", "pub fn render() {}\n");
    fixture.write(
        "src/ui/controller.rs",
        "pub fn refresh() {\n    crate::ui::render();\n    render();\n}\n",
    );
    fixture.write("src/app.rs", "pub fn refresh() {\n    render();\n}\n");

    let snapshot = function_snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let before_controller = fixture.read("src/ui/controller.rs");
    let before_render = fixture.read("src/ui/render.rs");
    let before_app = fixture.read("src/app.rs");

    let response = service.rename_planning(
        fixture.path(),
        RenamePlanningRequest {
            entity: EntitySelector::QualifiedName {
                qualified_name: "crate::ui::render".to_string(),
            },
            new_name: "draw".to_string(),
        },
    );

    let CapabilityResponse::Success { result } = response else {
        panic!("expected rename planning success");
    };

    assert_eq!(result.high_confidence_edits.len(), 2);
    assert_eq!(
        edit_paths(&result.high_confidence_edits),
        vec!["src/ui/controller.rs", "src/ui/render.rs"]
    );
    assert!(result.high_confidence_edits.iter().any(|edit| {
        edit.location.path == "src/ui/controller.rs"
            && edit.reason.as_deref()
                == Some("explicit path uniquely identifies the target")
    }));
    assert!(result.high_confidence_edits.iter().any(|edit| {
        edit.location.path == "src/ui/render.rs"
            && edit.reason.as_deref() == Some("rename target definition")
    }));

    assert_eq!(result.low_confidence_edits.len(), 1);
    assert_eq!(
        edit_paths(&result.low_confidence_edits),
        vec!["src/ui/controller.rs"]
    );
    assert_eq!(
        result.low_confidence_edits[0].reason.as_deref(),
        Some("locality breaks the tie, but the syntax has no explicit path")
    );

    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(
        result.conflicts[0]
            .location
            .as_ref()
            .map(|location| location.path.as_str()),
        Some("src/app.rs")
    );
    assert_eq!(
        result.conflicts[0].message,
        "top-ranked candidates are tied on syntax-only evidence"
    );

    assert_eq!(fixture.read("src/ui/controller.rs"), before_controller);
    assert_eq!(fixture.read("src/ui/render.rs"), before_render);
    assert_eq!(fixture.read("src/app.rs"), before_app);
}

#[test]
fn rename_planning_surfaces_same_name_method_calls_as_conflicts() {
    let fixture = WorkspaceFixture::new();
    fixture.write(
        "src/ui/widget.rs",
        "pub struct Widget;\nimpl Widget {\n    pub fn render(&self) {}\n}\n",
    );
    fixture.write(
        "src/ui/overlay.rs",
        "pub struct Overlay;\nimpl Overlay {\n    pub fn render(&self) {}\n}\n",
    );
    fixture.write(
        "src/ui/controller.rs",
        "pub fn refresh(widget: &crate::ui::Widget, overlay: &crate::ui::Overlay) {\n    widget.render();\n    overlay.render();\n}\n",
    );

    let snapshot = method_snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.rename_planning(
        fixture.path(),
        RenamePlanningRequest {
            entity: EntitySelector::QualifiedName {
                qualified_name: "crate::ui::Widget::render".to_string(),
            },
            new_name: "draw".to_string(),
        },
    );

    let CapabilityResponse::Success { result } = response else {
        panic!("expected rename planning success");
    };

    assert_eq!(
        edit_paths(&result.high_confidence_edits),
        vec!["src/ui/widget.rs"]
    );
    assert!(result.low_confidence_edits.is_empty());
    assert_eq!(result.conflicts.len(), 2);
    assert!(result.conflicts.iter().all(|conflict| {
        conflict
            .location
            .as_ref()
            .map(|location| location.path.as_str())
            == Some("src/ui/controller.rs")
    }));
    assert!(result.conflicts.iter().all(|conflict| {
        conflict.message
            == "method receiver types are unavailable in syntax-only ranking"
    }));
}

#[test]
fn rename_planning_surfaces_parse_failures_as_conflicts() {
    let fixture = WorkspaceFixture::new();
    fixture.write("src/ui/render.rs", "pub fn render() {}\n");
    fixture.write("src/graphics/render.rs", "pub fn render() {}\n");
    fixture.write("src/broken.rs", "pub fn refresh( {\n    render();\n}\n");

    let snapshot = parse_failure_snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.rename_planning(
        fixture.path(),
        RenamePlanningRequest {
            entity: EntitySelector::QualifiedName {
                qualified_name: "crate::ui::render".to_string(),
            },
            new_name: "draw".to_string(),
        },
    );

    let CapabilityResponse::Success { result } = response else {
        panic!("expected rename planning success");
    };

    assert_eq!(
        edit_paths(&result.high_confidence_edits),
        vec!["src/ui/render.rs"]
    );
    assert!(result.low_confidence_edits.is_empty());
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(
        result.conflicts[0]
            .location
            .as_ref()
            .map(|location| location.path.as_str()),
        Some("src/broken.rs")
    );
    assert!(
        result.conflicts[0].message.contains(
            "failed to parse rename preview candidates from src/broken.rs"
        )
    );
}

fn function_snapshot_fixture() -> WorkspaceSnapshot {
    let mut snapshot = base_snapshot();
    snapshot.entities = vec![
        function_entity(
            "function:src/ui/render.rs:render",
            "render",
            "crate::ui::render",
            "src/ui/render.rs",
            0,
            7,
            0,
            13,
        ),
        function_entity(
            "function:src/graphics/render.rs:render",
            "render",
            "crate::graphics::render",
            "src/graphics/render.rs",
            0,
            7,
            0,
            13,
        ),
        function_entity(
            "function:src/ui/controller.rs:refresh",
            "refresh",
            "crate::ui::controller::refresh",
            "src/ui/controller.rs",
            0,
            7,
            2,
            1,
        ),
        function_entity(
            "function:src/app.rs:refresh",
            "refresh",
            "crate::app::refresh",
            "src/app.rs",
            0,
            7,
            2,
            1,
        ),
    ];
    snapshot
}

fn method_snapshot_fixture() -> WorkspaceSnapshot {
    let mut snapshot = base_snapshot();
    snapshot.entities = vec![
        method_entity(
            "method:src/ui/widget.rs:Widget::render",
            "render",
            "crate::ui::Widget::render",
            "src/ui/widget.rs",
            2,
            11,
            2,
            17,
        ),
        method_entity(
            "method:src/ui/overlay.rs:Overlay::render",
            "render",
            "crate::ui::Overlay::render",
            "src/ui/overlay.rs",
            2,
            11,
            2,
            17,
        ),
        function_entity(
            "function:src/ui/controller.rs:refresh",
            "refresh",
            "crate::ui::controller::refresh",
            "src/ui/controller.rs",
            0,
            7,
            3,
            1,
        ),
    ];
    snapshot
}

fn parse_failure_snapshot_fixture() -> WorkspaceSnapshot {
    let mut snapshot = base_snapshot();
    snapshot.entities = vec![
        function_entity(
            "function:src/ui/render.rs:render",
            "render",
            "crate::ui::render",
            "src/ui/render.rs",
            0,
            7,
            0,
            13,
        ),
        function_entity(
            "function:src/graphics/render.rs:render",
            "render",
            "crate::graphics::render",
            "src/graphics/render.rs",
            0,
            7,
            0,
            13,
        ),
        function_entity(
            "function:src/broken.rs:refresh",
            "refresh",
            "crate::broken::refresh",
            "src/broken.rs",
            0,
            7,
            2,
            1,
        ),
    ];
    snapshot
}

fn base_snapshot() -> WorkspaceSnapshot {
    WorkspaceSnapshot::new(
        SnapshotKind::Working,
        SnapshotProvenance {
            revision: Some("abc123".to_string()),
            has_uncommitted_changes: false,
        },
    )
}

fn function_entity(
    id: &str,
    name: &str,
    qualified_name: &str,
    path: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) -> phidi_core::semantic_map::SemanticEntity {
    entity(
        id,
        EntityKind::Function,
        name,
        qualified_name,
        path,
        start_line,
        start_column,
        end_line,
        end_column,
    )
}

fn method_entity(
    id: &str,
    name: &str,
    qualified_name: &str,
    path: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) -> phidi_core::semantic_map::SemanticEntity {
    entity(
        id,
        EntityKind::Method,
        name,
        qualified_name,
        path,
        start_line,
        start_column,
        end_line,
        end_column,
    )
}

fn entity(
    id: &str,
    kind: EntityKind,
    name: &str,
    qualified_name: &str,
    path: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) -> phidi_core::semantic_map::SemanticEntity {
    phidi_core::semantic_map::SemanticEntity {
        id: EntityId(id.to_string()),
        kind,
        name: name.to_string(),
        qualified_name: Some(qualified_name.to_string()),
        location: Some(EntityLocation {
            path: path.to_string(),
            span: Some(TextSpan {
                start: TextPoint {
                    line: start_line,
                    column: start_column,
                },
                end: TextPoint {
                    line: end_line,
                    column: end_column,
                },
            }),
        }),
    }
}

fn edit_paths(edits: &[phidi_rpc::agent::RenameEdit]) -> Vec<&str> {
    edits
        .iter()
        .map(|edit| edit.location.path.as_str())
        .collect::<Vec<_>>()
}

struct WorkspaceFixture {
    tempdir: tempfile::TempDir,
}

impl WorkspaceFixture {
    fn new() -> Self {
        Self {
            tempdir: tempdir().unwrap(),
        }
    }

    fn path(&self) -> &Path {
        self.tempdir.path()
    }

    fn write(&self, relative_path: &str, contents: &str) {
        let path = self.path().join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn read(&self, relative_path: &str) -> String {
        fs::read_to_string(self.path().join(relative_path)).unwrap()
    }
}
