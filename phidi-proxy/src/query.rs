use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::Path,
};

use phidi_core::semantic_map::{
    Certainty as SemanticCertainty, CertaintyKind as SemanticCertaintyKind,
    EntityKind, ProvenanceSource as SemanticProvenanceSource, RelationshipKind,
    SemanticEntity, SemanticRelationship, WorkspaceSnapshot,
};
use phidi_rpc::agent::{
    AgentEntity, AgentEntityKind, AgentRelationshipKind, AnalysisCompleteness,
    BlastRadiusRequest, BlastRadiusResult, CapabilityError, CapabilityErrorCode,
    CapabilityResponse, ConceptDiscoveryRequest, ConceptDiscoveryResult,
    ConceptMatch, DeltaImpactScanRequest, DeltaImpactScanResult,
    EntityBriefingRequest, EntityBriefingResult, EntityLocation, EntitySelector,
    FileImpact, ImpactTarget, Provenance, ProvenanceSource, RelatedEntity,
    RelationshipDirection, RenameConflict, RenameEdit, RenamePlanningRequest,
    RenamePlanningResult, TextPoint, TextSpan, UnresolvedReference,
};
use phidi_rpc::source_control::FileDiff;
use syn::{Expr, visit::Visit};

use crate::git_workspace::collect_working_tree_diffs;

const CONCEPT_PREVIEW_RELATIONSHIP_LIMIT: usize = 2;
const HIGH_CONFIDENCE_REASON: &str = "explicit path uniquely identifies the target";
const LOW_CONFIDENCE_REASON: &str =
    "locality breaks the tie, but the syntax has no explicit path";
const FUNCTION_CONFLICT_REASON: &str =
    "top-ranked candidates are tied on syntax-only evidence";
const METHOD_CONFLICT_REASON: &str =
    "method receiver types are unavailable in syntax-only ranking";
const DEFINITION_EDIT_REASON: &str = "rename target definition";
const HEURISTIC_FUNCTION_CONFIDENCE: u8 = 72;
const HEURISTIC_METHOD_CONFIDENCE: u8 = 64;

pub struct SnapshotQueryService<'a> {
    entities_by_id: BTreeMap<&'a str, &'a SemanticEntity>,
    entities_by_path: BTreeMap<&'a str, Vec<&'a SemanticEntity>>,
    inbound_by_target: BTreeMap<&'a str, Vec<&'a SemanticRelationship>>,
    outbound_by_source: BTreeMap<&'a str, Vec<&'a SemanticRelationship>>,
}

impl<'a> SnapshotQueryService<'a> {
    pub fn new(snapshot: &'a WorkspaceSnapshot) -> Self {
        let mut entities_by_id = BTreeMap::new();
        let mut entities_by_path = BTreeMap::<&str, Vec<&SemanticEntity>>::new();
        for entity in &snapshot.entities {
            entities_by_id.insert(entity.id.0.as_str(), entity);
            if let Some(location) = entity.location.as_ref() {
                entities_by_path
                    .entry(location.path.as_str())
                    .or_default()
                    .push(entity);
            }
        }

        let mut inbound_by_target =
            BTreeMap::<&str, Vec<&SemanticRelationship>>::new();
        let mut outbound_by_source =
            BTreeMap::<&str, Vec<&SemanticRelationship>>::new();
        for relationship in &snapshot.relationships {
            inbound_by_target
                .entry(relationship.target.0.as_str())
                .or_default()
                .push(relationship);
            outbound_by_source
                .entry(relationship.source.0.as_str())
                .or_default()
                .push(relationship);
        }

        for relationships in inbound_by_target.values_mut() {
            relationships.sort_by(|left, right| {
                relationship_kind_rank(left.kind)
                    .cmp(&relationship_kind_rank(right.kind))
                    .then_with(|| left.source.0.cmp(&right.source.0))
                    .then_with(|| {
                        certainty_rank(left.certainty)
                            .cmp(&certainty_rank(right.certainty))
                    })
                    .then_with(|| {
                        provenance_rank(left.provenance.source)
                            .cmp(&provenance_rank(right.provenance.source))
                    })
                    .then_with(|| {
                        left.provenance.detail.cmp(&right.provenance.detail)
                    })
            });
        }
        for relationships in outbound_by_source.values_mut() {
            relationships.sort_by(|left, right| {
                relationship_kind_rank(left.kind)
                    .cmp(&relationship_kind_rank(right.kind))
                    .then_with(|| left.target.0.cmp(&right.target.0))
                    .then_with(|| {
                        certainty_rank(left.certainty)
                            .cmp(&certainty_rank(right.certainty))
                    })
                    .then_with(|| {
                        provenance_rank(left.provenance.source)
                            .cmp(&provenance_rank(right.provenance.source))
                    })
                    .then_with(|| {
                        left.provenance.detail.cmp(&right.provenance.detail)
                    })
            });
        }
        for entities in entities_by_path.values_mut() {
            entities.sort_by(|left, right| compare_entities(left, right));
        }

        Self {
            entities_by_id,
            entities_by_path,
            inbound_by_target,
            outbound_by_source,
        }
    }

