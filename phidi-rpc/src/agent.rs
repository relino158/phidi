//! Typed RPC records for the six built-in agent capabilities.
//!
//! These schemas define the stable wire contract for the native Atlas-style
//! operations without depending on `phidi-core`. They intentionally duplicate
//! a few foundational types so downstream crates can share one RPC crate
//! without creating a dependency cycle.

use std::{error::Error, fmt};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Request union for the six built-in agent capabilities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "capability", content = "params", rename_all = "kebab-case")]
pub enum AgentCapabilityRequest {
    ConceptDiscovery(ConceptDiscoveryRequest),
    EntityBriefing(EntityBriefingRequest),
    BlastRadiusEstimation(BlastRadiusRequest),
    DeltaImpactScan(DeltaImpactScanRequest),
    RenamePlanning(RenamePlanningRequest),
    StructuralQuery(StructuralQueryRequest),
}

/// Response union for the six built-in agent capabilities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "capability", content = "response", rename_all = "kebab-case")]
pub enum AgentCapabilityResponse {
    ConceptDiscovery(ConceptDiscoveryResponse),
    EntityBriefing(EntityBriefingResponse),
    BlastRadiusEstimation(BlastRadiusResponse),
    DeltaImpactScan(DeltaImpactScanResponse),
    RenamePlanning(RenamePlanningResponse),
    StructuralQuery(StructuralQueryResponse),
}

/// Shared execution envelope for all agent capability responses.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum CapabilityResponse<T> {
    /// The operation finished within its time budget.
    Success {
        /// Operation-specific result payload.
        result: T,
    },
    /// The operation hit its time budget and may include a partial result.
    Timeout {
        /// Standard timeout metadata shared by all operations.
        timeout: CapabilityTimeout,
        /// Partial output gathered before the deadline, when available.
        partial_result: Option<T>,
    },
    /// The operation failed before producing a usable result.
    Error {
        /// Standard error metadata shared by all operations.
        error: CapabilityError,
    },
}

/// Common error metadata for all agent capability failures.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityError {
    /// Stable machine-readable failure category.
    pub code: CapabilityErrorCode,
    /// Human-readable explanation suitable for logs and UI.
    pub message: String,
    /// Whether a caller may retry the operation without changing inputs.
    pub retryable: bool,
}

/// Standardized failure categories for built-in agent capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityErrorCode {
    InvalidRequest,
    SnapshotUnavailable,
    SnapshotIncompatible,
    UnsupportedQuery,
    Internal,
}

/// Standard timeout metadata for all agent capability responses.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityTimeout {
    /// Maximum runtime budget requested for the operation.
    pub limit_ms: u64,
    /// Runtime consumed before the timeout response was emitted.
    pub elapsed_ms: u64,
}

/// Reusable entity handle used across capability results.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AgentEntity {
    /// Snapshot-local or service-local stable identifier.
    pub id: String,
    /// Broad semantic category of the entity.
    pub kind: AgentEntityKind,
    /// Display name for UI and agent output.
    pub name: String,
    /// Fully-qualified or otherwise disambiguated name when available.
    pub qualified_name: Option<String>,
    /// Workspace-relative source location when the entity maps to source text.
    pub location: Option<EntityLocation>,
}

/// Semantic categories exposed through the agent capability layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentEntityKind {
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

/// Location of an entity or preview span, relative to the active workspace.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntityLocation {
    /// Workspace-relative file path.
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

/// Certainty metadata attached to inferred capability output.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Certainty {
    /// Distinguishes directly observed facts from inferred ones.
    pub kind: CertaintyKind,
    /// Confidence score in percent, bounded to `0..=100`.
    pub confidence: ConfidenceScore,
}

impl Certainty {
    /// Certainty metadata for directly observed facts.
    pub const fn observed() -> Self {
        Self {
            kind: CertaintyKind::Observed,
            confidence: ConfidenceScore::MAX,
        }
    }

