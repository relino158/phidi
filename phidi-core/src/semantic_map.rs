//! Core semantic-map contract shared by snapshot persistence, extraction, and RPC layers.
//!
//! The collections in [`WorkspaceSnapshot`] are intended to be serialized in a stable order so
//! identical inputs produce identical snapshot artifacts.

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use thiserror::Error;

/// The newest snapshot schema this build knows how to emit.
pub const CURRENT_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 1);

/// The oldest snapshot schema this build promises to read without migration.
pub const MINIMUM_READABLE_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0);

/// Persisted semantic-map artifact for one workspace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    /// Versioned schema marker for compatibility checks.
    pub schema_version: SchemaVersion,
    /// Distinguishes local working snapshots from optional VCS-tracked seed artifacts.
    pub kind: SnapshotKind,
    /// How well this snapshot still reflects the current workspace state.
    pub freshness: SnapshotFreshness,
    /// VCS information captured when the snapshot was built.
    pub provenance: SnapshotProvenance,
    /// Whether extraction completed fully or degraded to a partial result.
    pub completeness: SnapshotCompleteness,
    /// Semantic entities discovered for the workspace.
    pub entities: Vec<SemanticEntity>,
    /// Directed relationships between semantic entities.
    pub relationships: Vec<SemanticRelationship>,
    /// Non-fatal extraction issues attached to the artifact.
    pub diagnostics: Vec<SnapshotDiagnostic>,
}

impl WorkspaceSnapshot {
    /// Creates an empty snapshot using the current schema version.
    pub fn new(kind: SnapshotKind, provenance: SnapshotProvenance) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            kind,
            freshness: SnapshotFreshness::Exact,
            provenance,
            completeness: SnapshotCompleteness::Complete,
            entities: Vec::new(),
            relationships: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Reports whether this snapshot can be read by the current build.
    pub const fn schema_compatibility(&self) -> SchemaCompatibility {
        self.schema_version.compatibility_with_current()
    }
}

/// Snapshot schema version encoded as explicit major/minor fields.
///
/// The compatibility strategy is:
/// - writers always emit [`CURRENT_SCHEMA_VERSION`]
/// - readers accept versions in the closed range
///   [`MINIMUM_READABLE_SCHEMA_VERSION`, `CURRENT_SCHEMA_VERSION`]
/// - any newer version is treated as incompatible
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct SchemaVersion {
    /// Breaking schema line. Readers require the same major version.
    pub major: u16,
    /// Backward-compatible revision within a major schema line.
    pub minor: u16,
}

impl SchemaVersion {
    /// Creates a schema version.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// Compares this version against the supported range for a reader.
    pub const fn compatibility(
        self,
        current: SchemaVersion,
        minimum: SchemaVersion,
    ) -> SchemaCompatibility {
        if self.major != current.major {
            return if self.major < current.major {
                SchemaCompatibility::TooOld
            } else {
                SchemaCompatibility::TooNew
            };
        }

        if self.minor > current.minor {
            SchemaCompatibility::TooNew
        } else if self.minor < minimum.minor {
            SchemaCompatibility::TooOld
        } else if self.minor == current.minor {
            SchemaCompatibility::Current
        } else {
            SchemaCompatibility::Compatible
        }
    }

    /// Compares this version against the support policy compiled into the current build.
    pub const fn compatibility_with_current(self) -> SchemaCompatibility {
        self.compatibility(CURRENT_SCHEMA_VERSION, MINIMUM_READABLE_SCHEMA_VERSION)
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Result of comparing a snapshot's schema version with the current reader.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SchemaCompatibility {
    /// Matches the exact schema version produced by this build.
    Current,
    /// Older but still readable without a breaking migration.
    Compatible,
    /// Older than the minimum readable version for this build.
    TooOld,
    /// Newer than the reader understands.
    TooNew,
}

/// Snapshot storage mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotKind {
    /// Local artifact for the current working workspace.
    Working,
    /// Optional repository-tracked bootstrap artifact.
    Seed,
}

/// How closely a snapshot matches the current workspace state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotFreshness {
    /// Schema and workspace state match exactly.
    Exact,
    /// Schema is compatible and the revision matches, but the working tree is dirty.
    Drifted,
    /// Schema is compatible, but the snapshot was built from older workspace content.
    Outdated,
    /// Schema or workspace identity mismatch makes the artifact unsafe to trust.
    Incompatible,
}

/// VCS facts captured when building a snapshot.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotProvenance {
    /// Commit hash or revision identifier used as the snapshot baseline.
    pub revision: Option<String>,
    /// Whether the workspace had uncommitted changes at capture time.
    pub has_uncommitted_changes: bool,
}