    pub fn concept_discovery(
        &self,
        request: ConceptDiscoveryRequest,
    ) -> CapabilityResponse<ConceptDiscoveryResult> {
        let query = request.query.trim();
        let limit = usize_limit(request.limit);
        if query.is_empty() || limit == 0 {
            return CapabilityResponse::Success {
                result: ConceptDiscoveryResult {
                    matches: Vec::new(),
                },
            };
        }

        let normalized_query = normalize(query);
        let tokens = query_tokens(query);
        let mut matches = self
            .entities_by_id
            .values()
            .copied()
            .filter_map(|entity| {
                self.rank_concept_match(entity, query, &normalized_query, &tokens)
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            right
                .score
                .exact_name
                .cmp(&left.score.exact_name)
                .then_with(|| {
                    right
                        .score
                        .exact_qualified_name
                        .cmp(&left.score.exact_qualified_name)
                })
                .then_with(|| right.score.name_prefix.cmp(&left.score.name_prefix))
                .then_with(|| {
                    right.score.name_contains.cmp(&left.score.name_contains)
                })
                .then_with(|| {
                    right
                        .score
                        .qualified_name_contains
                        .cmp(&left.score.qualified_name_contains)
                })
                .then_with(|| {
                    right.score.path_contains.cmp(&left.score.path_contains)
                })
                .then_with(|| {
                    right.score.token_matches.cmp(&left.score.token_matches)
                })
                .then_with(|| left.result.entity.name.cmp(&right.result.entity.name))
                .then_with(|| {
                    left.result
                        .entity
                        .qualified_name
                        .cmp(&right.result.entity.qualified_name)
                })
                .then_with(|| left.result.entity.id.cmp(&right.result.entity.id))
        });
        matches.truncate(limit);

        CapabilityResponse::Success {
            result: ConceptDiscoveryResult {
                matches: matches.into_iter().map(|entry| entry.result).collect(),
            },
        }
    }

    pub fn entity_briefing(
        &self,
        request: EntityBriefingRequest,
    ) -> CapabilityResponse<EntityBriefingResult> {
        let Some(entity) = self.resolve_entity(&request.entity) else {
            return CapabilityResponse::Error {
                error: invalid_request_error(&request.entity),
            };
        };

        let relationship_limit = usize_limit(request.relationship_limit);
        let inbound_count = self
            .inbound_by_target
            .get(entity.id.0.as_str())
            .map_or(0, Vec::len);
        let outbound_count = self
            .outbound_by_source
            .get(entity.id.0.as_str())
            .map_or(0, Vec::len);

        CapabilityResponse::Success {
            result: EntityBriefingResult {
                entity: self.agent_entity(entity),
                summary: format!(
                    "{} {} with {} inbound and {} outbound relationships.",
                    entity_kind_label(entity.kind),
                    entity.name,
                    inbound_count,
                    outbound_count
                ),
                related_entities: self
                    .related_entities_for(entity, relationship_limit),
            },
        }
    }

    pub fn blast_radius_estimation(
        &self,
        request: BlastRadiusRequest,
    ) -> CapabilityResponse<BlastRadiusResult> {
        let Some(entity) = self.resolve_entity(&request.entity) else {
            return CapabilityResponse::Error {
                error: invalid_request_error(&request.entity),
            };
        };

        let max_depth = usize_limit(request.max_depth);
        if max_depth == 0 {
            return CapabilityResponse::Success {
                result: BlastRadiusResult {
                    direct_impacts: Vec::new(),
                    indirect_impacts: Vec::new(),
                    unresolved_references: Vec::new(),
                },
            };
        }

        let mut direct_impacts = Vec::new();
        let mut indirect_impacts = Vec::new();
        let mut unresolved_references = Vec::new();
        let mut queue = VecDeque::from([(entity, 0usize)]);
        let mut visited_depths = BTreeMap::from([(entity.id.0.as_str(), 0usize)]);

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            let Some(relationships) =
                self.inbound_by_target.get(current.id.0.as_str())
            else {
                continue;
            };

            for relationship in relationships {
                let source_id = relationship.source.0.as_str();
                let next_depth = depth + 1;
                let Some(impacted_entity) =
                    self.entities_by_id.get(source_id).copied()
                else {
                    unresolved_references
                        .push(self.unresolved_reference(current, relationship));
                    continue;
                };

                if visited_depths.contains_key(source_id) {
                    continue;
                }
                visited_depths.insert(source_id, next_depth);

                let impact = self.impact_target(
                    current,
                    impacted_entity,
                    relationship,
                    next_depth,
                );
                if next_depth == 1 {
                    direct_impacts.push(impact);
                } else {
                    indirect_impacts.push(impact);
                }

                if next_depth < max_depth {
                    queue.push_back((impacted_entity, next_depth));
                }
            }
        }

        unresolved_references.sort_by(|left, right| {
            left.description
                .cmp(&right.description)
                .then_with(|| {
                    left.certainty.confidence.cmp(&right.certainty.confidence)
                })
                .then_with(|| left.provenance.detail.cmp(&right.provenance.detail))
        });

