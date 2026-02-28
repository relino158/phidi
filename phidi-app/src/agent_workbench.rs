use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use floem::{
    ext_event::create_ext_action,
    reactive::{RwSignal, Scope, SignalUpdate},
};
use phidi_rpc::agent::{
    AgentCapabilityRequest, AgentCapabilityResponse, AgentEntity, AgentEntityKind,
    AgentRelationshipKind, AnalysisCompleteness, BlastRadiusRequest,
    BlastRadiusResult, CapabilityError, CapabilityErrorCode, CapabilityResponse,
    CapabilityTimeout, Certainty, ConceptDiscoveryRequest, ConceptDiscoveryResult,
    DeltaImpactScanRequest, DeltaImpactScanResult, DeltaScope,
    EntityBriefingRequest, EntityLocation, EntitySelector, FileImpact, ImpactTarget,
    Provenance, ProvenanceSource, RelatedEntity, RelationshipDirection,
    RenameConflict, RenameEdit, RenamePlanningRequest, RenamePlanningResult,
    StructuralQueryDialect, StructuralQueryMatch, StructuralQueryRequest,
    StructuralQueryResult, TextPoint, TextSpan, UnresolvedReference,
};

use crate::command::PhidiWorkbenchCommand;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AgentCapabilityKind {
    ConceptDiscovery,
    EntityBriefing,
    BlastRadiusEstimation,
    DeltaImpactScan,
    RenamePlanning,
    StructuralQuery,
}