    /// Certainty metadata for inferred facts.
    pub const fn inferred(confidence: ConfidenceScore) -> Self {
        Self {
            kind: CertaintyKind::Inferred,
            confidence,
        }
    }
}

/// Whether a fact was directly observed or inferred from indirect evidence.
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidConfidenceScore(pub u8);

impl fmt::Display for InvalidConfidenceScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "confidence score must be between 0 and 100, got {}",
            self.0
        )
    }
}

impl Error for InvalidConfidenceScore {}

/// Evidence attached to inferred capability output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// Main source of evidence for this fact or recommendation.
    pub source: ProvenanceSource,
    /// Optional human-readable explanation, such as the applied heuristic.
    pub detail: Option<String>,
}

/// High-level source categories for capability evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceSource {
    Heuristic,
    SymbolResolution,
    SyntaxTree,
    WorkingTree,
}

/// Direction of a relationship relative to the focal entity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RelationshipDirection {
    Inbound,
    Outbound,
}

/// Relationship kinds exposed by the agent capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentRelationshipKind {
    Calls,
    Contains,
    Defines,
    Implements,
    Imports,
    References,
}

/// Related entity plus the evidence that connects it to the focal entity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RelatedEntity {
    /// Direction of the relationship relative to the focal entity.
    pub direction: RelationshipDirection,
    /// Meaning of the relationship.
    pub relationship_kind: AgentRelationshipKind,
    /// Entity connected by the relationship.
    pub entity: AgentEntity,
    /// Short explanation suitable for summaries and previews.
    pub summary: Option<String>,
    /// Certainty metadata for this relationship.
    pub certainty: Certainty,
    /// Provenance metadata for this relationship.
    pub provenance: Provenance,
}

/// Selector used when a capability needs one focal entity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EntitySelector {
    /// Select by stable snapshot-local identifier.
    Id { id: String },
    /// Select by fully qualified or otherwise disambiguated name.
    QualifiedName { qualified_name: String },
}

/// Request parameters for concept discovery.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConceptDiscoveryRequest {
    /// Free-form concept or keyword query.
    pub query: String,
    /// Maximum number of matches to return.
    pub limit: u32,
}

/// Request parameters for one entity-oriented briefing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntityBriefingRequest {
    /// Entity to brief.
    pub entity: EntitySelector,
    /// Maximum related entities to return in each direction.
    pub relationship_limit: u32,
}

/// Request parameters for blast-radius estimation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlastRadiusRequest {
    /// Entity whose downstream impact should be estimated.
    pub entity: EntitySelector,
    /// Maximum traversal depth for indirect impact expansion.
    pub max_depth: u32,
}

/// Which working-tree changes should be inspected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeltaScope {
    Staged,
    Unstaged,
    All,
}

/// Request parameters for working-tree delta impact scanning.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeltaImpactScanRequest {
    /// Change scope to inspect.
    pub scope: DeltaScope,
}

/// Request parameters for coordinated rename planning.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenamePlanningRequest {
    /// Entity targeted for rename planning.
    pub entity: EntitySelector,
    /// Replacement identifier to preview.
    pub new_name: String,
}

/// Supported dialects for advanced structural queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StructuralQueryDialect {
    Pattern,
    Graph,
}

/// Request parameters for advanced structural queries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructuralQueryRequest {
    /// Query dialect used to interpret `query`.
    pub dialect: StructuralQueryDialect,
    /// Query text supplied by the caller.
    pub query: String,
    /// Maximum number of matches to return.
    pub limit: u32,
}

pub type ConceptDiscoveryResponse = CapabilityResponse<ConceptDiscoveryResult>;
pub type EntityBriefingResponse = CapabilityResponse<EntityBriefingResult>;
pub type BlastRadiusResponse = CapabilityResponse<BlastRadiusResult>;
pub type DeltaImpactScanResponse = CapabilityResponse<DeltaImpactScanResult>;
pub type RenamePlanningResponse = CapabilityResponse<RenamePlanningResult>;
pub type StructuralQueryResponse = CapabilityResponse<StructuralQueryResult>;

