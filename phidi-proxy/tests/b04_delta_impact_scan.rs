use std::{fs, path::Path};

use git2::{Repository, Signature};
use phidi_core::semantic_map::{
    EntityId, EntityKind, EntityLocation, SnapshotKind, SnapshotProvenance,
    TextPoint, TextSpan, WorkspaceSnapshot,
};
use phidi_proxy::query::SnapshotQueryService;
use phidi_rpc::agent::{
    AnalysisCompleteness, CapabilityResponse, DeltaImpactScanRequest, DeltaScope,
};
use tempfile::tempdir;

fn snapshot_fixture() -> WorkspaceSnapshot {
    let mut snapshot = WorkspaceSnapshot::new(
        SnapshotKind::Working,
        SnapshotProvenance {
            revision: Some("abc123".to_string()),
            has_uncommitted_changes: false,
        },
    );

    snapshot.entities = vec![
        entity("file:src/lib.rs", EntityKind::File, "lib.rs", "src/lib.rs"),
        entity(
            "function:src/lib.rs:render_panel",
            EntityKind::Function,
            "render_panel",
            "src/lib.rs",
        ),
        entity(
            "file:src/staged.rs",
            EntityKind::File,
            "staged.rs",
            "src/staged.rs",
        ),
        entity(
            "function:src/staged.rs:staged_only",
            EntityKind::Function,
            "staged_only",
            "src/staged.rs",
        ),
    ];

    snapshot
}

fn entity(
    id: &str,
    kind: EntityKind,
    name: &str,
    path: &str,
) -> phidi_core::semantic_map::SemanticEntity {
    phidi_core::semantic_map::SemanticEntity {
        id: EntityId(id.to_string()),
        kind,
        name: name.to_string(),
        qualified_name: None,
        location: Some(EntityLocation {
            path: path.to_string(),
            span: Some(TextSpan {
                start: TextPoint { line: 0, column: 0 },
                end: TextPoint { line: 3, column: 0 },
            }),
        }),
    }
}

#[test]
fn delta_impact_scan_scans_only_unstaged_changes() {
    let fixture = GitFixture::new();
    fixture.write("src/lib.rs", "pub fn render_panel() -> u8 { 2 }\n");

    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.delta_impact_scan(
        fixture.path(),
        DeltaImpactScanRequest {
            scope: DeltaScope::Unstaged,
        },
    );

    let CapabilityResponse::Success { result } = response else {
        panic!("expected successful delta scan");
    };

    assert_eq!(result.completeness, AnalysisCompleteness::Complete);
    assert_eq!(file_paths(&result), vec!["src/lib.rs"]);
    assert_eq!(
        impacted_entity_names(&result.file_impacts[0]),
        vec!["lib.rs", "render_panel"]
    );
}

#[test]
fn delta_impact_scan_scans_only_staged_changes() {
    let fixture = GitFixture::new();
    fixture.write("src/staged.rs", "pub fn staged_only() -> u8 { 9 }\n");
    fixture.stage("src/staged.rs");

    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.delta_impact_scan(
        fixture.path(),
        DeltaImpactScanRequest {
            scope: DeltaScope::Staged,
        },
    );

    let CapabilityResponse::Success { result } = response else {
        panic!("expected successful delta scan");
    };

    assert_eq!(result.completeness, AnalysisCompleteness::Complete);
    assert_eq!(file_paths(&result), vec!["src/staged.rs"]);
    assert_eq!(
        impacted_entity_names(&result.file_impacts[0]),
        vec!["staged.rs", "staged_only"]
    );
}

#[test]
fn delta_impact_scan_combines_staged_and_unstaged_changes_for_all_scope() {
    let fixture = GitFixture::new();
    fixture.write("src/staged.rs", "pub fn staged_only() -> u8 { 9 }\n");
    fixture.stage("src/staged.rs");
    fixture.write("src/lib.rs", "pub fn render_panel() -> u8 { 2 }\n");

    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.delta_impact_scan(
        fixture.path(),
        DeltaImpactScanRequest {
            scope: DeltaScope::All,
        },
    );

    let CapabilityResponse::Success { result } = response else {
        panic!("expected successful delta scan");
    };

    assert_eq!(result.completeness, AnalysisCompleteness::Complete);
    assert_eq!(file_paths(&result), vec!["src/lib.rs", "src/staged.rs"]);
}

#[test]
fn delta_impact_scan_marks_partial_when_a_changed_file_has_no_snapshot_entities() {
    let fixture = GitFixture::new();
    fixture.write("src/new_file.rs", "pub fn new_file() -> u8 { 7 }\n");

    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.delta_impact_scan(
        fixture.path(),
        DeltaImpactScanRequest {
            scope: DeltaScope::Unstaged,
        },
    );

    let CapabilityResponse::Success { result } = response else {
        panic!("expected successful delta scan");
    };

    assert_eq!(result.completeness, AnalysisCompleteness::Partial);
    assert_eq!(file_paths(&result), vec!["src/new_file.rs"]);
    assert!(result.file_impacts[0].impacted_entities.is_empty());
}

fn file_paths(result: &phidi_rpc::agent::DeltaImpactScanResult) -> Vec<&str> {
    result
        .file_impacts
        .iter()
        .map(|impact| impact.path.as_str())
        .collect()
}

fn impacted_entity_names(file_impact: &phidi_rpc::agent::FileImpact) -> Vec<&str> {
    file_impact
        .impacted_entities
        .iter()
        .map(|impact| impact.entity.name.as_str())
        .collect()
}

struct GitFixture {
    tempdir: tempfile::TempDir,
    repo: Repository,
}

impl GitFixture {
    fn new() -> Self {
        let tempdir = tempdir().unwrap();
        let repo = Repository::init(tempdir.path()).unwrap();
        let fixture = Self { tempdir, repo };
        fixture.write("src/lib.rs", "pub fn render_panel() -> u8 { 1 }\n");
        fixture.write("src/staged.rs", "pub fn staged_only() -> u8 { 5 }\n");
        fixture.commit_all("initial");
        fixture
    }

    fn path(&self) -> &Path {
        self.tempdir.path()
    }

    fn write(&self, relative_path: &str, contents: &str) {
        let path = self.path().join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn stage(&self, relative_path: &str) {
        let mut index = self.repo.index().unwrap();
        index.add_path(Path::new(relative_path)).unwrap();
        index.write().unwrap();
    }

    fn commit_all(&self, message: &str) {
        let mut index = self.repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();
        let signature = Signature::now("Phidi Tests", "tests@phidi.dev").unwrap();
        let parent = self
            .repo
            .head()
            .ok()
            .and_then(|head| head.target())
            .and_then(|oid| self.repo.find_commit(oid).ok());

        match parent {
            Some(parent) => {
                self.repo
                    .commit(
                        Some("HEAD"),
                        &signature,
                        &signature,
                        message,
                        &tree,
                        &[&parent],
                    )
                    .unwrap();
            }
            None => {
                self.repo
                    .commit(
                        Some("HEAD"),
                        &signature,
                        &signature,
                        message,
                        &tree,
                        &[],
                    )
                    .unwrap();
            }
        }
    }
}
