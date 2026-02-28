use phidi_core::semantic_map::{
    Certainty, ConfidenceScore, EntityId, EntityKind, EntityLocation,
    ProvenanceSource, RelationshipKind, RelationshipProvenance, SemanticEntity,
    SemanticRelationship, SnapshotKind, SnapshotProvenance, TextPoint, TextSpan,
    WorkspaceSnapshot,
};
use phidi_proxy::query::SnapshotQueryService;
use phidi_rpc::agent::{
    BlastRadiusRequest, CapabilityErrorCode, CapabilityResponse, EntitySelector,
    ProvenanceSource as AgentProvenanceSource,
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
            "function:render_alpha",
            EntityKind::Function,
            "render_alpha",
            Some("crate::ui::render_alpha"),
            Some("src/render.rs"),
        ),
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
            "function:panel_entry",
            EntityKind::Function,
            "panel_entry",
            Some("crate::panel::panel_entry"),
            Some("src/panel.rs"),
        ),
        entity(
            "test:controller_smoke",
            EntityKind::Test,
            "controller_smoke",
            Some("crate::tests::controller_smoke"),
            Some("tests/controller.rs"),
        ),
    ];

    snapshot.relationships = vec![
        relationship(
            "function:controller_a",
            "function:render_alpha",
            RelationshipKind::Calls,
            Certainty::observed(),
            ProvenanceSource::SyntaxTree,
            None,
        ),
        relationship(
            "function:controller_b",
            "function:render_alpha",
            RelationshipKind::References,
            Certainty::inferred(ConfidenceScore::new(61).unwrap()),
            ProvenanceSource::Heuristic,
            Some("name proximity"),
        ),
        relationship(
            "function:panel_entry",
            "function:controller_a",
            RelationshipKind::Imports,
            Certainty::inferred(ConfidenceScore::new(73).unwrap()),
            ProvenanceSource::SymbolResolution,
            Some("resolved import chain"),
        ),
        relationship(
            "test:controller_smoke",
            "function:controller_a",
            RelationshipKind::References,
            Certainty::inferred(ConfidenceScore::new(52).unwrap()),
            ProvenanceSource::Heuristic,
            Some("test naming heuristic"),
        ),
        relationship(
            "function:missing_runtime",
            "function:controller_a",
            RelationshipKind::Calls,
            Certainty::inferred(ConfidenceScore::new(35).unwrap()),
            ProvenanceSource::Heuristic,
            Some("dynamic dispatch"),
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
fn blast_radius_separates_direct_and_indirect_impacts_and_preserves_edge_metadata() {
    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.blast_radius_estimation(BlastRadiusRequest {
        entity: EntitySelector::QualifiedName {
            qualified_name: "crate::ui::render_alpha".to_string(),
        },
        max_depth: 3,
    });

    let CapabilityResponse::Success { result } = response else {
        panic!("expected blast radius success");
    };

    let direct_names = result
        .direct_impacts
        .iter()
        .map(|impact| impact.entity.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(direct_names, vec!["controller_a", "controller_b"]);

    let indirect_names = result
        .indirect_impacts
        .iter()
        .map(|impact| impact.entity.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(indirect_names, vec!["panel_entry", "controller_smoke"]);

    let direct_reference = &result.direct_impacts[1];
    assert_eq!(direct_reference.depth, 1);
    assert_eq!(
        direct_reference.reason.as_deref(),
        Some("controller_b references render_alpha.")
    );
    assert_eq!(direct_reference.certainty.confidence.get(), 61);
    assert_eq!(
        direct_reference.provenance.source,
        AgentProvenanceSource::Heuristic
    );
    assert_eq!(
        direct_reference.provenance.detail.as_deref(),
        Some("name proximity")
    );

    let indirect_import = &result.indirect_impacts[0];
    assert_eq!(indirect_import.depth, 2);
    assert_eq!(
        indirect_import.reason.as_deref(),
        Some("panel_entry imports controller_a.")
    );
    assert_eq!(indirect_import.certainty.confidence.get(), 73);
    assert_eq!(
        indirect_import.provenance.source,
        AgentProvenanceSource::SymbolResolution
    );
    assert_eq!(
        indirect_import.provenance.detail.as_deref(),
        Some("resolved import chain")
    );

    assert_eq!(result.unresolved_references.len(), 1);
}

#[test]
fn blast_radius_reports_unresolved_references_without_dropping_results() {
    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.blast_radius_estimation(BlastRadiusRequest {
        entity: EntitySelector::Id {
            id: "function:controller_a".to_string(),
        },
        max_depth: 2,
    });

    let CapabilityResponse::Success { result } = response else {
        panic!("expected blast radius success");
    };

    assert_eq!(
        result
            .direct_impacts
            .iter()
            .map(|impact| impact.entity.name.as_str())
            .collect::<Vec<_>>(),
        vec!["panel_entry", "controller_smoke"]
    );
    assert!(result.indirect_impacts.is_empty());
    assert_eq!(result.unresolved_references.len(), 1);

    let unresolved = &result.unresolved_references[0];
    assert!(
        unresolved.description.contains("function:missing_runtime"),
        "missing edge description should mention the unresolved source id"
    );
    assert!(
        unresolved.description.contains("controller_a"),
        "missing edge description should mention the impacted entity"
    );
    assert_eq!(unresolved.certainty.confidence.get(), 35);
    assert_eq!(
        unresolved.provenance.source,
        AgentProvenanceSource::Heuristic
    );
    assert_eq!(
        unresolved.provenance.detail.as_deref(),
        Some("dynamic dispatch")
    );
}

#[test]
fn blast_radius_returns_invalid_request_for_unknown_entities() {
    let snapshot = snapshot_fixture();
    let service = SnapshotQueryService::new(&snapshot);

    let response = service.blast_radius_estimation(BlastRadiusRequest {
        entity: EntitySelector::Id {
            id: "function:missing".to_string(),
        },
        max_depth: 2,
    });

    let CapabilityResponse::Error { error } = response else {
        panic!("expected invalid request error");
    };

    assert_eq!(error.code, CapabilityErrorCode::InvalidRequest);
}
