use std::{
    fs,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, ensure};
use git2::{ErrorCode::NotFound, Repository, StatusOptions};
use phidi_core::{
    directory::Directory,
    semantic_map::{
        CURRENT_SCHEMA_VERSION, MINIMUM_READABLE_SCHEMA_VERSION,
        SchemaCompatibility, SchemaVersion, SnapshotCompleteness,
        SnapshotFreshness, SnapshotProvenance, WorkspaceSnapshot,
    },
};
use phidi_rpc::core::{
    CoreRpcHandler, LogLevel, SemanticMapDegradedReason, SemanticMapStatus,
};
use serde::Deserialize;

const SNAPSHOT_DIRECTORY: &str = "atlas/snapshots";
const SNAPSHOT_FILE_NAME: &str = "workspace_snapshot.json";
const SNAPSHOT_LOG_TARGET: &str = "atlas.snapshot";

pub trait SnapshotStorage {
    type Writer: Write;

    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    fn create_file(&self, path: &Path) -> io::Result<Self::Writer>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
}

#[derive(Debug, Default)]
pub struct FileSystemSnapshotStorage;

impl SnapshotStorage for FileSystemSnapshotStorage {
    type Writer = fs::File;

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn create_file(&self, path: &Path) -> io::Result<Self::Writer> {
        fs::File::create(path)
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }
}

#[derive(Debug)]
pub struct SnapshotStore<S = FileSystemSnapshotStorage> {
    root: PathBuf,
    storage: S,
}

impl SnapshotStore {
    pub fn local() -> Result<Self> {
        let root = Directory::cache_directory()
            .ok_or_else(|| anyhow!("can't get cache directory"))?
            .join(SNAPSHOT_DIRECTORY);
        Ok(Self::from_directory(root))
    }

    pub fn from_directory(root: PathBuf) -> Self {
        Self::new(root, FileSystemSnapshotStorage)
    }
}

impl<S> SnapshotStore<S> {
    pub fn new(root: PathBuf, storage: S) -> Self {
        Self { root, storage }
    }
}

impl<S: SnapshotStorage> SnapshotStore<S> {
    pub fn save(
        &self,
        workspace_root: &Path,
        snapshot: &WorkspaceSnapshot,
    ) -> Result<PathBuf> {
        ensure!(
            snapshot.schema_version == CURRENT_SCHEMA_VERSION,
            "refusing to persist snapshot schema {}; expected {}",
            snapshot.schema_version,
            CURRENT_SCHEMA_VERSION
        );

        let snapshot_path = self.path_for_workspace(workspace_root);
        let parent = snapshot_path
            .parent()
            .ok_or_else(|| anyhow!("snapshot path missing parent"))?;
        self.storage.create_dir_all(parent).with_context(|| {
            format!("failed to create snapshot directory {}", parent.display())
        })?;

        let file = self.storage.create_file(&snapshot_path).with_context(|| {
            format!("failed to create snapshot file {}", snapshot_path.display())
        })?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, snapshot).with_context(|| {
            format!("failed to serialize snapshot {}", snapshot_path.display())
        })?;