/// Whether extraction completed fully or had to emit a partial artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotCompleteness {
    /// No extraction failures were recorded.
    Complete,
    /// Some files or relationships were skipped, but the artifact remains usable.
    Partial,
}

/// Stable identifier for a semantic entity inside a snapshot.
#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct EntityId(pub String);

/// Semantic node stored in the workspace graph.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticEntity {
    /// Snapshot-local stable identifier.
    pub id: EntityId,
    /// Broad semantic category of the entity.
    pub kind: EntityKind,
    /// Display name for UI and agent output.
    pub name: String,
    /// Fully-qualified or otherwise disambiguated name when available.
    pub qualified_name: Option<String>,
    /// Workspace-relative source location when the entity maps to source text.
    pub location: Option<EntityLocation>,
}

/// Broad semantic categories supported by the Rust v1 extractor.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntityKind {
    Enum,
    File,
    Function,
    ImplBlock,
    Import,
    Macro,
    Method,
    Module,
    Package,
    Struct,
    Test,
    Trait,
    Workspace,
}

/// Directed edge between two semantic entities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SemanticRelationship {
    /// Source entity identifier.
    pub source: EntityId,
    /// Target entity identifier.
    pub target: EntityId,
    /// Meaning of the directed relationship.
    pub kind: RelationshipKind,
    /// Explicit certainty metadata for direct and inferred links.
    pub certainty: Certainty,
    /// Origin of the evidence used to create the edge.
    pub provenance: RelationshipProvenance,
}

/// Relationship kinds needed by the initial extractor and agent features.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelationshipKind {
    Calls,
    Contains,
    Defines,
    Implements,
    Imports,
    References,
}

/// Certainty metadata carried by every semantic relationship.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Certainty {
    /// Distinguishes directly observed facts from inferred ones.
    pub kind: CertaintyKind,
    /// Confidence score in percent, bounded to `0..=100`.
    pub confidence: ConfidenceScore,
}

impl Certainty {
    /// Certainty metadata for directly observed relationships.
    pub const fn observed() -> Self {
        Self {
            kind: CertaintyKind::Observed,
            confidence: ConfidenceScore::MAX,
        }
    }

    /// Certainty metadata for inferred relationships.
    pub const fn inferred(confidence: ConfidenceScore) -> Self {
        Self {
            kind: CertaintyKind::Inferred,
            confidence,
        }
    }
}

/// Whether a relationship was directly observed or inferred from indirect evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CertaintyKind {
    Observed,
    Inferred,
}

/// Confidence score stored as a percentage to keep the wire format deterministic.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConfidenceScore(u8);

impl ConfidenceScore {
    /// Zero confidence.
    pub const MIN: Self = Self(0);
    /// Maximum confidence for directly observed facts.
    pub const MAX: Self = Self(100);

    /// Creates a bounded confidence score.
    pub fn new(value: u8) -> Result<Self, InvalidConfidenceScore> {
        if value <= Self::MAX.0 {
            Ok(Self(value))
        } else {
            Err(InvalidConfidenceScore(value))
        }
    }

    /// Returns the underlying percentage.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for ConfidenceScore {
    type Error = InvalidConfidenceScore;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ConfidenceScore> for u8 {
    fn from(value: ConfidenceScore) -> Self {
        value.get()
    }
}

impl Serialize for ConfidenceScore {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for ConfidenceScore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u8::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

/// Error returned when a confidence score falls outside the allowed range.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("confidence score must be between 0 and 100, got {0}")]
pub struct InvalidConfidenceScore(pub u8);

/// Evidence attached to a semantic relationship.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RelationshipProvenance {
    /// Main source of evidence for the relationship.
    pub source: ProvenanceSource,
    /// Optional human-readable explanation, such as the applied heuristic.
    pub detail: Option<String>,
}

/// High-level source categories for relationship evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceSource {
    Heuristic,
    SymbolResolution,
    SyntaxTree,
}

/// Workspace-relative location of a semantic entity or diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntityLocation {
    /// Path relative to the workspace root.
    pub path: String,
    /// Optional zero-based span inside the file.
    pub span: Option<TextSpan>,
}

/// Zero-based source span.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct TextSpan {
    pub start: TextPoint,
    pub end: TextPoint,
}

/// Zero-based line and column position.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct TextPoint {
    pub line: u32,
    pub column: u32,
}

/// Non-fatal issue captured while building a snapshot.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotDiagnostic {
    /// Machine-readable classifier when available.
    pub code: Option<String>,
    /// Issue severity.
    pub severity: DiagnosticSeverity,
    /// Human-readable description.
    pub message: String,
    /// Optional source location related to the issue.
    pub location: Option<EntityLocation>,
}