/// Ordered concept-discovery result list.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConceptDiscoveryResult {
    /// Matches sorted into deterministic presentation order.
    pub matches: Vec<ConceptMatch>,
}

/// One concept-discovery match.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConceptMatch {
    /// Matched semantic entity.
    pub entity: AgentEntity,
    /// Short explanation of why this entity matches the query.
    pub summary: String,
    /// Relationship preview attached to the match.
    pub related_entities: Vec<RelatedEntity>,
}

/// Detailed briefing for one focal entity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EntityBriefingResult {
    /// Entity being described.
    pub entity: AgentEntity,
    /// Summary suitable for panel and agent output.
    pub summary: String,
    /// Inbound and outbound relationship context for the entity.
    pub related_entities: Vec<RelatedEntity>,
}

/// Estimated impact surface for a proposed change.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BlastRadiusResult {
    /// Immediate dependents or directly connected impact targets.
    pub direct_impacts: Vec<ImpactTarget>,
    /// Transitively affected entities beyond the first hop.
    pub indirect_impacts: Vec<ImpactTarget>,
    /// References that could not be resolved confidently enough to place.
    pub unresolved_references: Vec<UnresolvedReference>,
}

/// One impacted entity in a blast-radius estimate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImpactTarget {
    /// Entity affected by the proposed change.
    pub entity: AgentEntity,
    /// Number of hops from the changed entity.
    pub depth: u32,
    /// Optional human-readable explanation of the impact.
    pub reason: Option<String>,
    /// Certainty metadata for this estimate.
    pub certainty: Certainty,
    /// Provenance metadata for this estimate.
    pub provenance: Provenance,
}

/// Safe representation of an unresolved dependency edge.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UnresolvedReference {
    /// Human-readable description of the unresolved edge.
    pub description: String,
    /// Certainty metadata for the unresolved observation.
    pub certainty: Certainty,
    /// Provenance metadata for the unresolved observation.
    pub provenance: Provenance,
}

/// Whether an analysis completed fully or returned a degraded result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnalysisCompleteness {
    Complete,
    Partial,
}

/// Working-tree impact scan result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeltaImpactScanResult {
    /// Whether the scan covered every requested change successfully.
    pub completeness: AnalysisCompleteness,
    /// Changed files linked to impacted entities.
    pub file_impacts: Vec<FileImpact>,
}

/// Impact summary for one changed file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FileImpact {
    /// Workspace-relative path for the changed file.
    pub path: String,
    /// Entities likely affected by this file change.
    pub impacted_entities: Vec<ImpactTarget>,
}

/// Rename preview result split by confidence bucket.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenamePlanningResult {
    /// Preview edits that are safe enough to apply by default.
    pub high_confidence_edits: Vec<RenameEdit>,
    /// Preview edits that require manual review.
    pub low_confidence_edits: Vec<RenameEdit>,
    /// Conflicts or blockers that prevent a safe rename.
    pub conflicts: Vec<RenameConflict>,
}

/// One preview edit emitted by rename planning.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenameEdit {
    /// Workspace-relative edit location.
    pub location: EntityLocation,
    /// Replacement text proposed for this edit.
    pub replacement: String,
    /// Optional explanation for the proposed edit.
    pub reason: Option<String>,
    /// Certainty metadata for this edit.
    pub certainty: Certainty,
    /// Provenance metadata for this edit.
    pub provenance: Provenance,
}

/// One conflict discovered during rename planning.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RenameConflict {
    /// Workspace-relative location related to the conflict, when available.
    pub location: Option<EntityLocation>,
    /// Human-readable explanation of the conflict.
    pub message: String,
}

/// Result set for advanced structural queries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructuralQueryResult {
    /// Query matches in deterministic presentation order.
    pub matches: Vec<StructuralQueryMatch>,
}