        Ok(snapshot_path)
    }

    pub fn load(&self, workspace_root: &Path) -> Result<SnapshotLoadResult> {
        let snapshot_path = self.path_for_workspace(workspace_root);
        let bytes = match self.storage.read(&snapshot_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(SnapshotLoadResult::Recovery(
                    SnapshotRecoveryStatus::Missing {
                        path: snapshot_path,
                    },
                ));
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to read snapshot {}", snapshot_path.display())
                });
            }
        };

        let header = match serde_json::from_slice::<SnapshotHeader>(&bytes) {
            Ok(header) => header,
            Err(error) => {
                return Ok(SnapshotLoadResult::Recovery(
                    SnapshotRecoveryStatus::Corrupt {
                        path: snapshot_path,
                        detail: error.to_string(),
                        line: error.line(),
                        column: error.column(),
                    },
                ));
            }
        };

        let compatibility = header.schema_version.compatibility_with_current();
        if matches!(
            compatibility,
            SchemaCompatibility::TooOld | SchemaCompatibility::TooNew
        ) {
            return Ok(SnapshotLoadResult::Recovery(
                SnapshotRecoveryStatus::IncompatibleSchema {
                    path: snapshot_path,
                    found_version: header.schema_version,
                    compatibility,
                },
            ));
        }

        let snapshot = match serde_json::from_slice::<WorkspaceSnapshot>(&bytes) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return Ok(SnapshotLoadResult::Recovery(
                    SnapshotRecoveryStatus::Corrupt {
                        path: snapshot_path,
                        detail: error.to_string(),
                        line: error.line(),
                        column: error.column(),
                    },
                ));
            }
        };

        Ok(SnapshotLoadResult::Loaded(snapshot))
    }

    fn path_for_workspace(&self, workspace_root: &Path) -> PathBuf {
        self.root
            .join(workspace_directory_name(workspace_root))
            .join(SNAPSHOT_FILE_NAME)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum SnapshotLoadResult {
    Loaded(WorkspaceSnapshot),
    Recovery(SnapshotRecoveryStatus),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotRecoveryStatus {
    Missing {
        path: PathBuf,
    },
    IncompatibleSchema {
        path: PathBuf,
        found_version: SchemaVersion,
        compatibility: SchemaCompatibility,
    },
    Corrupt {
        path: PathBuf,
        detail: String,
        line: usize,
        column: usize,
    },
}

impl SnapshotRecoveryStatus {
    pub fn log_message(&self) -> Option<String> {
        match self {
            Self::Missing { .. } => None,
            Self::IncompatibleSchema {
                path,
                found_version,
                compatibility,
            } => Some(format!(
                "Ignoring incompatible workspace snapshot at {}: {}. Supported snapshot schemas for this build are {} through {}. Rebuild the snapshot with the current proxy.",
                path.display(),
                incompatible_schema_guidance(*found_version, *compatibility),
                MINIMUM_READABLE_SCHEMA_VERSION,
                CURRENT_SCHEMA_VERSION,
            )),
            Self::Corrupt {
                path,
                detail,
                line,
                column,
            } => Some(format!(
                "Ignoring corrupt workspace snapshot at {} (line {}, column {}): {}. Rebuild the snapshot to recover.",
                path.display(),
                line,
                column,
                detail
            )),
        }
    }
}

fn incompatible_schema_guidance(
    found_version: SchemaVersion,
    compatibility: SchemaCompatibility,
) -> String {
    match compatibility {
        SchemaCompatibility::TooOld => {
            format!("found schema {} which is too old to read", found_version)
        }
        SchemaCompatibility::TooNew => {
            format!("found schema {} which is too new to read", found_version)
        }
        SchemaCompatibility::Current | SchemaCompatibility::Compatible => {
            format!("found schema {} with unexpected compatibility state", found_version)
        }
    }
}

#[derive(Deserialize)]
struct SnapshotHeader {
    schema_version: SchemaVersion,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct StartupSnapshotState {
    status: SemanticMapStatus,
    log_message: Option<String>,
}

impl StartupSnapshotState {
    fn ready() -> Self {
        Self {
            status: SemanticMapStatus::Ready,
            log_message: None,
        }
    }

    fn degraded(
        reason: SemanticMapDegradedReason,
        detail: String,
        log_message: Option<String>,
    ) -> Self {
        Self {
            status: SemanticMapStatus::Degraded { reason, detail },
            log_message,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotFreshnessStatus {
    pub freshness: SnapshotFreshness,
    pub guidance: RebuildGuidance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RebuildGuidance {
    None,
    Recommended(String),
    Required(String),
}

pub fn capture_workspace_provenance(
    workspace_path: &Path,
) -> Result<SnapshotProvenance> {
    let repo = match Repository::discover(workspace_path) {
        Ok(repo) => repo,
        Err(error) if error.code() == NotFound => {
            return Ok(SnapshotProvenance::default());
        }
        Err(error) => return Err(error.into()),
    };

    Ok(SnapshotProvenance {
        revision: current_revision(&repo)?,
        has_uncommitted_changes: has_uncommitted_changes(&repo)?,
    })
}

pub fn evaluate_snapshot_freshness(
    snapshot: &WorkspaceSnapshot,
    workspace_provenance: &SnapshotProvenance,
) -> SnapshotFreshnessStatus {
    match snapshot.schema_compatibility() {
        SchemaCompatibility::Current | SchemaCompatibility::Compatible => {}
        SchemaCompatibility::TooOld | SchemaCompatibility::TooNew => {
            return SnapshotFreshnessStatus {
                freshness: SnapshotFreshness::Incompatible,
                guidance: RebuildGuidance::Required(format!(
                    "Rebuild required: snapshot schema {} is not readable by this build.",
                    snapshot.schema_version
                )),
            };
        }
    }

    let freshness = match (
        snapshot.provenance.revision.as_deref(),
        workspace_provenance.revision.as_deref(),
    ) {
        (Some(snapshot_revision), Some(workspace_revision)) => {
            if snapshot_revision == workspace_revision {
                if workspace_provenance.has_uncommitted_changes {
                    SnapshotFreshness::Drifted
                } else if snapshot.provenance.has_uncommitted_changes {
                    SnapshotFreshness::Outdated
                } else {
                    SnapshotFreshness::Exact
                }
            } else {
                SnapshotFreshness::Outdated
            }
        }
        (None, None) => {
            if workspace_provenance.has_uncommitted_changes {
                SnapshotFreshness::Drifted
            } else if snapshot.provenance.has_uncommitted_changes {
                SnapshotFreshness::Outdated
            } else {
                SnapshotFreshness::Exact
            }
        }
        _ => SnapshotFreshness::Incompatible,
    };

    SnapshotFreshnessStatus {
        guidance: guidance_for(snapshot, workspace_provenance, freshness),
        freshness,
    }
}

pub(crate) fn load_workspace_snapshot_for_startup(
    core_rpc: &CoreRpcHandler,
    workspace_root: &Path,
) {
    match SnapshotStore::local() {
        Ok(store) => {
            load_workspace_snapshot_for_startup_with_store(
                &store,
                core_rpc,
                workspace_root,
            );
        }
        Err(error) => {
            emit_startup_snapshot_state(
                core_rpc,
                startup_storage_unavailable_state(workspace_root, &error),
            );
        }
    }
}

fn load_workspace_snapshot_for_startup_with_store<S: SnapshotStorage>(
    store: &SnapshotStore<S>,
    core_rpc: &CoreRpcHandler,
    workspace_root: &Path,
) {
    emit_startup_snapshot_state(
        core_rpc,
        startup_snapshot_state(store, workspace_root),
    );
}

fn startup_snapshot_state<S: SnapshotStorage>(
    store: &SnapshotStore<S>,
    workspace_root: &Path,
) -> StartupSnapshotState {
    match store.load(workspace_root) {
        Ok(SnapshotLoadResult::Loaded(snapshot)) => {
            if snapshot.completeness == SnapshotCompleteness::Partial {
                let detail = format!(
                    "Loaded partial workspace snapshot for {} with {} diagnostic{}.",
                    workspace_root.display(),
                    snapshot.diagnostics.len(),
                    if snapshot.diagnostics.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                );
                return StartupSnapshotState::degraded(
                    SemanticMapDegradedReason::PartialSnapshot,
                    detail.clone(),
                    Some(detail),
                );
            }
            StartupSnapshotState::ready()
        }
        Ok(SnapshotLoadResult::Recovery(status)) => {
            if let Some(message) = status.log_message() {
                StartupSnapshotState::degraded(
                    SemanticMapDegradedReason::SnapshotRecovery,
                    message.clone(),
                    Some(message),
                )
            } else {
                StartupSnapshotState::ready()
            }
        }
        Err(error) => startup_storage_unavailable_state(workspace_root, &error),
    }
}

fn startup_storage_unavailable_state(
    workspace_root: &Path,
    error: &anyhow::Error,
) -> StartupSnapshotState {
    let message = format!(
        "Atlas snapshot storage unavailable for {}: {}",
        workspace_root.display(),
        error
    );
    StartupSnapshotState::degraded(
        SemanticMapDegradedReason::StorageUnavailable,
        message.clone(),
        Some(message),
    )
}

fn emit_startup_snapshot_state(
    core_rpc: &CoreRpcHandler,
    state: StartupSnapshotState,
) {
    let StartupSnapshotState {
        status,
        log_message,
    } = state;
    if let Some(message) = log_message {
        core_rpc.log(
            LogLevel::Warn,
            message,
            Some(SNAPSHOT_LOG_TARGET.to_string()),
        );
    }
    core_rpc.semantic_map_status(status);
}

fn current_revision(repo: &Repository) -> Result<Option<String>> {
    match repo.head() {
        Ok(head) => Ok(head
            .target()
            .or_else(|| head.peel_to_commit().ok().map(|commit| commit.id()))
            .map(|oid| oid.to_string())),
        Err(error) if error.code() == NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn has_uncommitted_changes(repo: &Repository) -> Result<bool> {
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut options))?;
    Ok(statuses.iter().next().is_some())
}

fn guidance_for(
    snapshot: &WorkspaceSnapshot,
    workspace_provenance: &SnapshotProvenance,
    freshness: SnapshotFreshness,
) -> RebuildGuidance {
    match freshness {
        SnapshotFreshness::Exact => RebuildGuidance::None,
        SnapshotFreshness::Drifted => RebuildGuidance::Recommended(
            "Rebuild recommended: workspace has uncommitted changes.".to_string(),
        ),
        SnapshotFreshness::Outdated => match (
            snapshot.provenance.revision.as_deref(),
            workspace_provenance.revision.as_deref(),
        ) {
            (Some(snapshot_revision), Some(workspace_revision))
                if snapshot_revision != workspace_revision =>
            {
                RebuildGuidance::Required(format!(
                    "Rebuild required: snapshot was built from revision {}, current workspace is at {}.",
                    snapshot_revision, workspace_revision
                ))
            }
            _ => RebuildGuidance::Required(
                "Rebuild required: snapshot no longer matches the current workspace state."
                    .to_string(),
            ),
        },
        SnapshotFreshness::Incompatible => RebuildGuidance::Required(
            "Rebuild required: snapshot provenance cannot be compared to this workspace."
                .to_string(),
        ),
    }
}

fn workspace_directory_name(workspace_root: &Path) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .append_key_only(&workspace_root.to_string_lossy())
        .finish()
}

#[cfg(test)]
mod tests {
    use std::{fs, io, path::Path, path::PathBuf, time::Duration};

    use git2::{Repository, Signature};
    use phidi_core::semantic_map::{
        CURRENT_SCHEMA_VERSION, MINIMUM_READABLE_SCHEMA_VERSION,
        SchemaCompatibility, SchemaVersion, SnapshotCompleteness,
        SnapshotDiagnostic, SnapshotFreshness, SnapshotKind,
        SnapshotProvenance, WorkspaceSnapshot,
    };
    use phidi_rpc::core::{
        CoreNotification, CoreRpc, CoreRpcHandler, LogLevel,
        SemanticMapDegradedReason, SemanticMapStatus,
    };
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        RebuildGuidance, SNAPSHOT_LOG_TARGET, SnapshotLoadResult,
        SnapshotRecoveryStatus, SnapshotStorage, SnapshotStore,
        capture_workspace_provenance, evaluate_snapshot_freshness,
        load_workspace_snapshot_for_startup_with_store,
    };

    // Canonical snapshot fixture for schema, freshness, and recovery contract tests.
    fn snapshot_fixture() -> WorkspaceSnapshot {
        WorkspaceSnapshot::new(
            SnapshotKind::Working,
            SnapshotProvenance {
                revision: Some("abc123".to_string()),
                has_uncommitted_changes: false,
            },
        )
    }

    fn workspace_root(tempdir: &tempfile::TempDir) -> PathBuf {
        tempdir.path().join("workspace")
    }

    fn snapshot_path_for(
        store: &SnapshotStore,
        workspace_root: &std::path::Path,
    ) -> PathBuf {
        let snapshot = snapshot_fixture();
        store.save(workspace_root, &snapshot).unwrap()
    }

    fn rewrite_snapshot_version(
        snapshot_path: &Path,
        schema_version: SchemaVersion,
    ) {
        let mut serialized = serde_json::to_value(snapshot_fixture()).unwrap();
        serialized["schema_version"] = json!({
            "major": schema_version.major,
            "minor": schema_version.minor,
        });
        fs::write(snapshot_path, serde_json::to_vec_pretty(&serialized).unwrap())
            .unwrap();
    }

    fn recv_notification(core_rpc: &CoreRpcHandler) -> CoreNotification {
        let message = core_rpc
            .rx()
            .recv_timeout(Duration::from_secs(1))
            .expect("expected notification");
        let CoreRpc::Notification(notification) = message else {
            panic!("expected notification");
        };
        *notification
    }

    #[derive(Clone, Copy)]
    enum FailingOperation {
        CreateDirAll,
        CreateFile,
        Read,
    }

    struct FailingStorage {
        operation: FailingOperation,
    }

    impl SnapshotStorage for FailingStorage {
        type Writer = Vec<u8>;

        fn create_dir_all(&self, _path: &Path) -> io::Result<()> {
            if matches!(self.operation, FailingOperation::CreateDirAll) {
                return Err(io::Error::other("injected create_dir_all failure"));
            }
            Ok(())
        }

        fn create_file(&self, _path: &Path) -> io::Result<Self::Writer> {
            if matches!(self.operation, FailingOperation::CreateFile) {
                return Err(io::Error::other("injected create_file failure"));
            }
            Ok(Vec::new())
        }

        fn read(&self, _path: &Path) -> io::Result<Vec<u8>> {
            if matches!(self.operation, FailingOperation::Read) {
                return Err(io::Error::other("injected read failure"));
            }
            Ok(Vec::new())
        }
    }

    #[test]
    fn saves_and_loads_valid_snapshots() {
        let tempdir = tempdir().unwrap();
        let store = SnapshotStore::from_directory(tempdir.path().join("snapshots"));
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let snapshot = snapshot_fixture();
        let saved_path = store.save(&workspace_root, &snapshot).unwrap();

        assert!(saved_path.exists());
        assert_eq!(
            store.load(&workspace_root).unwrap(),
            SnapshotLoadResult::Loaded(snapshot)
        );
    }

    #[test]
    fn rejects_writing_non_current_schema_versions() {
        let tempdir = tempdir().unwrap();
        let store = SnapshotStore::from_directory(tempdir.path().join("snapshots"));
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let mut snapshot = snapshot_fixture();
        snapshot.schema_version.major += 1;

        let error = store.save(&workspace_root, &snapshot).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("refusing to persist snapshot schema")
        );
    }

    #[test]
    fn save_reports_injected_directory_creation_failures() {
        let tempdir = tempdir().unwrap();
        let store = SnapshotStore::new(
            tempdir.path().join("snapshots"),
            FailingStorage {
                operation: FailingOperation::CreateDirAll,
            },
        );
        let workspace_root = workspace_root(&tempdir);

        let error = store
            .save(&workspace_root, &snapshot_fixture())
            .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to create snapshot directory")
        );
        assert!(
            error
                .root_cause()
                .to_string()
                .contains("injected create_dir_all failure")
        );
    }

    #[test]
    fn save_reports_injected_file_creation_failures() {
        let tempdir = tempdir().unwrap();
        let store = SnapshotStore::new(
            tempdir.path().join("snapshots"),
            FailingStorage {
                operation: FailingOperation::CreateFile,
            },
        );
        let workspace_root = workspace_root(&tempdir);

        let error = store
            .save(&workspace_root, &snapshot_fixture())
            .unwrap_err();

        assert!(error.to_string().contains("failed to create snapshot file"));
        assert!(
            error
                .root_cause()
                .to_string()
                .contains("injected create_file failure")
        );
    }

    #[test]
    fn returns_structured_recovery_for_incompatible_schema_versions() {
        let tempdir = tempdir().unwrap();
        let store = SnapshotStore::from_directory(tempdir.path().join("snapshots"));
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let snapshot_path = snapshot_path_for(&store, &workspace_root);
        rewrite_snapshot_version(
            &snapshot_path,
            SchemaVersion {
                major: CURRENT_SCHEMA_VERSION.major + 1,
                minor: CURRENT_SCHEMA_VERSION.minor,
            },
        );

        assert_eq!(
            store.load(&workspace_root).unwrap(),
            SnapshotLoadResult::Recovery(
                SnapshotRecoveryStatus::IncompatibleSchema {
                    path: snapshot_path,
                    found_version: SchemaVersion {
                        major: CURRENT_SCHEMA_VERSION.major + 1,
                        minor: CURRENT_SCHEMA_VERSION.minor,
                    },
                    compatibility: SchemaCompatibility::TooNew,
                },
            )
        );
    }

    #[test]
    fn loads_previous_minor_schema_snapshots() {
        let tempdir = tempdir().unwrap();
        let store = SnapshotStore::from_directory(tempdir.path().join("snapshots"));
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let snapshot_path = snapshot_path_for(&store, &workspace_root);
        let previous_minor = MINIMUM_READABLE_SCHEMA_VERSION;
        rewrite_snapshot_version(&snapshot_path, previous_minor);

        let SnapshotLoadResult::Loaded(snapshot) = store.load(&workspace_root).unwrap()
        else {
            panic!("expected previous minor snapshot to load");
        };

        assert_eq!(snapshot.schema_version, previous_minor);
    }

    #[test]
    fn incompatible_schema_guidance_explains_supported_reader_window() {
        let too_old = SnapshotRecoveryStatus::IncompatibleSchema {
            path: PathBuf::from("/tmp/old-snapshot.json"),
            found_version: SchemaVersion::new(
                CURRENT_SCHEMA_VERSION.major - 1,
                CURRENT_SCHEMA_VERSION.minor,
            ),
            compatibility: SchemaCompatibility::TooOld,
        };
        let too_new = SnapshotRecoveryStatus::IncompatibleSchema {
            path: PathBuf::from("/tmp/new-snapshot.json"),
            found_version: SchemaVersion::new(
                CURRENT_SCHEMA_VERSION.major,
                CURRENT_SCHEMA_VERSION.minor + 1,
            ),
            compatibility: SchemaCompatibility::TooNew,
        };

        let too_old_message = too_old.log_message().unwrap();
        assert!(too_old_message.contains("too old"));
        assert!(too_old_message.contains(&MINIMUM_READABLE_SCHEMA_VERSION.to_string()));
        assert!(too_old_message.contains(&CURRENT_SCHEMA_VERSION.to_string()));
        assert!(too_old_message.contains("Rebuild the snapshot"));

        let too_new_message = too_new.log_message().unwrap();
        assert!(too_new_message.contains("too new"));
        assert!(too_new_message.contains(&MINIMUM_READABLE_SCHEMA_VERSION.to_string()));
        assert!(too_new_message.contains(&CURRENT_SCHEMA_VERSION.to_string()));
        assert!(too_new_message.contains("Rebuild the snapshot"));
    }

    #[test]
    fn returns_structured_recovery_for_corrupt_snapshots() {
        let tempdir = tempdir().unwrap();
        let store = SnapshotStore::from_directory(tempdir.path().join("snapshots"));
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let snapshot_path = snapshot_path_for(&store, &workspace_root);
        fs::write(&snapshot_path, br#"{"schema_version":"oops""#).unwrap();

        let SnapshotLoadResult::Recovery(SnapshotRecoveryStatus::Corrupt {
            path,
            detail,
            ..
        }) = store.load(&workspace_root).unwrap()
        else {
            panic!("expected corrupt snapshot recovery");
        };

        assert_eq!(path, snapshot_path);
        assert!(!detail.is_empty());
    }

    #[test]
    fn missing_snapshot_returns_recovery_status_instead_of_error() {
        let tempdir = tempdir().unwrap();
        let store = SnapshotStore::from_directory(tempdir.path().join("snapshots"));
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let result = store.load(&workspace_root).unwrap();
        let SnapshotLoadResult::Recovery(SnapshotRecoveryStatus::Missing { path }) =
            result
        else {
            panic!("expected missing snapshot recovery");
        };

        assert!(path.ends_with("workspace_snapshot.json"));
    }

    #[test]
    fn load_reports_injected_read_failures() {
        let tempdir = tempdir().unwrap();
        let store = SnapshotStore::new(
            tempdir.path().join("snapshots"),
            FailingStorage {
                operation: FailingOperation::Read,
            },
        );
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let error = store.load(&workspace_root).unwrap_err();

        assert!(error.to_string().contains("failed to read snapshot"));
        assert!(
            error
                .root_cause()
                .to_string()
                .contains("injected read failure")
        );
    }

    #[test]
    fn startup_safe_load_path_logs_and_recovers_from_incompatible_snapshots() {
        let tempdir = tempdir().unwrap();
        let store_root = tempdir.path().join("snapshots");
        let store = SnapshotStore::from_directory(store_root.clone());
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let snapshot_path = snapshot_path_for(&store, &workspace_root);
        rewrite_snapshot_version(
            &snapshot_path,
            SchemaVersion::new(
                CURRENT_SCHEMA_VERSION.major + 1,
                CURRENT_SCHEMA_VERSION.minor,
            ),
        );

        let core_rpc = CoreRpcHandler::new();
        let startup_store = SnapshotStore::from_directory(store_root);
        load_workspace_snapshot_for_startup_with_store(
            &startup_store,
            &core_rpc,
            &workspace_root,
        );

        let CoreNotification::Log {
            level,
            message,
            target,
        } = recv_notification(&core_rpc)
        else {
            panic!("expected log notification");
        };

        assert!(matches!(level, LogLevel::Warn));
        assert_eq!(target.as_deref(), Some(SNAPSHOT_LOG_TARGET));
        assert!(message.contains("Ignoring incompatible workspace snapshot"));

        let CoreNotification::SemanticMapStatus { status } =
            recv_notification(&core_rpc)
        else {
            panic!("expected semantic-map status notification");
        };
        assert!(matches!(
            status,
            SemanticMapStatus::Degraded {
                reason: SemanticMapDegradedReason::SnapshotRecovery,
                ..
            }
        ));
    }

    #[test]
    fn startup_safe_load_path_logs_corrupt_snapshots_with_location_details() {
        let tempdir = tempdir().unwrap();
        let store_root = tempdir.path().join("snapshots");
        let store = SnapshotStore::from_directory(store_root.clone());
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let snapshot_path = snapshot_path_for(&store, &workspace_root);
        fs::write(&snapshot_path, b"{\n  \"schema_version\":\n").unwrap();

        let core_rpc = CoreRpcHandler::new();
        let startup_store = SnapshotStore::from_directory(store_root);
        load_workspace_snapshot_for_startup_with_store(
            &startup_store,
            &core_rpc,
            &workspace_root,
        );

        let CoreNotification::Log {
            level,
            message,
            target,
        } = recv_notification(&core_rpc)
        else {
            panic!("expected log notification");
        };

        assert!(matches!(level, LogLevel::Warn));
        assert_eq!(target.as_deref(), Some(SNAPSHOT_LOG_TARGET));
        assert!(message.contains("Ignoring corrupt workspace snapshot"));
        assert!(message.contains("line"));
        assert!(message.contains("column"));

        let CoreNotification::SemanticMapStatus { status } =
            recv_notification(&core_rpc)
        else {
            panic!("expected semantic-map status notification");
        };
        assert!(matches!(
            status,
            SemanticMapStatus::Degraded {
                reason: SemanticMapDegradedReason::SnapshotRecovery,
                ..
            }
        ));
    }

    #[test]
    fn startup_safe_load_path_marks_partial_snapshots_as_degraded() {
        let tempdir = tempdir().unwrap();
        let store_root = tempdir.path().join("snapshots");
        let store = SnapshotStore::from_directory(store_root.clone());
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();

        let mut snapshot = snapshot_fixture();
        snapshot.completeness = SnapshotCompleteness::Partial;
        snapshot.diagnostics.push(SnapshotDiagnostic {
            code: Some("parse-file".to_string()),
            severity: phidi_core::semantic_map::DiagnosticSeverity::Warning,
            message: "failed to parse Rust source file".to_string(),
            location: None,
        });
        store.save(&workspace_root, &snapshot).unwrap();

        let core_rpc = CoreRpcHandler::new();
        let startup_store = SnapshotStore::from_directory(store_root);
        load_workspace_snapshot_for_startup_with_store(
            &startup_store,
            &core_rpc,
            &workspace_root,
        );

        let CoreNotification::Log {
            level,
            message,
            target,
        } = recv_notification(&core_rpc)
        else {
            panic!("expected log notification");
        };
        assert!(matches!(level, LogLevel::Warn));
        assert_eq!(target.as_deref(), Some(SNAPSHOT_LOG_TARGET));
        assert!(message.contains("Loaded partial workspace snapshot"));

        let CoreNotification::SemanticMapStatus { status } =
            recv_notification(&core_rpc)
        else {
            panic!("expected semantic-map status notification");
        };
        assert!(matches!(
            status,
            SemanticMapStatus::Degraded {
                reason: SemanticMapDegradedReason::PartialSnapshot,
                ..
            }
        ));
    }

    #[test]
    fn startup_safe_load_path_marks_storage_failures_as_degraded() {
        let tempdir = tempdir().unwrap();
        let workspace_root = workspace_root(&tempdir);
        fs::create_dir_all(&workspace_root).unwrap();
        let store = SnapshotStore::new(
            tempdir.path().join("snapshots"),
            FailingStorage {
                operation: FailingOperation::Read,
            },
        );

        let core_rpc = CoreRpcHandler::new();
        load_workspace_snapshot_for_startup_with_store(
            &store,
            &core_rpc,
            &workspace_root,
        );

        let CoreNotification::Log {
            level,
            message,
            target,
        } = recv_notification(&core_rpc)
        else {
            panic!("expected log notification");
        };
        assert!(matches!(level, LogLevel::Warn));
        assert_eq!(target.as_deref(), Some(SNAPSHOT_LOG_TARGET));
        assert!(message.contains("Atlas snapshot storage unavailable"));

        let CoreNotification::SemanticMapStatus { status } =
            recv_notification(&core_rpc)
        else {
            panic!("expected semantic-map status notification");
        };
        assert!(matches!(
            status,
            SemanticMapStatus::Degraded {
                reason: SemanticMapDegradedReason::StorageUnavailable,
                ..
            }
        ));
    }

    #[test]
    fn evaluates_matching_clean_commit_as_exact() {
        let fixture = GitFixture::new();
        let provenance = capture_workspace_provenance(fixture.path()).unwrap();
        let snapshot = WorkspaceSnapshot::new(SnapshotKind::Working, provenance);

        let status = evaluate_snapshot_freshness(&snapshot, &snapshot.provenance);

        assert_eq!(status.freshness, SnapshotFreshness::Exact);
        assert_eq!(status.guidance, RebuildGuidance::None);
    }

    #[test]
    fn evaluates_matching_commit_with_dirty_workspace_as_drifted() {
        let fixture = GitFixture::new();
        let clean_provenance = capture_workspace_provenance(fixture.path()).unwrap();
        fixture.write("src/lib.rs", "pub fn value() -> u8 { 2 }\n");
        let workspace_provenance =
            capture_workspace_provenance(fixture.path()).unwrap();
        let snapshot =
            WorkspaceSnapshot::new(SnapshotKind::Working, clean_provenance);

        let status = evaluate_snapshot_freshness(&snapshot, &workspace_provenance);

        assert_eq!(status.freshness, SnapshotFreshness::Drifted);
        assert_eq!(
            status.guidance,
            RebuildGuidance::Recommended(
                "Rebuild recommended: workspace has uncommitted changes."
                    .to_string()
            )
        );
    }

    #[test]
    fn evaluates_revision_change_as_outdated() {
        let fixture = GitFixture::new();
        let initial_provenance =
            capture_workspace_provenance(fixture.path()).unwrap();
        fixture.write("src/lib.rs", "pub fn value() -> u8 { 2 }\n");
        fixture.commit_all("second");
        let workspace_provenance =
            capture_workspace_provenance(fixture.path()).unwrap();
        let snapshot = WorkspaceSnapshot::new(
            SnapshotKind::Working,
            initial_provenance.clone(),
        );

        let status = evaluate_snapshot_freshness(&snapshot, &workspace_provenance);

        assert_eq!(status.freshness, SnapshotFreshness::Outdated);
        assert_eq!(
            status.guidance,
            RebuildGuidance::Required(format!(
                "Rebuild required: snapshot was built from revision {}, current workspace is at {}.",
                initial_provenance.revision.unwrap(),
                workspace_provenance.revision.unwrap()
            ))
        );
    }

    #[test]
    fn evaluates_uncomparable_provenance_as_incompatible() {
        let fixture = GitFixture::new();
        let workspace_provenance =
            capture_workspace_provenance(fixture.path()).unwrap();
        let snapshot = WorkspaceSnapshot::new(
            SnapshotKind::Working,
            SnapshotProvenance {
                revision: None,
                has_uncommitted_changes: false,
            },
        );

        let status = evaluate_snapshot_freshness(&snapshot, &workspace_provenance);

        assert_eq!(status.freshness, SnapshotFreshness::Incompatible);
        assert_eq!(
            status.guidance,
            RebuildGuidance::Required(
                "Rebuild required: snapshot provenance cannot be compared to this workspace."
                    .to_string()
            )
        );
    }

    #[test]
    fn evaluates_schema_mismatch_as_incompatible() {
        let fixture = GitFixture::new();
        let provenance = capture_workspace_provenance(fixture.path()).unwrap();
        let mut snapshot = WorkspaceSnapshot::new(SnapshotKind::Working, provenance);
        snapshot.schema_version =
            SchemaVersion::new(CURRENT_SCHEMA_VERSION.major + 1, 0);

        let status = evaluate_snapshot_freshness(&snapshot, &snapshot.provenance);

        assert_eq!(status.freshness, SnapshotFreshness::Incompatible);
        assert_eq!(
            status.guidance,
            RebuildGuidance::Required(format!(
                "Rebuild required: snapshot schema {} is not readable by this build.",
                snapshot.schema_version
            ))
        );
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
            fixture.write("src/lib.rs", "pub fn value() -> u8 { 1 }\n");
            fixture.commit_all("initial");
            fixture
        }

        fn path(&self) -> &Path {
            self.tempdir.path()
        }

        fn write(&self, relative_path: &str, contents: &str) {
            let path = self.path().join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }

        fn commit_all(&self, message: &str) {
            let mut index = self.repo.index().unwrap();
            index
                .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
                .unwrap();
            index.write().unwrap();

            let tree_id = index.write_tree().unwrap();
            let tree = self.repo.find_tree(tree_id).unwrap();
            let signature =
                Signature::now("Phidi Tests", "tests@phidi.dev").unwrap();
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
}
