use phidi_core::semantic_map::{
    Certainty, ConfidenceScore, EntityId, EntityKind, EntityLocation,
    ProvenanceSource, RelationshipKind, RelationshipProvenance, SemanticEntity,
    SemanticRelationship, SnapshotKind, SnapshotProvenance, TextPoint, TextSpan,
    WorkspaceSnapshot,
};
use phidi_proxy::query::SnapshotQueryService;
use phidi_rpc::agent::{
    AgentRelationshipKind, CapabilityErrorCode, CapabilityResponse,
    ConceptDiscoveryRequest, EntityBriefingRequest, EntitySelector,
    ProvenanceSource as AgentProvenanceSource, RelationshipDirection,
};

fn snapshot_fixture() -> WorkspaceSnapshot {
    let mut snapshot = WorkspaceSnapshot::new(
        SnapshotKind::Working,
        SnapshotProvenance {
            revision: Some("abc123".to_string()),
            has_uncommitted_changes: false,
        },
    );

    snapshot.entities = vec![
        entity("workspace", EntityKind::Workspace, "workspace", None, None),
        entity(
            "function:controller_a",
            EntityKind::Function,
            "controller_a",
            Some("crate::ui::controller_a"),
            Some("src/controller.rs"),
        ),
        entity(
            "function:controller_b",
            EntityKind::Function,
            "controller_b",
            Some("crate::ui::controller_b"),
            Some("src/controller.rs"),
        ),
        entity(
            "function:draw_frame",
            EntityKind::Function,
            "draw_frame",
            Some("crate::graphics::draw_frame"),
            Some("src/render.rs"),
        ),
        entity(
            "function:render_alpha",
            EntityKind::Function,
            "render_alpha",
            Some("crate::ui::render_alpha"),
            Some("src/render.rs"),
        ),
        entity(
            "function:render_beta",
            EntityKind::Function,
            "render_beta",
            Some("crate::ui::render_beta"),
            Some("src/render.rs"),
        ),
    ];

    snapshot.relationships = vec![
        relationship(
            "function:controller_b",
            "function:render_alpha",
            RelationshipKind::References,
            Certainty::inferred(ConfidenceScore::new(61).unwrap()),
            ProvenanceSource::Heuristic,
            Some("name proximity"),
        ),
        relationship(
            "function:render_alpha",
            "function:draw_frame",
            RelationshipKind::Calls,
            Certainty::inferred(ConfidenceScore::new(78).unwrap()),
            ProvenanceSource::SymbolResolution,
            Some("resolved call target"),
        ),
        relationship(
            "function:controller_a",
            "function:render_alpha",
            RelationshipKind::Calls,
            Certainty::observed(),
            ProvenanceSource::SyntaxTree,
            None,
        ),
    ];

    snapshot
}

fn entity(
    id: &str,
    kind: EntityKind,
    name: &str,
    qualified_name: Option<&str>,
    path: Option<&str>,
) -> SemanticEntity {
    SemanticEntity {
        id: EntityId(id.to_string()),
        kind,
        name: name.to_string(),
        qualified_name: qualified_name.map(str::to_string),
        location: path.map(|path| EntityLocation {
            path: path.to_string(),
            span: Some(TextSpan {
                start: TextPoint { line: 4, column: 0 },
                end: TextPoint {
                    line: 12,
                    column: 1,
                },
            }),
        }),
    }
}

fn relationship(
    source: &str,
    target: &str,
    kind: RelationshipKind,
    certainty: Certainty,
    provenance_source: ProvenanceSource,
    detail: Option<&str>,
) -> SemanticRelationship {
    SemanticRelationship {
        source: EntityId(source.to_string()),
        target: EntityId(target.to_string()),
        kind,
        certainty,
        provenance: RelationshipProvenance {
            source: provenance_source,
            detail: detail.map(str::to_string),
        },
    }
}

#[test]
fn concept_discovery_orders_matches_deterministically_and_preserves_edge_metadata() {
    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let request = ConceptDiscoveryRequest {
        query: "render".to_string(),
        limit: 2,
    };

    let first = service.concept_discovery(request.clone());
    let second = service.concept_discovery(request);

    assert_eq!(first, second);

    let CapabilityResponse::Success { result } = first else {
        panic!("expected concept discovery success");
    };

    let names = result
        .matches
        .iter()
        .map(|entry| entry.entity.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["render_alpha", "render_beta"]);

    let first_match = &result.matches[0];
    assert!(first_match.summary.contains("render"));
    assert!(first_match.related_entities.iter().any(|related| {
        related.direction == RelationshipDirection::Inbound
            && related.relationship_kind == AgentRelationshipKind::Calls
            && related.entity.name == "controller_a"
    }));

    let outbound = first_match
        .related_entities
        .iter()
        .find(|related| {
            related.direction == RelationshipDirection::Outbound
                && related.relationship_kind == AgentRelationshipKind::Calls
                && related.entity.name == "draw_frame"
        })
        .expect("expected outbound call context");
    assert_eq!(outbound.certainty.confidence.get(), 78);
    assert_eq!(
        outbound.provenance.source,
        AgentProvenanceSource::SymbolResolution
    );
    assert_eq!(
        outbound.provenance.detail.as_deref(),
        Some("resolved call target")
    );
}

#[test]
fn entity_briefing_supports_qualified_name_lookup_and_per_direction_limits() {
    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.entity_briefing(EntityBriefingRequest {
        entity: EntitySelector::QualifiedName {
            qualified_name: "crate::ui::render_alpha".to_string(),
        },
        relationship_limit: 1,
    });

    let CapabilityResponse::Success { result } = response else {
        panic!("expected entity briefing success");
    };

    assert_eq!(result.entity.name, "render_alpha");
    assert!(result.summary.contains("render_alpha"));
    assert_eq!(result.related_entities.len(), 2);
    assert_eq!(
        result
            .related_entities
            .iter()
            .filter(|related| related.direction == RelationshipDirection::Inbound)
            .count(),
        1
    );
    assert_eq!(
        result
            .related_entities
            .iter()
            .filter(|related| related.direction == RelationshipDirection::Outbound)
            .count(),
        1
    );

    let inbound = result
        .related_entities
        .iter()
        .find(|related| related.direction == RelationshipDirection::Inbound)
        .expect("expected one inbound relationship");
    assert_eq!(inbound.entity.name, "controller_a");
    assert_eq!(inbound.provenance.source, AgentProvenanceSource::SyntaxTree);

    let outbound = result
        .related_entities
        .iter()
        .find(|related| related.direction == RelationshipDirection::Outbound)
        .expect("expected one outbound relationship");
    assert_eq!(outbound.entity.name, "draw_frame");
    assert_eq!(outbound.certainty.confidence.get(), 78);
}

#[test]
fn entity_briefing_returns_invalid_request_for_unknown_entities() {
    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.entity_briefing(EntityBriefingRequest {
        entity: EntitySelector::Id {
            id: "function:missing".to_string(),
        },
        relationship_limit: 2,
    });

    let CapabilityResponse::Error { error } = response else {
        panic!("expected invalid request error");
    };

    assert_eq!(error.code, CapabilityErrorCode::InvalidRequest);
    assert!(!error.retryable);
    assert!(error.message.contains("function:missing"));
}