/// One advanced structural-query match.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructuralQueryMatch {
    /// Matched entity or anchor.
    pub entity: AgentEntity,
    /// Short explanation of why the query matched.
    pub summary: String,
    /// Certainty metadata for this query match.
    pub certainty: Certainty,
    /// Provenance metadata for this query match.
    pub provenance: Provenance,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        AgentCapabilityRequest, AgentCapabilityResponse, AgentEntity,
        AgentEntityKind, AgentRelationshipKind, BlastRadiusResult, CapabilityError,
        CapabilityErrorCode, CapabilityResponse, CapabilityTimeout, Certainty,
        CertaintyKind, ConceptDiscoveryRequest, ConfidenceScore, DeltaScope,
        EntityBriefingRequest, EntityLocation, EntitySelector, ImpactTarget,
        InvalidConfidenceScore, Provenance, ProvenanceSource, RelatedEntity,
        RelationshipDirection, RenamePlanningRequest, StructuralQueryDialect,
        StructuralQueryMatch, StructuralQueryRequest, StructuralQueryResult,
        TextPoint, TextSpan, UnresolvedReference,
    };

    #[test]
    fn request_union_exposes_all_six_capabilities_with_stable_tags() {
        let requests = [
            AgentCapabilityRequest::ConceptDiscovery(ConceptDiscoveryRequest {
                query: "workspace graph".to_string(),
                limit: 5,
            }),
            AgentCapabilityRequest::EntityBriefing(EntityBriefingRequest {
                entity: EntitySelector::Id {
                    id: "entity:1".to_string(),
                },
                relationship_limit: 8,
            }),
            AgentCapabilityRequest::BlastRadiusEstimation(
                super::BlastRadiusRequest {
                    entity: EntitySelector::QualifiedName {
                        qualified_name: "crate::module::item".to_string(),
                    },
                    max_depth: 3,
                },
            ),
            AgentCapabilityRequest::DeltaImpactScan(super::DeltaImpactScanRequest {
                scope: DeltaScope::All,
            }),
            AgentCapabilityRequest::RenamePlanning(RenamePlanningRequest {
                entity: EntitySelector::Id {
                    id: "entity:2".to_string(),
                },
                new_name: "renamed_item".to_string(),
            }),
            AgentCapabilityRequest::StructuralQuery(StructuralQueryRequest {
                dialect: StructuralQueryDialect::Graph,
                query: "match function callers".to_string(),
                limit: 10,
            }),
        ];

        let capabilities = requests
            .into_iter()
            .map(|request| {
                serde_json::to_value(request).unwrap()["capability"].clone()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            capabilities,
            vec![
                json!("concept-discovery"),
                json!("entity-briefing"),
                json!("blast-radius-estimation"),
                json!("delta-impact-scan"),
                json!("rename-planning"),
                json!("structural-query"),
            ]
        );
    }

    #[test]
    fn timeout_and_error_envelopes_are_standardized_across_operations() {
        let timeout =
            AgentCapabilityResponse::StructuralQuery(CapabilityResponse::Timeout {
                timeout: CapabilityTimeout {
                    limit_ms: 200,
                    elapsed_ms: 200,
                },
                partial_result: Some(StructuralQueryResult {
                    matches: vec![StructuralQueryMatch {
                        entity: sample_entity(),
                        summary: "resolved one match before timeout".to_string(),
                        certainty: Certainty::inferred(
                            ConfidenceScore::new(82).unwrap(),
                        ),
                        provenance: Provenance {
                            source: ProvenanceSource::Heuristic,
                            detail: Some(
                                "ranked from partial traversal".to_string(),
                            ),
                        },
                    }],
                }),
            });
        let error =
            AgentCapabilityResponse::ConceptDiscovery(CapabilityResponse::Error {
                error: CapabilityError {
                    code: CapabilityErrorCode::SnapshotUnavailable,
                    message: "no workspace snapshot is available".to_string(),
                    retryable: true,
                },
            });

        let timeout_value = serde_json::to_value(timeout).unwrap();
        let error_value = serde_json::to_value(error).unwrap();

        assert_eq!(timeout_value["response"]["status"], json!("timeout"));
        assert_eq!(
            timeout_value["response"]["timeout"],
            json!({
                "limit_ms": 200,
                "elapsed_ms": 200
            })
        );
        assert_eq!(error_value["response"]["status"], json!("error"));
        assert_eq!(
            error_value["response"]["error"],
            json!({
                "code": "snapshot-unavailable",
                "message": "no workspace snapshot is available",
                "retryable": true
            })
        );
    }

    #[test]
    fn timeout_envelope_preserves_partial_result_payload() {
        let response =
            AgentCapabilityResponse::StructuralQuery(CapabilityResponse::Timeout {
                timeout: CapabilityTimeout {
                    limit_ms: 200,
                    elapsed_ms: 200,
                },
                partial_result: Some(StructuralQueryResult {
                    matches: vec![StructuralQueryMatch {
                        entity: sample_entity(),
                        summary: "resolved one match before timeout".to_string(),
                        certainty: Certainty::inferred(
                            ConfidenceScore::new(82).unwrap(),
                        ),
                        provenance: Provenance {
                            source: ProvenanceSource::Heuristic,
                            detail: Some(
                                "ranked from partial traversal".to_string(),
                            ),
                        },
                    }],
                }),
            });

        let value = serde_json::to_value(response).unwrap();

        assert_eq!(
            value["response"]["partial_result"]["matches"][0]["entity"]["id"],
            json!("entity:1")
        );
        assert_eq!(
            value["response"]["partial_result"]["matches"][0]["certainty"],
            json!({
                "kind": "inferred",
                "confidence": 82
            })
        );
    }

    #[test]
    fn capability_requests_ignore_unknown_future_fields() {
        let request: AgentCapabilityRequest = serde_json::from_value(json!({
            "capability": "entity-briefing",
            "params": {
                "entity": {
                    "kind": "id",
                    "id": "entity:9"
                },
                "relationship_limit": 4,
                "future_flag": true
            },
            "future_outer": "ignored"
        }))
        .unwrap();

        assert_eq!(
            request,
            AgentCapabilityRequest::EntityBriefing(EntityBriefingRequest {
                entity: EntitySelector::Id {
                    id: "entity:9".to_string(),
                },
                relationship_limit: 4,
            })
        );
    }

    #[test]
    fn capability_responses_ignore_unknown_future_fields() {
        let response: AgentCapabilityResponse = serde_json::from_value(json!({
            "capability": "blast-radius-estimation",
            "response": {
                "status": "success",
                "result": {
                    "direct_impacts": [
                        {
                            "entity": {
                                "id": "entity:3",
                                "kind": "function",
                                "name": "render_panel",
                                "qualified_name": null,
                                "location": null
                            },
                            "depth": 1,
                            "reason": "calls the renamed function",
                            "certainty": {
                                "kind": "observed",
                                "confidence": 100
                            },
                            "provenance": {
                                "source": "syntax-tree",
                                "detail": null
                            },
                            "future_field": "ignored"
                        }
                    ],
                    "indirect_impacts": [],
                    "unresolved_references": [],
                    "future_result_field": "ignored"
                },
                "future_response_field": "ignored"
            }
        }))
        .unwrap();

        assert_eq!(
            response,
            AgentCapabilityResponse::BlastRadiusEstimation(
                CapabilityResponse::Success {
                    result: BlastRadiusResult {
                        direct_impacts: vec![ImpactTarget {
                            entity: AgentEntity {
                                id: "entity:3".to_string(),
                                kind: AgentEntityKind::Function,
                                name: "render_panel".to_string(),
                                qualified_name: None,
                                location: None,
                            },
                            depth: 1,
                            reason: Some("calls the renamed function".to_string()),
                            certainty: Certainty::observed(),
                            provenance: Provenance {
                                source: ProvenanceSource::SyntaxTree,
                                detail: None,
                            },
                        }],
                        indirect_impacts: Vec::new(),
                        unresolved_references: Vec::new(),
                    },
                }
            )
        );
    }

    #[test]
    fn certainty_and_provenance_fields_serialize_in_schema_payloads() {
        let response = AgentCapabilityResponse::BlastRadiusEstimation(
            CapabilityResponse::Success {
                result: BlastRadiusResult {
                    direct_impacts: vec![ImpactTarget {
                        entity: sample_entity(),
                        depth: 1,
                        reason: Some("imports the changed module".to_string()),
                        certainty: Certainty::inferred(
                            ConfidenceScore::new(74).unwrap(),
                        ),
                        provenance: Provenance {
                            source: ProvenanceSource::SymbolResolution,
                            detail: Some("resolved from import graph".to_string()),
                        },
                    }],
                    indirect_impacts: vec![ImpactTarget {
                        entity: AgentEntity {
                            id: "entity:2".to_string(),
                            kind: AgentEntityKind::Test,
                            name: "render_panel_smoke".to_string(),
                            qualified_name: None,
                            location: None,
                        },
                        depth: 2,
                        reason: Some("covers a direct dependent".to_string()),
                        certainty: Certainty::inferred(
                            ConfidenceScore::new(61).unwrap(),
                        ),
                        provenance: Provenance {
                            source: ProvenanceSource::Heuristic,
                            detail: Some("test naming heuristic".to_string()),
                        },
                    }],
                    unresolved_references: vec![UnresolvedReference {
                        description: "dynamic dispatch target could not be expanded"
                            .to_string(),
                        certainty: Certainty {
                            kind: CertaintyKind::Inferred,
                            confidence: ConfidenceScore::new(35).unwrap(),
                        },
                        provenance: Provenance {
                            source: ProvenanceSource::Heuristic,
                            detail: Some("trait object receiver".to_string()),
                        },
                    }],
                },
            },
        );

        let value = serde_json::to_value(response).unwrap();
        let impact = &value["response"]["result"]["direct_impacts"][0];

        assert_eq!(
            impact["certainty"],
            json!({"kind": "inferred", "confidence": 74})
        );
        assert_eq!(
            impact["provenance"],
            json!({
                "source": "symbol-resolution",
                "detail": "resolved from import graph"
            })
        );
    }

    #[test]
    fn confidence_score_rejects_values_above_one_hundred() {
        assert_eq!(ConfidenceScore::new(101), Err(InvalidConfidenceScore(101)));
    }

    fn sample_entity() -> AgentEntity {
        AgentEntity {
            id: "entity:1".to_string(),
            kind: AgentEntityKind::Function,
            name: "build_snapshot".to_string(),
            qualified_name: Some("crate::snapshot::build_snapshot".to_string()),
            location: Some(EntityLocation {
                path: "src/snapshot.rs".to_string(),
                span: Some(TextSpan {
                    start: TextPoint {
                        line: 12,
                        column: 4,
                    },
                    end: TextPoint {
                        line: 28,
                        column: 1,
                    },
                }),
            }),
        }
    }

    #[test]
    fn related_entities_include_documented_certainty_and_provenance() {
        let related = RelatedEntity {
            direction: RelationshipDirection::Outbound,
            relationship_kind: AgentRelationshipKind::Calls,
            entity: sample_entity(),
            summary: Some("invoked during concept ranking".to_string()),
            certainty: Certainty::observed(),
            provenance: Provenance {
                source: ProvenanceSource::SyntaxTree,
                detail: None,
            },
        };

        let value = serde_json::to_value(related).unwrap();

        assert_eq!(value["certainty"]["kind"], json!("observed"));
        assert_eq!(value["provenance"]["source"], json!("syntax-tree"));
    }
}