        CapabilityResponse::Success {
            result: BlastRadiusResult {
                direct_impacts,
                indirect_impacts,
                unresolved_references,
            },
        }
    }

    pub fn delta_impact_scan(
        &self,
        workspace_root: &Path,
        request: DeltaImpactScanRequest,
    ) -> CapabilityResponse<DeltaImpactScanResult> {
        let diffs = match collect_working_tree_diffs(workspace_root, request.scope) {
            Ok(diffs) => diffs,
            Err(error) => {
                return CapabilityResponse::Error {
                    error: CapabilityError {
                        code: CapabilityErrorCode::Internal,
                        message: format!(
                            "failed to inspect working-tree changes: {error}"
                        ),
                        retryable: true,
                    },
                };
            }
        };

        let mut completeness = AnalysisCompleteness::Complete;
        let file_impacts = diffs
            .iter()
            .map(|diff| {
                let path = diff_display_path(workspace_root, diff);
                let impacted_entities =
                    self.impacted_entities_for_diff(workspace_root, diff);
                if impacted_entities.is_empty() {
                    completeness = AnalysisCompleteness::Partial;
                }
                FileImpact {
                    path,
                    impacted_entities,
                }
            })
            .collect();

        CapabilityResponse::Success {
            result: DeltaImpactScanResult {
                completeness,
                file_impacts,
            },
        }
    }

    pub fn rename_planning(
        &self,
        workspace_root: &Path,
        request: RenamePlanningRequest,
    ) -> CapabilityResponse<RenamePlanningResult> {
        let Some(entity) = self.resolve_entity(&request.entity) else {
            return CapabilityResponse::Error {
                error: invalid_request_error(&request.entity),
            };
        };

        if request.new_name.trim().is_empty() {
            return CapabilityResponse::Error {
                error: CapabilityError {
                    code: CapabilityErrorCode::InvalidRequest,
                    message: "rename target must not be empty".to_string(),
                    retryable: false,
                },
            };
        }

        let mut high_confidence_edits = Vec::new();
        let mut low_confidence_edits = Vec::new();
        let mut conflicts = Vec::new();

        match entity.location.as_ref() {
            Some(location) => high_confidence_edits.push(RenameEdit {
                location: map_location(location),
                replacement: request.new_name.clone(),
                reason: Some(DEFINITION_EDIT_REASON.to_string()),
                certainty: phidi_rpc::agent::Certainty::observed(),
                provenance: Provenance {
                    source: ProvenanceSource::SyntaxTree,
                    detail: Some("target entity declaration".to_string()),
                },
            }),
            None => conflicts.push(RenameConflict {
                location: None,
                message: format!(
                    "unable to preview the target definition for `{}` because the snapshot has no source location",
                    entity.name
                ),
            }),
        }

        let Some(callable_kind) = rename_callable_kind(entity.kind) else {
            sort_rename_edits(&mut high_confidence_edits);
            sort_rename_edits(&mut low_confidence_edits);
            sort_rename_conflicts(&mut conflicts);
            return CapabilityResponse::Success {
                result: RenamePlanningResult {
                    high_confidence_edits,
                    low_confidence_edits,
                    conflicts,
                },
            };
        };

        let candidates = rename_candidates(
            self.entities_by_id.values().copied(),
            &entity.name,
            callable_kind,
        );
        if !candidates
            .iter()
            .any(|candidate| candidate.entity.id == entity.id)
        {
            conflicts.push(RenameConflict {
                location: entity.location.as_ref().map(map_location),
                message: format!(
                    "unable to rank rename candidates for `{}` in the current snapshot",
                    entity.name
                ),
            });
        }

        for file_path in rename_source_files(self.entities_by_path.keys().copied()) {
            let absolute_path = workspace_root.join(file_path.as_str());
            let source = match fs::read_to_string(&absolute_path) {
                Ok(source) => source,
                Err(error) => {
                    conflicts.push(RenameConflict {
                        location: Some(EntityLocation {
                            path: file_path.clone(),
                            span: None,
                        }),
                        message: format!(
                            "failed to read rename preview candidates from {}: {}",
                            file_path, error
                        ),
                    });
                    continue;
                }
            };
            let syntax = match syn::parse_file(&source) {
                Ok(syntax) => syntax,
                Err(error) => {
                    conflicts.push(RenameConflict {
                        location: Some(EntityLocation {
                            path: file_path.clone(),
                            span: None,
                        }),
                        message: format!(
                            "failed to parse rename preview candidates from {}: {}",
                            file_path, error
                        ),
                    });
                    continue;
                }
            };

            for site in collect_rename_sites(file_path.as_str(), &syntax) {
                if site.target_name != entity.name
                    || site.callable_kind != callable_kind
                {
                    continue;
                }

                match classify_rename_site(&site, entity.id.0.as_str(), &candidates)
                {
                    RenameSiteDecision::Skip => {}
                    RenameSiteDecision::HighConfidence { reason } => {
                        high_confidence_edits.push(RenameEdit {
                            location: site.location.clone(),
                            replacement: request.new_name.clone(),
                            reason: Some(reason.clone()),
                            certainty: phidi_rpc::agent::Certainty::observed(),
                            provenance: Provenance {
                                source: ProvenanceSource::SyntaxTree,
                                detail: Some(reason),
                            },
                        });
                    }
                    RenameSiteDecision::LowConfidence { reason } => {
                        let confidence = match site.callable_kind {
                            RenameCallableKind::Function => {
                                HEURISTIC_FUNCTION_CONFIDENCE
                            }
                            RenameCallableKind::Method => {
                                HEURISTIC_METHOD_CONFIDENCE
                            }
                        };
                        low_confidence_edits.push(RenameEdit {
                            location: site.location.clone(),
                            replacement: request.new_name.clone(),
                            reason: Some(reason.clone()),
                            certainty: phidi_rpc::agent::Certainty::inferred(
                                phidi_rpc::agent::ConfidenceScore::new(confidence)
                                    .expect(
                                        "rename preview confidence should be valid",
                                    ),
                            ),
                            provenance: Provenance {
                                source: ProvenanceSource::Heuristic,
                                detail: Some(reason),
                            },
                        });
                    }
                    RenameSiteDecision::Conflict { message } => {
                        conflicts.push(RenameConflict {
                            location: Some(site.location.clone()),
                            message,
                        });
                    }
                }
            }
        }

        sort_rename_edits(&mut high_confidence_edits);
        sort_rename_edits(&mut low_confidence_edits);
        sort_rename_conflicts(&mut conflicts);

        CapabilityResponse::Success {
            result: RenamePlanningResult {
                high_confidence_edits,
                low_confidence_edits,
                conflicts,
            },
        }
    }

    pub(crate) fn resolve_entity(
        &self,
        selector: &EntitySelector,
    ) -> Option<&'a SemanticEntity> {
        match selector {
            EntitySelector::Id { id } => {
                self.entities_by_id.get(id.as_str()).copied()
            }
            EntitySelector::QualifiedName { qualified_name } => {
                self.entities_by_id.values().copied().find(|entity| {
                    entity.qualified_name.as_deref() == Some(qualified_name.as_str())
                })
            }
        }
    }

    pub(crate) fn related_entities_for(
        &self,
        entity: &SemanticEntity,
        relationship_limit: usize,
    ) -> Vec<RelatedEntity> {
        let mut related_entities =
            Vec::with_capacity(relationship_limit.saturating_mul(2));

        if let Some(inbound) = self.inbound_by_target.get(entity.id.0.as_str()) {
            related_entities.extend(
                inbound
                    .iter()
                    .take(relationship_limit)
                    .filter_map(|relationship| {
                        self.related_entity(
                            entity,
                            relationship,
                            RelationshipDirection::Inbound,
                        )
                    }),
            );
        }

        if let Some(outbound) = self.outbound_by_source.get(entity.id.0.as_str()) {
            related_entities.extend(
                outbound.iter().take(relationship_limit).filter_map(
                    |relationship| {
                        self.related_entity(
                            entity,
                            relationship,
                            RelationshipDirection::Outbound,
                        )
                    },
                ),
            );
        }

        related_entities
    }

    pub(crate) fn agent_entity(&self, entity: &SemanticEntity) -> AgentEntity {
        AgentEntity {
            id: entity.id.0.clone(),
            kind: map_entity_kind(entity.kind),
            name: entity.name.clone(),
            qualified_name: entity.qualified_name.clone(),
            location: entity.location.as_ref().map(map_location),
        }
    }

    fn rank_concept_match(
        &self,
        entity: &SemanticEntity,
        query: &str,
        normalized_query: &str,
        tokens: &[String],
    ) -> Option<RankedConceptMatch> {
        let normalized_name = normalize(&entity.name);
        let normalized_qualified_name =
            entity.qualified_name.as_deref().map(normalize);
        let normalized_path = entity
            .location
            .as_ref()
            .map(|location| normalize(&location.path));
        let combined = [
            normalized_name.as_str(),
            normalized_qualified_name.as_deref().unwrap_or(""),
            normalized_path.as_deref().unwrap_or(""),
            entity_kind_label(entity.kind),
        ]
        .join(" ");

        let token_matches = tokens
            .iter()
            .filter(|token| combined.contains(token.as_str()))
            .count();
        let exact_name = normalized_name == normalized_query;
        let exact_qualified_name =
            normalized_qualified_name.as_deref() == Some(normalized_query);
        let name_prefix = normalized_name.starts_with(normalized_query);
        let name_contains = normalized_name.contains(normalized_query);
        let qualified_name_contains = normalized_qualified_name
            .as_deref()
            .is_some_and(|value| value.contains(normalized_query));
        let path_contains = normalized_path
            .as_deref()
            .is_some_and(|value| value.contains(normalized_query));

        let matches_query = name_contains
            || qualified_name_contains
            || path_contains
            || (!tokens.is_empty() && token_matches == tokens.len());
        if !matches_query {
            return None;
        }

        let mut matched_fields = Vec::new();
        if name_contains {
            matched_fields.push("name");
        }
        if qualified_name_contains {
            matched_fields.push("qualified name");
        }
        if path_contains {
            matched_fields.push("path");
        }
        if matched_fields.is_empty() && token_matches > 0 {
            matched_fields.push("entity metadata");
        }

        Some(RankedConceptMatch {
            score: MatchScore {
                exact_name,
                exact_qualified_name,
                name_prefix,
                name_contains,
                qualified_name_contains,
                path_contains,
                token_matches,
            },
            result: ConceptMatch {
                entity: self.agent_entity(entity),
                summary: format!(
                    "Matched \"{}\" in {}.",
                    query,
                    matched_fields.join(", ")
                ),
                related_entities: self.related_entities_for(
                    entity,
                    CONCEPT_PREVIEW_RELATIONSHIP_LIMIT,
                ),
            },
        })
    }

    fn related_entity(
        &self,
        focal_entity: &SemanticEntity,
        relationship: &SemanticRelationship,
        direction: RelationshipDirection,
    ) -> Option<RelatedEntity> {
        let related_entity_id = match direction {
            RelationshipDirection::Inbound => relationship.source.0.as_str(),
            RelationshipDirection::Outbound => relationship.target.0.as_str(),
        };
        let related_entity = self.entities_by_id.get(related_entity_id).copied()?;

        Some(RelatedEntity {
            direction,
            relationship_kind: map_relationship_kind(relationship.kind),
            entity: self.agent_entity(related_entity),
            summary: Some(relationship_summary(
                direction,
                relationship.kind,
                &focal_entity.name,
                &related_entity.name,
            )),
            certainty: map_certainty(relationship.certainty),
            provenance: map_provenance(&relationship.provenance),
        })
    }

    fn impact_target(
        &self,
        focal_entity: &SemanticEntity,
        impacted_entity: &SemanticEntity,
        relationship: &SemanticRelationship,
        depth: usize,
    ) -> ImpactTarget {
        ImpactTarget {
            entity: self.agent_entity(impacted_entity),
            depth: depth_to_u32(depth),
            reason: Some(relationship_summary(
                RelationshipDirection::Inbound,
                relationship.kind,
                &focal_entity.name,
                &impacted_entity.name,
            )),
            certainty: map_certainty(relationship.certainty),
            provenance: map_provenance(&relationship.provenance),
        }
    }

    fn unresolved_reference(
        &self,
        focal_entity: &SemanticEntity,
        relationship: &SemanticRelationship,
    ) -> UnresolvedReference {
        UnresolvedReference {
            description: format!(
                "Unresolved {} relationship from `{}` to `{}`.",
                relationship_verb(relationship.kind),
                relationship.source.0,
                focal_entity.name
            ),
            certainty: map_certainty(relationship.certainty),
            provenance: map_provenance(&relationship.provenance),
        }
    }

    fn impacted_entities_for_diff(
        &self,
        workspace_root: &Path,
        diff: &FileDiff,
    ) -> Vec<ImpactTarget> {
        let mut entities = BTreeMap::new();
        for path in diff_lookup_paths(workspace_root, diff) {
            if let Some(candidates) = self.entities_by_path.get(path.as_str()) {
                for entity in candidates {
                    entities.insert(entity.id.0.as_str(), *entity);
                }
            }
        }

        entities
            .into_values()
            .map(|entity| ImpactTarget {
                entity: self.agent_entity(entity),
                depth: 1,
                reason: Some(format!(
                    "{} is located in changed file {}.",
                    entity.name,
                    entity
                        .location
                        .as_ref()
                        .map(|location| location.path.as_str())
                        .unwrap_or_default()
                )),
                certainty: phidi_rpc::agent::Certainty::observed(),
                provenance: Provenance {
                    source: ProvenanceSource::WorkingTree,
                    detail: Some(diff_reason(diff)),
                },
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
struct MatchScore {
    exact_name: bool,
    exact_qualified_name: bool,
    name_prefix: bool,
    name_contains: bool,
    qualified_name_contains: bool,
    path_contains: bool,
    token_matches: usize,
}

struct RankedConceptMatch {
    score: MatchScore,
    result: ConceptMatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenameCallableKind {
    Function,
    Method,
}

#[derive(Clone, Debug)]
struct RenameCandidate<'a> {
    entity: &'a SemanticEntity,
    qualified_name: Option<&'a str>,
    file_path: &'a str,
    module_path: Vec<String>,
    callable_kind: RenameCallableKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RenameSite {
    location: EntityLocation,
    file_path: String,
    target_name: String,
    target_path: Option<String>,
    callable_kind: RenameCallableKind,
    caller_module_path: Vec<String>,
}

enum RenameSiteDecision {
    Skip,
    HighConfidence { reason: String },
    LowConfidence { reason: String },
    Conflict { message: String },
}

#[derive(Default)]
struct RenameSiteCollector {
    file_path: String,
    caller_module_path: Vec<String>,
    sites: Vec<RenameSite>,
}

impl<'ast> Visit<'ast> for RenameSiteCollector {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(path_expr) = node.func.as_ref() {
            if let Some(segment) = path_expr.path.segments.last() {
                self.sites.push(RenameSite {
                    location: span_location(&self.file_path, segment.ident.span()),
                    file_path: self.file_path.clone(),
                    target_name: segment.ident.to_string(),
                    target_path: (path_expr.path.segments.len() > 1)
                        .then(|| syn_path_label(&path_expr.path)),
                    callable_kind: RenameCallableKind::Function,
                    caller_module_path: self.caller_module_path.clone(),
                });
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        self.sites.push(RenameSite {
            location: span_location(&self.file_path, node.method.span()),
            file_path: self.file_path.clone(),
            target_name: node.method.to_string(),
            target_path: None,
            callable_kind: RenameCallableKind::Method,
            caller_module_path: self.caller_module_path.clone(),
        });
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn rename_callable_kind(kind: EntityKind) -> Option<RenameCallableKind> {
    match kind {
        EntityKind::Function | EntityKind::Test => {
            Some(RenameCallableKind::Function)
        }
        EntityKind::Method => Some(RenameCallableKind::Method),
        _ => None,
    }
}

fn rename_candidates<'a>(
    entities: impl Iterator<Item = &'a SemanticEntity>,
    target_name: &str,
    callable_kind: RenameCallableKind,
) -> Vec<RenameCandidate<'a>> {
    entities
        .filter_map(|entity| {
            if entity.name != target_name {
                return None;
            }
            let entity_callable_kind = rename_callable_kind(entity.kind)?;
            if entity_callable_kind != callable_kind {
                return None;
            }
            let file_path = entity
                .location
                .as_ref()
                .map(|location| location.path.as_str())
                .unwrap_or_default();
            let module_path = entity
                .qualified_name
                .as_deref()
                .map(qualified_module_path)
                .unwrap_or_else(|| module_path_for_file(file_path));
            Some(RenameCandidate {
                entity,
                qualified_name: entity.qualified_name.as_deref(),
                file_path,
                module_path,
                callable_kind: entity_callable_kind,
            })
        })
        .collect()
}

fn rename_source_files<'a>(paths: impl Iterator<Item = &'a str>) -> Vec<String> {
    paths
        .filter(|path| path.ends_with(".rs"))
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_rename_sites(file_path: &str, syntax: &syn::File) -> Vec<RenameSite> {
    let mut collector = RenameSiteCollector {
        file_path: file_path.to_string(),
        caller_module_path: module_path_for_file(file_path),
        sites: Vec::new(),
    };
    collector.visit_file(syntax);
    collector.sites
}

fn classify_rename_site(
    site: &RenameSite,
    target_entity_id: &str,
    candidates: &[RenameCandidate<'_>],
) -> RenameSiteDecision {
    let matching_candidates = candidates
        .iter()
        .filter(|candidate| candidate.callable_kind == site.callable_kind)
        .collect::<Vec<_>>();
    if matching_candidates.is_empty() {
        return RenameSiteDecision::Skip;
    }

    match site.callable_kind {
        RenameCallableKind::Function => {
            if let Some(target_path) = site.target_path.as_deref() {
                let exact_matches = matching_candidates
                    .iter()
                    .copied()
                    .filter(|candidate| {
                        candidate.qualified_name.is_some_and(|qualified_name| {
                            qualified_name == target_path
                                || qualified_name
                                    .ends_with(&format!("::{target_path}"))
                                || qualified_name.ends_with(target_path)
                        })
                    })
                    .collect::<Vec<_>>();
                return classify_exact_path_match(
                    target_entity_id,
                    &exact_matches,
                    HIGH_CONFIDENCE_REASON,
                    FUNCTION_CONFLICT_REASON,
                );
            }

            classify_heuristic_function_site(
                site,
                target_entity_id,
                &matching_candidates,
            )
        }
        RenameCallableKind::Method => {
            if matching_candidates.len() == 1
                && matching_candidates[0].entity.id.0 == target_entity_id
            {
                return RenameSiteDecision::LowConfidence {
                    reason: LOW_CONFIDENCE_REASON.to_string(),
                };
            }
            if matching_candidates
                .iter()
                .any(|candidate| candidate.entity.id.0 == target_entity_id)
            {
                return RenameSiteDecision::Conflict {
                    message: METHOD_CONFLICT_REASON.to_string(),
                };
            }
            RenameSiteDecision::Skip
        }
    }
}

fn classify_exact_path_match(
    target_entity_id: &str,
    exact_matches: &[&RenameCandidate<'_>],
    high_confidence_reason: &str,
    conflict_reason: &str,
) -> RenameSiteDecision {
    if exact_matches.is_empty() {
        return RenameSiteDecision::Skip;
    }

    if exact_matches.len() == 1 {
        return if exact_matches[0].entity.id.0 == target_entity_id {
            RenameSiteDecision::HighConfidence {
                reason: high_confidence_reason.to_string(),
            }
        } else {
            RenameSiteDecision::Skip
        };
    }

    if exact_matches
        .iter()
        .any(|candidate| candidate.entity.id.0 == target_entity_id)
    {
        RenameSiteDecision::Conflict {
            message: conflict_reason.to_string(),
        }
    } else {
        RenameSiteDecision::Skip
    }
}

fn classify_heuristic_function_site(
    site: &RenameSite,
    target_entity_id: &str,
    candidates: &[&RenameCandidate<'_>],
) -> RenameSiteDecision {
    let mut ranked_candidates = candidates
        .iter()
        .copied()
        .map(|candidate| (rename_candidate_rank(site, candidate), candidate))
        .collect::<Vec<_>>();
    ranked_candidates.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.file_path.cmp(right.1.file_path))
            .then_with(|| left.1.qualified_name.cmp(&right.1.qualified_name))
            .then_with(|| left.1.entity.id.0.cmp(&right.1.entity.id.0))
    });

    let Some((best_rank, _)) = ranked_candidates.first() else {
        return RenameSiteDecision::Skip;
    };
    let top_candidates = ranked_candidates
        .iter()
        .filter(|(rank, _)| rank == best_rank)
        .map(|(_, candidate)| *candidate)
        .collect::<Vec<_>>();

    if !top_candidates
        .iter()
        .any(|candidate| candidate.entity.id.0 == target_entity_id)
    {
        return RenameSiteDecision::Skip;
    }

    if top_candidates.len() == 1 {
        RenameSiteDecision::LowConfidence {
            reason: LOW_CONFIDENCE_REASON.to_string(),
        }
    } else {
        RenameSiteDecision::Conflict {
            message: FUNCTION_CONFLICT_REASON.to_string(),
        }
    }
}

fn rename_candidate_rank(
    site: &RenameSite,
    candidate: &RenameCandidate<'_>,
) -> (bool, usize) {
    (
        candidate.file_path == site.file_path,
        shared_prefix_depth(&site.caller_module_path, &candidate.module_path),
    )
}

fn sort_rename_edits(edits: &mut Vec<RenameEdit>) {
    edits.sort_by(|left, right| {
        compare_locations(&left.location, &right.location)
            .then_with(|| left.replacement.cmp(&right.replacement))
            .then_with(|| left.reason.cmp(&right.reason))
            .then_with(|| left.provenance.detail.cmp(&right.provenance.detail))
    });
}

fn sort_rename_conflicts(conflicts: &mut Vec<RenameConflict>) {
    conflicts.sort_by(|left, right| {
        compare_optional_locations(left.location.as_ref(), right.location.as_ref())
            .then_with(|| left.message.cmp(&right.message))
    });
}

fn compare_locations(
    left: &EntityLocation,
    right: &EntityLocation,
) -> std::cmp::Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| left.span.cmp(&right.span))
}

fn compare_optional_locations(
    left: Option<&EntityLocation>,
    right: Option<&EntityLocation>,
) -> std::cmp::Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_locations(left, right),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn module_path_for_file(relative_path: &str) -> Vec<String> {
    let path = Path::new(relative_path);
    let mut segments: Vec<String> = path
        .iter()
        .filter_map(|segment| segment.to_str().map(str::to_string))
        .collect();

    if matches!(
        segments.first().map(String::as_str),
        Some("src" | "tests" | "examples")
    ) {
        segments.remove(0);
    }

    let Some(last_segment) = segments.pop() else {
        return Vec::new();
    };
    let stem = Path::new(&last_segment)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();
    if !matches!(stem.as_str(), "lib" | "main" | "mod") {
        segments.push(stem);
    }

    segments
}

fn qualified_module_path(qualified_name: &str) -> Vec<String> {
    let mut segments = qualified_name
        .split("::")
        .filter(|segment| !segment.is_empty())
        .filter(|segment| !matches!(*segment, "crate" | "self" | "super"))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !segments.is_empty() {
        segments.pop();
    }
    segments
}

fn shared_prefix_depth(left: &[String], right: &[String]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(left, right)| left == right)
        .count()
}

fn syn_path_label(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn span_location(path: &str, span: proc_macro2::Span) -> EntityLocation {
    let start = span.start();
    let end = span.end();
    EntityLocation {
        path: path.to_string(),
        span: Some(TextSpan {
            start: TextPoint {
                line: start.line.saturating_sub(1) as u32,
                column: start.column as u32,
            },
            end: TextPoint {
                line: end.line.saturating_sub(1) as u32,
                column: end.column as u32,
            },
        }),
    }
}

fn compare_entities(
    left: &SemanticEntity,
    right: &SemanticEntity,
) -> std::cmp::Ordering {
    entity_kind_rank(left.kind)
        .cmp(&entity_kind_rank(right.kind))
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.qualified_name.cmp(&right.qualified_name))
        .then_with(|| left.id.0.cmp(&right.id.0))
}

fn invalid_request_error(selector: &EntitySelector) -> CapabilityError {
    CapabilityError {
        code: CapabilityErrorCode::InvalidRequest,
        message: match selector {
            EntitySelector::Id { id } => {
                format!("entity `{id}` was not found in the current snapshot")
            }
            EntitySelector::QualifiedName { qualified_name } => format!(
                "entity `{qualified_name}` was not found in the current snapshot"
            ),
        },
        retryable: false,
    }
}

fn map_entity_kind(kind: EntityKind) -> AgentEntityKind {
    match kind {
        EntityKind::Enum => AgentEntityKind::Enum,
        EntityKind::File => AgentEntityKind::File,
        EntityKind::Function => AgentEntityKind::Function,
        EntityKind::ImplBlock => AgentEntityKind::ImplBlock,
        EntityKind::Import => AgentEntityKind::Import,
        EntityKind::Macro => AgentEntityKind::Macro,
        EntityKind::Method => AgentEntityKind::Method,
        EntityKind::Module => AgentEntityKind::Module,
        EntityKind::Package => AgentEntityKind::Package,
        EntityKind::Struct => AgentEntityKind::Struct,
        EntityKind::Test => AgentEntityKind::Test,
        EntityKind::Trait => AgentEntityKind::Trait,
        EntityKind::Workspace => AgentEntityKind::Workspace,
    }
}

fn entity_kind_rank(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Workspace => 0,
        EntityKind::Package => 1,
        EntityKind::Module => 2,
        EntityKind::File => 3,
        EntityKind::Import => 4,
        EntityKind::Trait => 5,
        EntityKind::Struct => 6,
        EntityKind::Enum => 7,
        EntityKind::ImplBlock => 8,
        EntityKind::Function => 9,
        EntityKind::Method => 10,
        EntityKind::Macro => 11,
        EntityKind::Test => 12,
    }
}

fn map_relationship_kind(kind: RelationshipKind) -> AgentRelationshipKind {
    match kind {
        RelationshipKind::Calls => AgentRelationshipKind::Calls,
        RelationshipKind::Contains => AgentRelationshipKind::Contains,
        RelationshipKind::Defines => AgentRelationshipKind::Defines,
        RelationshipKind::Implements => AgentRelationshipKind::Implements,
        RelationshipKind::Imports => AgentRelationshipKind::Imports,
        RelationshipKind::References => AgentRelationshipKind::References,
    }
}

fn map_certainty(certainty: SemanticCertainty) -> phidi_rpc::agent::Certainty {
    phidi_rpc::agent::Certainty {
        kind: match certainty.kind {
            SemanticCertaintyKind::Observed => {
                phidi_rpc::agent::CertaintyKind::Observed
            }
            SemanticCertaintyKind::Inferred => {
                phidi_rpc::agent::CertaintyKind::Inferred
            }
        },
        confidence: phidi_rpc::agent::ConfidenceScore::new(
            certainty.confidence.get(),
        )
        .expect("semantic-map confidence should already be validated"),
    }
}

fn map_provenance(
    provenance: &phidi_core::semantic_map::RelationshipProvenance,
) -> Provenance {
    Provenance {
        source: match provenance.source {
            SemanticProvenanceSource::Heuristic => ProvenanceSource::Heuristic,
            SemanticProvenanceSource::SymbolResolution => {
                ProvenanceSource::SymbolResolution
            }
            SemanticProvenanceSource::SyntaxTree => ProvenanceSource::SyntaxTree,
        },
        detail: provenance.detail.clone(),
    }
}

fn map_location(
    location: &phidi_core::semantic_map::EntityLocation,
) -> EntityLocation {
    EntityLocation {
        path: location.path.clone(),
        span: location.span.map(|span| TextSpan {
            start: TextPoint {
                line: span.start.line,
                column: span.start.column,
            },
            end: TextPoint {
                line: span.end.line,
                column: span.end.column,
            },
        }),
    }
}

fn relationship_summary(
    direction: RelationshipDirection,
    kind: RelationshipKind,
    focal_name: &str,
    related_name: &str,
) -> String {
    let verb = relationship_verb(kind);
    match direction {
        RelationshipDirection::Inbound => {
            format!("{related_name} {verb} {focal_name}.")
        }
        RelationshipDirection::Outbound => {
            format!("{focal_name} {verb} {related_name}.")
        }
    }
}

fn relationship_verb(kind: RelationshipKind) -> &'static str {
    match kind {
        RelationshipKind::Calls => "calls",
        RelationshipKind::Contains => "contains",
        RelationshipKind::Defines => "defines",
        RelationshipKind::Implements => "implements",
        RelationshipKind::Imports => "imports",
        RelationshipKind::References => "references",
    }
}

