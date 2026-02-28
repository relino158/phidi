use std::collections::{BTreeMap, VecDeque};

use phidi_core::semantic_map::{
    Certainty as SemanticCertainty, CertaintyKind as SemanticCertaintyKind,
    EntityKind, ProvenanceSource as SemanticProvenanceSource, RelationshipKind,
    SemanticEntity, SemanticRelationship, WorkspaceSnapshot,
};
use phidi_rpc::agent::{
    AgentEntity, AgentEntityKind, AgentRelationshipKind, BlastRadiusRequest,
    BlastRadiusResult, CapabilityError, CapabilityErrorCode, CapabilityResponse,
    ConceptDiscoveryRequest, ConceptDiscoveryResult, ConceptMatch,
    EntityBriefingRequest, EntityBriefingResult, EntityLocation, EntitySelector,
    ImpactTarget, Provenance, ProvenanceSource, RelatedEntity,
    RelationshipDirection, TextPoint, TextSpan, UnresolvedReference,
};

const CONCEPT_PREVIEW_RELATIONSHIP_LIMIT: usize = 2;

pub struct SnapshotQueryService<'a> {
    entities_by_id: BTreeMap<&'a str, &'a SemanticEntity>,
    inbound_by_target: BTreeMap<&'a str, Vec<&'a SemanticRelationship>>,
    outbound_by_source: BTreeMap<&'a str, Vec<&'a SemanticRelationship>>,
}

impl<'a> SnapshotQueryService<'a> {
    pub fn new(snapshot: &'a WorkspaceSnapshot) -> Self {
        let mut entities_by_id = BTreeMap::new();
        for entity in &snapshot.entities {
            entities_by_id.insert(entity.id.0.as_str(), entity);
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

        Self {
            entities_by_id,
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