impl AgentCapabilityKind {
    pub const ALL: [Self; 6] = [
        Self::ConceptDiscovery,
        Self::EntityBriefing,
        Self::BlastRadiusEstimation,
        Self::DeltaImpactScan,
        Self::RenamePlanning,
        Self::StructuralQuery,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::ConceptDiscovery => "Concept Discovery",
            Self::EntityBriefing => "Entity Briefing",
            Self::BlastRadiusEstimation => "Blast Radius Estimation",
            Self::DeltaImpactScan => "Delta Impact Scan",
            Self::RenamePlanning => "Rename Planning",
            Self::StructuralQuery => "Structural Query",
        }
    }

    pub fn command_message(self) -> &'static str {
        match self {
            Self::ConceptDiscovery => "Atlas: Run Concept Discovery",
            Self::EntityBriefing => "Atlas: Run Entity Briefing",
            Self::BlastRadiusEstimation => "Atlas: Run Blast Radius Estimation",
            Self::DeltaImpactScan => "Atlas: Run Delta Impact Scan",
            Self::RenamePlanning => "Atlas: Run Rename Planning",
            Self::StructuralQuery => "Atlas: Run Structural Query",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityCommandSpec {
    pub capability: AgentCapabilityKind,
    pub command: PhidiWorkbenchCommand,
}

pub const fn capability_command_specs() -> [CapabilityCommandSpec; 6] {
    [
        CapabilityCommandSpec {
            capability: AgentCapabilityKind::ConceptDiscovery,
            command: PhidiWorkbenchCommand::AtlasConceptDiscovery,
        },
        CapabilityCommandSpec {
            capability: AgentCapabilityKind::EntityBriefing,
            command: PhidiWorkbenchCommand::AtlasEntityBriefing,
        },
        CapabilityCommandSpec {
            capability: AgentCapabilityKind::BlastRadiusEstimation,
            command: PhidiWorkbenchCommand::AtlasBlastRadiusEstimation,
        },
        CapabilityCommandSpec {
            capability: AgentCapabilityKind::DeltaImpactScan,
            command: PhidiWorkbenchCommand::AtlasDeltaImpactScan,
        },
        CapabilityCommandSpec {
            capability: AgentCapabilityKind::RenamePlanning,
            command: PhidiWorkbenchCommand::AtlasRenamePlanning,
        },
        CapabilityCommandSpec {
            capability: AgentCapabilityKind::StructuralQuery,
            command: PhidiWorkbenchCommand::AtlasStructuralQuery,
        },
    ]
}

pub fn capability_from_command(
    command: PhidiWorkbenchCommand,
) -> Option<AgentCapabilityKind> {
    capability_command_specs()
        .into_iter()
        .find(|spec| spec.command == command)
        .map(|spec| spec.capability)
}

pub fn default_request_for(
    capability: AgentCapabilityKind,
) -> AgentCapabilityRequest {
    match capability {
        AgentCapabilityKind::ConceptDiscovery => {
            AgentCapabilityRequest::ConceptDiscovery(ConceptDiscoveryRequest {
                query: "workspace graph".to_string(),
                limit: 5,
            })
        }
        AgentCapabilityKind::EntityBriefing => {
            AgentCapabilityRequest::EntityBriefing(EntityBriefingRequest {
                entity: EntitySelector::QualifiedName {
                    qualified_name: "crate::snapshot::build_snapshot".to_string(),
                },
                relationship_limit: 6,
            })
        }
        AgentCapabilityKind::BlastRadiusEstimation => {
            AgentCapabilityRequest::BlastRadiusEstimation(BlastRadiusRequest {
                entity: EntitySelector::QualifiedName {
                    qualified_name: "crate::panel::render_panel".to_string(),
                },
                max_depth: 3,
            })
        }
        AgentCapabilityKind::DeltaImpactScan => {
            AgentCapabilityRequest::DeltaImpactScan(DeltaImpactScanRequest {
                scope: DeltaScope::All,
            })
        }
        AgentCapabilityKind::RenamePlanning => {
            AgentCapabilityRequest::RenamePlanning(RenamePlanningRequest {
                entity: EntitySelector::Id {
                    id: "entity:rename-target".to_string(),
                },
                new_name: "atlas_panel".to_string(),
            })
        }
        AgentCapabilityKind::StructuralQuery => {
            AgentCapabilityRequest::StructuralQuery(StructuralQueryRequest {
                dialect: StructuralQueryDialect::Graph,
                query: "match panel callers".to_string(),
                limit: 8,
            })
        }
    }
}

pub fn mocked_response_for(
    request: &AgentCapabilityRequest,
) -> AgentCapabilityResponse {
    match request {
        AgentCapabilityRequest::ConceptDiscovery(_) => {
            AgentCapabilityResponse::ConceptDiscovery(CapabilityResponse::Success {
                result: ConceptDiscoveryResult {
                    matches: vec![
                        concept_match(
                            "entity:panel",
                            "render_panel",
                            "crate::panel::render_panel",
                            "Matches Atlas panel wiring and typed result rendering.",
                        ),
                        concept_match(
                            "entity:snapshot",
                            "build_snapshot",
                            "crate::snapshot::build_snapshot",
                            "Connects snapshot freshness metadata into Atlas flows.",
                        ),
                    ],
                },
            })
        }
        AgentCapabilityRequest::EntityBriefing(_) => {
            AgentCapabilityResponse::EntityBriefing(CapabilityResponse::Error {
                error: CapabilityError {
                    code: CapabilityErrorCode::SnapshotUnavailable,
                    message: "No workspace snapshot is available yet.".to_string(),
                    retryable: true,
                },
            })
        }
        AgentCapabilityRequest::BlastRadiusEstimation(_) => {
            AgentCapabilityResponse::BlastRadiusEstimation(
                CapabilityResponse::Success {
                    result: BlastRadiusResult {
                        direct_impacts: vec![impact_target(
                            "entity:panel-view",
                            "panel_view",
                            "crate::panel::view::panel_view",
                            1,
                            Some("Directly renders the panel tab body."),
                            Certainty::observed(),
                            ProvenanceSource::SyntaxTree,
                        )],
                        indirect_impacts: vec![impact_target(
                            "entity:window-tab",
                            "run_workbench_command",
                            "crate::window_tab::run_workbench_command",
                            2,
                            Some("Dispatches the command that opens the panel."),
                            inferred(78),
                            ProvenanceSource::SymbolResolution,
                        )],
                        unresolved_references: vec![UnresolvedReference {
                            description:
                                "Dynamic panel focus transitions require runtime inspection."
                                    .to_string(),
                            certainty: inferred(41),
                            provenance: provenance(
                                ProvenanceSource::Heuristic,
                                Some("focus routing inferred from panel kind".to_string()),
                            ),
                        }],
                    },
                },
            )
        }
        AgentCapabilityRequest::DeltaImpactScan(_) => {
            AgentCapabilityResponse::DeltaImpactScan(CapabilityResponse::Success {
                result: DeltaImpactScanResult {
                    completeness: AnalysisCompleteness::Partial,
                    file_impacts: vec![
                        FileImpact {
                            path: "phidi-app/src/window_tab.rs".to_string(),
                            impacted_entities: vec![impact_target(
                                "entity:run-command",
                                "run_workbench_command",
                                "crate::window_tab::run_workbench_command",
                                1,
                                Some("Owns workbench command dispatch."),
                                Certainty::observed(),
                                ProvenanceSource::WorkingTree,
                            )],
                        },
                        FileImpact {
                            path: "phidi-app/src/panel/view.rs".to_string(),
                            impacted_entities: vec![impact_target(
                                "entity:panel-tabs",
                                "panel_view",
                                "crate::panel::view::panel_view",
                                1,
                                Some("Hosts the panel tab body selection."),
                                Certainty::observed(),
                                ProvenanceSource::WorkingTree,
                            )],
                        },
                    ],
                },
            })
        }
        AgentCapabilityRequest::RenamePlanning(request) => {
            AgentCapabilityResponse::RenamePlanning(CapabilityResponse::Success {
                result: RenamePlanningResult {
                    high_confidence_edits: vec![RenameEdit {
                        location: sample_location("phidi-app/src/agent_workbench.rs", 42, 8, 42, 20),
                        replacement: request.new_name.clone(),
                        reason: Some("Capability label and panel copy stay aligned.".to_string()),
                        certainty: Certainty::observed(),
                        provenance: provenance(ProvenanceSource::SyntaxTree, None),
                    }],
                    low_confidence_edits: vec![RenameEdit {
                        location: sample_location("phidi-app/src/panel/agent_view.rs", 118, 16, 118, 28),
                        replacement: request.new_name.clone(),
                        reason: Some("UI label may need manual tone review.".to_string()),
                        certainty: inferred(67),
                        provenance: provenance(
                            ProvenanceSource::Heuristic,
                            Some("derived from repeated label text".to_string()),
                        ),
                    }],
                    conflicts: vec![RenameConflict {
                        location: Some(sample_location(
                            "phidi-app/src/command.rs",
                            430,
                            4,
                            430,
                            24,
                        )),
                        message: "A workbench command name already uses this display string."
                            .to_string(),
                    }],
                },
            })
        }
        AgentCapabilityRequest::StructuralQuery(_) => {
            AgentCapabilityResponse::StructuralQuery(CapabilityResponse::Timeout {
                timeout: CapabilityTimeout {
                    limit_ms: 350,
                    elapsed_ms: 350,
                },
                partial_result: Some(StructuralQueryResult {
                    matches: vec![StructuralQueryMatch {
                        entity: sample_entity(
                            "entity:panel",
                            "panel_view",
                            "crate::panel::view::panel_view",
                            Some(sample_location(
                                "phidi-app/src/panel/view.rs",
                                460,
                                1,
                                512,
                                1,
                            )),
                        ),
                        summary: "Matched the main panel body dispatcher before timeout."
                            .to_string(),
                        certainty: inferred(83),
                        provenance: provenance(
                            ProvenanceSource::SymbolResolution,
                            Some("partial graph traversal".to_string()),
                        ),
                    }],
                }),
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentRunState {
    Running,
    Complete { response: AgentCapabilityResponse },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRun {
    pub id: u64,
    pub capability: AgentCapabilityKind,
    pub request: AgentCapabilityRequest,
    pub summary: AgentRunSummary,
    pub state: AgentRunState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunSummary {
    pub title: String,
    pub status_label: String,
    pub detail: String,
}

impl AgentRunSummary {
    pub fn pending(capability: AgentCapabilityKind) -> Self {
        Self {
            title: capability.title().to_string(),
            status_label: "Running".to_string(),
            detail:
                "Queued on a background worker so the workbench stays responsive."
                    .to_string(),
        }
    }

    pub fn from_response(
        capability: AgentCapabilityKind,
        response: &AgentCapabilityResponse,
    ) -> Self {
        let (status_label, detail) = match response {
            AgentCapabilityResponse::ConceptDiscovery(
                CapabilityResponse::Success { result },
            ) => (
                "Success".to_string(),
                format!(
                    "{} concept matches ranked for review.",
                    result.matches.len()
                ),
            ),
            AgentCapabilityResponse::EntityBriefing(
                CapabilityResponse::Success { result },
            ) => (
                "Success".to_string(),
                format!(
                    "Briefed {} related entities.",
                    result.related_entities.len()
                ),
            ),
            AgentCapabilityResponse::BlastRadiusEstimation(
                CapabilityResponse::Success { result },
            ) => (
                "Success".to_string(),
                format!(
                    "{} direct and {} indirect impacts estimated.",
                    result.direct_impacts.len(),
                    result.indirect_impacts.len()
                ),
            ),
            AgentCapabilityResponse::DeltaImpactScan(
                CapabilityResponse::Success { result },
            ) => (
                "Success".to_string(),
                format!(
                    "{} changed files mapped with {:?} completeness.",
                    result.file_impacts.len(),
                    result.completeness
                ),
            ),
            AgentCapabilityResponse::RenamePlanning(
                CapabilityResponse::Success { result },
            ) => (
                "Success".to_string(),
                format!(
                    "{} high-confidence edits, {} conflicts.",
                    result.high_confidence_edits.len(),
                    result.conflicts.len()
                ),
            ),
            AgentCapabilityResponse::StructuralQuery(
                CapabilityResponse::Success { result },
            ) => (
                "Success".to_string(),
                format!("{} structural matches returned.", result.matches.len()),
            ),
            AgentCapabilityResponse::ConceptDiscovery(
                CapabilityResponse::Timeout {
                    timeout,
                    partial_result,
                },
            ) => timeout_detail(
                timeout,
                partial_result.as_ref().map(|r| r.matches.len()),
            ),
            AgentCapabilityResponse::EntityBriefing(
                CapabilityResponse::Timeout {
                    timeout,
                    partial_result,
                },
            ) => timeout_detail(
                timeout,
                partial_result.as_ref().map(|r| r.related_entities.len()),
            ),
            AgentCapabilityResponse::BlastRadiusEstimation(
                CapabilityResponse::Timeout {
                    timeout,
                    partial_result,
                },
            ) => timeout_detail(
                timeout,
                partial_result
                    .as_ref()
                    .map(|r| r.direct_impacts.len() + r.indirect_impacts.len()),
            ),
            AgentCapabilityResponse::DeltaImpactScan(
                CapabilityResponse::Timeout {
                    timeout,
                    partial_result,
                },
            ) => timeout_detail(
                timeout,
                partial_result.as_ref().map(|r| r.file_impacts.len()),
            ),
            AgentCapabilityResponse::RenamePlanning(
                CapabilityResponse::Timeout {
                    timeout,
                    partial_result,
                },
            ) => timeout_detail(
                timeout,
                partial_result.as_ref().map(|r| {
                    r.high_confidence_edits.len() + r.low_confidence_edits.len()
                }),
            ),
            AgentCapabilityResponse::StructuralQuery(
                CapabilityResponse::Timeout {
                    timeout,
                    partial_result,
                },
            ) => timeout_detail(
                timeout,
                partial_result.as_ref().map(|r| r.matches.len()),
            ),
            AgentCapabilityResponse::ConceptDiscovery(
                CapabilityResponse::Error { error },
            )
            | AgentCapabilityResponse::EntityBriefing(CapabilityResponse::Error {
                error,
            })
            | AgentCapabilityResponse::BlastRadiusEstimation(
                CapabilityResponse::Error { error },
            )
            | AgentCapabilityResponse::DeltaImpactScan(
                CapabilityResponse::Error { error },
            )
            | AgentCapabilityResponse::RenamePlanning(CapabilityResponse::Error {
                error,
            })
            | AgentCapabilityResponse::StructuralQuery(
                CapabilityResponse::Error { error },
            ) => (
                "Error".to_string(),
                format!("{}: {}", error_code_label(error.code), error.message),
            ),
        };

        Self {
            title: capability.title().to_string(),
            status_label,
            detail,
        }
    }
}

pub fn request_detail_lines(request: &AgentCapabilityRequest) -> Vec<String> {
    match request {
        AgentCapabilityRequest::ConceptDiscovery(request) => vec![
            format!("Query: {}", request.query),
            format!("Limit: {}", request.limit),
        ],
        AgentCapabilityRequest::EntityBriefing(request) => vec![
            format!("Entity: {}", selector_label(&request.entity)),
            format!("Relationship limit: {}", request.relationship_limit),
        ],
        AgentCapabilityRequest::BlastRadiusEstimation(request) => vec![
            format!("Entity: {}", selector_label(&request.entity)),
            format!("Max depth: {}", request.max_depth),
        ],
        AgentCapabilityRequest::DeltaImpactScan(request) => {
            vec![format!("Scope: {:?}", request.scope)]
        }
        AgentCapabilityRequest::RenamePlanning(request) => vec![
            format!("Entity: {}", selector_label(&request.entity)),
            format!("New name: {}", request.new_name),
        ],
        AgentCapabilityRequest::StructuralQuery(request) => vec![
            format!("Dialect: {:?}", request.dialect),
            format!("Query: {}", request.query),
            format!("Limit: {}", request.limit),
        ],
    }
}

pub fn response_detail_lines(response: &AgentCapabilityResponse) -> Vec<String> {
    match response {
        AgentCapabilityResponse::ConceptDiscovery(CapabilityResponse::Success {
            result,
        }) => result
            .matches
            .iter()
            .map(|item| format!("{}: {}", item.entity.name, item.summary))
            .collect(),
        AgentCapabilityResponse::EntityBriefing(CapabilityResponse::Success {
            result,
        }) => {
            let mut lines = vec![result.summary.clone()];
            lines.extend(result.related_entities.iter().map(|item| {
                format!(
                    "{:?} {:?}: {}",
                    item.direction, item.relationship_kind, item.entity.name
                )
            }));
            lines
        }
        AgentCapabilityResponse::BlastRadiusEstimation(
            CapabilityResponse::Success { result },
        ) => {
            let mut lines = result
                .direct_impacts
                .iter()
                .map(|item| format!("Direct d{}: {}", item.depth, item.entity.name))
                .collect::<Vec<_>>();
            lines.extend(result.indirect_impacts.iter().map(|item| {
                format!("Indirect d{}: {}", item.depth, item.entity.name)
            }));
            lines.extend(
                result
                    .unresolved_references
                    .iter()
                    .map(|item| format!("Unresolved: {}", item.description)),
            );
            lines
        }
        AgentCapabilityResponse::DeltaImpactScan(CapabilityResponse::Success {
            result,
        }) => {
            let mut lines = vec![format!("Completeness: {:?}", result.completeness)];
            lines.extend(result.file_impacts.iter().map(|item| {
                format!(
                    "{} -> {} impacted entities",
                    item.path,
                    item.impacted_entities.len()
                )
            }));
            lines
        }
        AgentCapabilityResponse::RenamePlanning(CapabilityResponse::Success {
            result,
        }) => {
            let mut lines = result
                .high_confidence_edits
                .iter()
                .map(|item| format!("High: {}", location_label(&item.location)))
                .collect::<Vec<_>>();
            lines.extend(
                result.low_confidence_edits.iter().map(|item| {
                    format!("Review: {}", location_label(&item.location))
                }),
            );
            lines.extend(
                result
                    .conflicts
                    .iter()
                    .map(|item| format!("Conflict: {}", item.message)),
            );
            lines
        }
        AgentCapabilityResponse::StructuralQuery(CapabilityResponse::Success {
            result,
        }) => result
            .matches
            .iter()
            .map(|item| format!("{}: {}", item.entity.name, item.summary))
            .collect(),
        AgentCapabilityResponse::ConceptDiscovery(CapabilityResponse::Timeout {
            partial_result,
            ..
        }) => partial_timeout_lines(
            partial_result
                .as_ref()
                .map(|result| result.matches.len())
                .unwrap_or_default(),
        ),
        AgentCapabilityResponse::EntityBriefing(CapabilityResponse::Timeout {
            partial_result,
            ..
        }) => partial_timeout_lines(
            partial_result
                .as_ref()
                .map(|result| result.related_entities.len())
                .unwrap_or_default(),
        ),
        AgentCapabilityResponse::BlastRadiusEstimation(
            CapabilityResponse::Timeout { partial_result, .. },
        ) => partial_timeout_lines(
            partial_result
                .as_ref()
                .map(|result| {
                    result.direct_impacts.len() + result.indirect_impacts.len()
                })
                .unwrap_or_default(),
        ),
        AgentCapabilityResponse::DeltaImpactScan(CapabilityResponse::Timeout {
            partial_result,
            ..
        }) => partial_timeout_lines(
            partial_result
                .as_ref()
                .map(|result| result.file_impacts.len())
                .unwrap_or_default(),
        ),
        AgentCapabilityResponse::RenamePlanning(CapabilityResponse::Timeout {
            partial_result,
            ..
        }) => partial_timeout_lines(
            partial_result
                .as_ref()
                .map(|result| {
                    result.high_confidence_edits.len()
                        + result.low_confidence_edits.len()
                })
                .unwrap_or_default(),
        ),
        AgentCapabilityResponse::StructuralQuery(CapabilityResponse::Timeout {
            partial_result,
            ..
        }) => partial_timeout_lines(
            partial_result
                .as_ref()
                .map(|result| result.matches.len())
                .unwrap_or_default(),
        ),
        AgentCapabilityResponse::ConceptDiscovery(CapabilityResponse::Error {
            error,
        })
        | AgentCapabilityResponse::EntityBriefing(CapabilityResponse::Error {
            error,
        })
        | AgentCapabilityResponse::BlastRadiusEstimation(
            CapabilityResponse::Error { error },
        )
        | AgentCapabilityResponse::DeltaImpactScan(CapabilityResponse::Error {
            error,
        })
        | AgentCapabilityResponse::RenamePlanning(CapabilityResponse::Error {
            error,
        })
        | AgentCapabilityResponse::StructuralQuery(CapabilityResponse::Error {
            error,
        }) => vec![
            format!("Code: {}", error_code_label(error.code)),
            format!("Retryable: {}", error.retryable),
            error.message.clone(),
        ],
    }
}

fn timeout_detail(
    timeout: &CapabilityTimeout,
    partial_count: Option<usize>,
) -> (String, String) {
    let partial = partial_count
        .map(|count| format!("{count} partial result(s)"))
        .unwrap_or_else(|| "no partial results".to_string());
    (
        "Timed Out".to_string(),
        format!("Stopped after {} ms with {partial}.", timeout.elapsed_ms),
    )
}

fn partial_timeout_lines(count: usize) -> Vec<String> {
    vec![format!("Partial results available: {count}")]
}

fn selector_label(selector: &EntitySelector) -> String {
    match selector {
        EntitySelector::Id { id } => id.clone(),
        EntitySelector::QualifiedName { qualified_name } => qualified_name.clone(),
    }
}

fn location_label(location: &EntityLocation) -> String {
    match location.span {
        Some(span) => format!(
            "{}:{}:{}",
            location.path, span.start.line, span.start.column
        ),
        None => location.path.clone(),
    }
}

fn error_code_label(code: CapabilityErrorCode) -> &'static str {
    match code {
        CapabilityErrorCode::InvalidRequest => "invalid-request",
        CapabilityErrorCode::SnapshotUnavailable => "snapshot-unavailable",
        CapabilityErrorCode::SnapshotIncompatible => "snapshot-incompatible",
        CapabilityErrorCode::UnsupportedQuery => "unsupported-query",
        CapabilityErrorCode::Internal => "internal",
    }
}

#[derive(Clone)]
pub struct AgentWorkbenchData {
    next_run_id: Arc<AtomicU64>,
    pub active_run: RwSignal<Option<AgentRun>>,
    pub recent_runs: RwSignal<Vec<AgentRun>>,
}

impl AgentWorkbenchData {
    pub fn new(cx: Scope) -> Self {
        Self {
            next_run_id: Arc::new(AtomicU64::new(1)),
            active_run: cx.create_rw_signal(None),
            recent_runs: cx.create_rw_signal(Vec::new()),
        }
    }

    pub fn submit(&self, cx: Scope, capability: AgentCapabilityKind) {
        let run_id = self.next_run_id.fetch_add(1, Ordering::Relaxed);
        let request = default_request_for(capability);
        let pending_run = AgentRun {
            id: run_id,
            capability,
            request: request.clone(),
            summary: AgentRunSummary::pending(capability),
            state: AgentRunState::Running,
        };

        self.active_run.set(Some(pending_run.clone()));
        self.recent_runs.update(|runs| {
            runs.insert(0, pending_run);
            runs.truncate(12);
        });

        let active_run = self.active_run;
        let recent_runs = self.recent_runs;
        let completed_request = request.clone();
        let worker_request = request.clone();
        let send =
            create_ext_action(cx, move |response: AgentCapabilityResponse| {
                let finished_run = AgentRun {
                    id: run_id,
                    capability,
                    request: completed_request.clone(),
                    summary: AgentRunSummary::from_response(capability, &response),
                    state: AgentRunState::Complete { response },
                };

                active_run.update(|slot| {
                    if slot.as_ref().is_some_and(|run| run.id == run_id) {
                        *slot = Some(finished_run.clone());
                    }
                });
                recent_runs.update(|runs| {
                    if let Some(index) = runs.iter().position(|run| run.id == run_id)
                    {
                        runs[index] = finished_run.clone();
                    } else {
                        runs.insert(0, finished_run.clone());
                    }
                    runs.truncate(12);
                });
            });

        thread::Builder::new()
            .name(format!("AtlasCapability::{capability:?}"))
            .spawn(move || {
                thread::sleep(Duration::from_millis(mock_delay_ms(capability)));
                send(mocked_response_for(&worker_request));
            })
            .unwrap();
    }
}

fn mock_delay_ms(capability: AgentCapabilityKind) -> u64 {
    match capability {
        AgentCapabilityKind::ConceptDiscovery => 120,
        AgentCapabilityKind::EntityBriefing => 180,
        AgentCapabilityKind::BlastRadiusEstimation => 240,
        AgentCapabilityKind::DeltaImpactScan => 140,
        AgentCapabilityKind::RenamePlanning => 220,
        AgentCapabilityKind::StructuralQuery => 260,
    }
}

fn concept_match(
    id: &str,
    name: &str,
    qualified_name: &str,
    summary: &str,
) -> phidi_rpc::agent::ConceptMatch {
    phidi_rpc::agent::ConceptMatch {
        entity: sample_entity(
            id,
            name,
            qualified_name,
            Some(sample_location("src/mock.rs", 12, 4, 28, 1)),
        ),
        summary: summary.to_string(),
        related_entities: vec![RelatedEntity {
            direction: RelationshipDirection::Outbound,
            relationship_kind: AgentRelationshipKind::Calls,
            entity: sample_entity(
                "entity:helper",
                "render_summary",
                "crate::panel::render_summary",
                None,
            ),
            summary: Some("Feeds the panel summary list.".to_string()),
            certainty: Certainty::observed(),
            provenance: provenance(ProvenanceSource::SyntaxTree, None),
        }],
    }
}

fn impact_target(
    id: &str,
    name: &str,
    qualified_name: &str,
    depth: u32,
    reason: Option<&str>,
    certainty: Certainty,
    source: ProvenanceSource,
) -> ImpactTarget {
    ImpactTarget {
        entity: sample_entity(id, name, qualified_name, None),
        depth,
        reason: reason.map(str::to_string),
        certainty,
        provenance: provenance(source, None),
    }
}

fn sample_entity(
    id: &str,
    name: &str,
    qualified_name: &str,
    location: Option<EntityLocation>,
) -> AgentEntity {
    AgentEntity {
        id: id.to_string(),
        kind: AgentEntityKind::Function,
        name: name.to_string(),
        qualified_name: Some(qualified_name.to_string()),
        location,
    }
}

fn sample_location(
    path: &str,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
) -> EntityLocation {
    EntityLocation {
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
    }
}

fn provenance(source: ProvenanceSource, detail: Option<String>) -> Provenance {
    Provenance { source, detail }
}

fn inferred(confidence: u8) -> Certainty {
    Certainty::inferred(confidence.try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use phidi_rpc::agent::{
        AgentCapabilityRequest, AgentCapabilityResponse, CapabilityError,
        CapabilityErrorCode, CapabilityResponse,
    };

    use crate::command::PhidiWorkbenchCommand;

    use super::{
        AgentCapabilityKind, AgentRunSummary, capability_command_specs,
        default_request_for, mocked_response_for,
    };

    #[test]
    fn capability_commands_cover_all_six_operations() {
        let capabilities = capability_command_specs()
            .iter()
            .map(|spec| spec.capability)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            capabilities,
            BTreeSet::from([
                AgentCapabilityKind::ConceptDiscovery,
                AgentCapabilityKind::EntityBriefing,
                AgentCapabilityKind::BlastRadiusEstimation,
                AgentCapabilityKind::DeltaImpactScan,
                AgentCapabilityKind::RenamePlanning,
                AgentCapabilityKind::StructuralQuery,
            ])
        );

        let commands = capability_command_specs()
            .iter()
            .map(|spec| spec.command.to_string())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            commands,
            BTreeSet::from([
                PhidiWorkbenchCommand::AtlasConceptDiscovery.to_string(),
                PhidiWorkbenchCommand::AtlasEntityBriefing.to_string(),
                PhidiWorkbenchCommand::AtlasBlastRadiusEstimation.to_string(),
                PhidiWorkbenchCommand::AtlasDeltaImpactScan.to_string(),
                PhidiWorkbenchCommand::AtlasRenamePlanning.to_string(),
                PhidiWorkbenchCommand::AtlasStructuralQuery.to_string(),
            ])
        );
    }

    #[test]
    fn default_requests_use_stable_capability_tags() {
        let requests = AgentCapabilityKind::ALL
            .into_iter()
            .map(default_request_for)
            .collect::<Vec<_>>();

        let capability_tags = requests
            .iter()
            .map(|request| {
                serde_json::to_value(request).unwrap()["capability"].clone()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            capability_tags,
            vec![
                serde_json::json!("concept-discovery"),
                serde_json::json!("entity-briefing"),
                serde_json::json!("blast-radius-estimation"),
                serde_json::json!("delta-impact-scan"),
                serde_json::json!("rename-planning"),
                serde_json::json!("structural-query"),
            ]
        );

        assert!(matches!(
            requests[0],
            AgentCapabilityRequest::ConceptDiscovery(_)
        ));
    }

    #[test]
    fn mocked_responses_exercise_success_error_and_timeout_states() {
        let responses = AgentCapabilityKind::ALL
            .into_iter()
            .map(|capability| mocked_response_for(&default_request_for(capability)))
            .collect::<Vec<_>>();

        assert!(responses.iter().any(|response| matches!(
            response,
            AgentCapabilityResponse::ConceptDiscovery(
                CapabilityResponse::Success { .. }
            ) | AgentCapabilityResponse::BlastRadiusEstimation(
                CapabilityResponse::Success { .. }
            ) | AgentCapabilityResponse::DeltaImpactScan(
                CapabilityResponse::Success { .. }
            ) | AgentCapabilityResponse::RenamePlanning(
                CapabilityResponse::Success { .. }
            )
        )));
        assert!(responses.iter().any(|response| matches!(
            response,
            AgentCapabilityResponse::EntityBriefing(
                CapabilityResponse::Error { .. }
            )
        )));
        assert!(responses.iter().any(|response| matches!(
            response,
            AgentCapabilityResponse::StructuralQuery(
                CapabilityResponse::Timeout { .. }
            )
        )));
    }

    #[test]
    fn run_summary_reports_typed_status_details() {
        let error_summary = AgentRunSummary::from_response(
            AgentCapabilityKind::EntityBriefing,
            &AgentCapabilityResponse::EntityBriefing(CapabilityResponse::Error {
                error: CapabilityError {
                    code: CapabilityErrorCode::SnapshotUnavailable,
                    message: "snapshot missing".to_string(),
                    retryable: true,
                },
            }),
        );
        assert_eq!(error_summary.status_label, "Error");
        assert!(error_summary.detail.contains("snapshot-unavailable"));

        let timeout_summary = AgentRunSummary::from_response(
            AgentCapabilityKind::StructuralQuery,
            &mocked_response_for(&default_request_for(
                AgentCapabilityKind::StructuralQuery,
            )),
        );
        assert_eq!(timeout_summary.status_label, "Timed Out");
        assert!(timeout_summary.detail.contains("partial"));
    }
}