fn relationship_kind_rank(kind: RelationshipKind) -> u8 {
    match kind {
        RelationshipKind::Calls => 0,
        RelationshipKind::Contains => 1,
        RelationshipKind::Defines => 2,
        RelationshipKind::Implements => 3,
        RelationshipKind::Imports => 4,
        RelationshipKind::References => 5,
    }
}

fn certainty_rank(certainty: SemanticCertainty) -> (u8, u8) {
    let kind_rank = match certainty.kind {
        SemanticCertaintyKind::Observed => 0,
        SemanticCertaintyKind::Inferred => 1,
    };
    (kind_rank, u8::MAX - certainty.confidence.get())
}

fn provenance_rank(source: SemanticProvenanceSource) -> u8 {
    match source {
        SemanticProvenanceSource::SyntaxTree => 0,
        SemanticProvenanceSource::SymbolResolution => 1,
        SemanticProvenanceSource::Heuristic => 2,
    }
}

fn entity_kind_label(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Enum => "enum",
        EntityKind::File => "file",
        EntityKind::Function => "function",
        EntityKind::ImplBlock => "impl block",
        EntityKind::Import => "import",
        EntityKind::Macro => "macro",
        EntityKind::Method => "method",
        EntityKind::Module => "module",
        EntityKind::Package => "package",
        EntityKind::Struct => "struct",
        EntityKind::Test => "test",
        EntityKind::Trait => "trait",
        EntityKind::Workspace => "workspace",
    }
}