/// Diagnostic severity stored in a snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        CURRENT_SCHEMA_VERSION, MINIMUM_READABLE_SCHEMA_VERSION, Certainty,
        ConfidenceScore, EntityId, InvalidConfidenceScore, ProvenanceSource,
        RelationshipKind, RelationshipProvenance, SchemaCompatibility,
        SchemaVersion, SemanticRelationship, SnapshotKind, SnapshotProvenance,
        WorkspaceSnapshot,
    };

    #[test]
    fn schema_version_compatibility_follows_supported_range() {
        assert!(
            CURRENT_SCHEMA_VERSION.minor > 0,
            "compatibility matrix requires a current schema with a previous minor"
        );

        let previous_minor = SchemaVersion::new(
            CURRENT_SCHEMA_VERSION.major,
            CURRENT_SCHEMA_VERSION.minor - 1,
        );
        let too_old = SchemaVersion::new(
            CURRENT_SCHEMA_VERSION.major - 1,
            CURRENT_SCHEMA_VERSION.minor,
        );
        let too_new = SchemaVersion::new(
            CURRENT_SCHEMA_VERSION.major,
            CURRENT_SCHEMA_VERSION.minor + 1,
        );

        assert_eq!(
            CURRENT_SCHEMA_VERSION.compatibility_with_current(),
            SchemaCompatibility::Current
        );
        assert_eq!(
            MINIMUM_READABLE_SCHEMA_VERSION.compatibility_with_current(),
            SchemaCompatibility::Compatible
        );
        assert_eq!(
            previous_minor.compatibility_with_current(),
            SchemaCompatibility::Compatible
        );
        assert_eq!(
            too_old.compatibility_with_current(),
            SchemaCompatibility::TooOld
        );
        assert_eq!(
            too_new.compatibility_with_current(),
            SchemaCompatibility::TooNew
        );
    }

    #[test]
    fn confidence_score_rejects_out_of_range_values() {
        assert_eq!(
            ConfidenceScore::new(101).unwrap_err(),
            InvalidConfidenceScore(101)
        );
        assert_eq!(ConfidenceScore::new(100).unwrap().get(), 100);
    }

    #[test]
    fn workspace_snapshot_defaults_to_current_schema() {
        let snapshot = WorkspaceSnapshot::new(
            SnapshotKind::Working,
            SnapshotProvenance {
                revision: Some("abc123".to_string()),
                has_uncommitted_changes: false,
            },
        );

        assert_eq!(snapshot.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(
            snapshot.schema_compatibility(),
            SchemaCompatibility::Current
        );
        assert!(snapshot.entities.is_empty());
        assert!(snapshot.relationships.is_empty());
        assert!(snapshot.diagnostics.is_empty());
    }

    #[test]
    fn relationship_schema_serializes_as_expected() {
        let mut snapshot = WorkspaceSnapshot::new(
            SnapshotKind::Working,
            SnapshotProvenance {
                revision: Some("abc123".to_string()),
                has_uncommitted_changes: true,
            },
        );

        snapshot.relationships.push(SemanticRelationship {
            source: EntityId("module:alpha".to_string()),
            target: EntityId("function:beta".to_string()),
            kind: RelationshipKind::Calls,
            certainty: Certainty::inferred(ConfidenceScore::new(72).unwrap()),
            provenance: RelationshipProvenance {
                source: ProvenanceSource::Heuristic,
                detail: Some("name matched call target".to_string()),
            },
        });

        let value = serde_json::to_value(snapshot).unwrap();

        assert_eq!(value["kind"], json!("working"));
        assert_eq!(value["freshness"], json!("exact"));
        assert_eq!(
            value["relationships"][0]["certainty"]["kind"],
            json!("inferred")
        );
        assert_eq!(
            value["relationships"][0]["certainty"]["confidence"],
            json!(72)
        );
        assert_eq!(
            value["relationships"][0]["provenance"]["source"],
            json!("heuristic")
        );
    }

    #[test]
    fn workspace_snapshot_serializes_foundation_contract_fields_stably() {
        let mut snapshot = WorkspaceSnapshot::new(
            SnapshotKind::Working,
            SnapshotProvenance {
                revision: Some("abc123".to_string()),
                has_uncommitted_changes: false,
            },
        );
        snapshot.completeness = super::SnapshotCompleteness::Partial;

        let value = serde_json::to_value(snapshot).unwrap();

        assert_eq!(
            value["schema_version"],
            json!({
                "major": CURRENT_SCHEMA_VERSION.major,
                "minor": CURRENT_SCHEMA_VERSION.minor
            })
        );
        assert_eq!(value["completeness"], json!("partial"));
    }
}