fn normalize(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn query_tokens(query: &str) -> Vec<String> {
    query
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(normalize)
        .collect()
}

fn usize_limit(limit: u32) -> usize {
    match usize::try_from(limit) {
        Ok(limit) => limit,
        Err(_) => usize::MAX,
    }
}

fn depth_to_u32(depth: usize) -> u32 {
    match u32::try_from(depth) {
        Ok(depth) => depth,
        Err(_) => u32::MAX,
    }
}

fn diff_lookup_paths(workspace_root: &Path, diff: &FileDiff) -> Vec<String> {
    match diff {
        FileDiff::Modified(path)
        | FileDiff::Added(path)
        | FileDiff::Deleted(path) => {
            vec![relative_path_string(workspace_root, path)]
        }
        FileDiff::Renamed(new_path, old_path) => vec![
            relative_path_string(workspace_root, new_path),
            relative_path_string(workspace_root, old_path),
        ],
    }
}

fn diff_display_path(workspace_root: &Path, diff: &FileDiff) -> String {
    let path = match diff {
        FileDiff::Modified(path)
        | FileDiff::Added(path)
        | FileDiff::Deleted(path)
        | FileDiff::Renamed(path, _) => path,
    };
    relative_path_string(workspace_root, path)
}

fn relative_path_string(workspace_root: &Path, path: &Path) -> String {
    path.strip_prefix(workspace_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn diff_reason(diff: &FileDiff) -> String {
    match diff {
        FileDiff::Modified(_) => "Observed modified working-tree file.".to_string(),
        FileDiff::Added(_) => "Observed added working-tree file.".to_string(),
        FileDiff::Deleted(_) => "Observed deleted working-tree file.".to_string(),
        FileDiff::Renamed(new_path, old_path) => format!(
            "Observed renamed working-tree file from {} to {}.",
            old_path.to_string_lossy(),
            new_path.to_string_lossy()
        ),
    }
}
