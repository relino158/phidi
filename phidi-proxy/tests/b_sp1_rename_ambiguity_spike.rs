use syn::{Expr, ItemFn, parse_str, visit::Visit};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallableKind {
    Function,
    Method,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Candidate {
    entity_id: &'static str,
    qualified_name: &'static str,
    file_path: &'static str,
    callable_kind: CallableKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RenameSite {
    file_path: &'static str,
    caller_qualified_name: &'static str,
    target_name: String,
    target_path: Option<String>,
    callable_kind: CallableKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RenameDecision {
    HighConfidence {
        entity_id: String,
        reason: String,
    },
    LowConfidence {
        entity_id: String,
        reason: String,
    },
    Conflict {
        candidate_ids: Vec<String>,
        message: String,
    },
}

#[test]
fn qualified_function_path_ranks_an_exact_match_highest() {
    let site = parse_site(
        "crate::graphics::render();",
        "src/ui/controller.rs",
        "ui::controller::refresh",
    );
    let decision = classify_rename_site(
        &site,
        &[
            function_candidate("fn:ui:render", "ui::render", "src/ui/render.rs"),
            function_candidate(
                "fn:graphics:render",
                "graphics::render",
                "src/graphics/render.rs",
            ),
        ],
    );

    assert_eq!(
        decision,
        RenameDecision::HighConfidence {
            entity_id: "fn:graphics:render".to_string(),
            reason: "explicit path uniquely identifies the target".to_string(),
        }
    );
}

#[test]
fn unqualified_function_call_uses_locality_but_stays_low_confidence() {
    let site = parse_site(
        "render();",
        "src/ui/controller.rs",
        "ui::controller::refresh",
    );
    let decision = classify_rename_site(
        &site,
        &[
            function_candidate("fn:ui:render", "ui::render", "src/ui/controller.rs"),
            function_candidate(
                "fn:graphics:render",
                "graphics::render",
                "src/graphics/render.rs",
            ),
        ],
    );

    assert_eq!(
        decision,
        RenameDecision::LowConfidence {
            entity_id: "fn:ui:render".to_string(),
            reason: "locality breaks the tie, but the syntax has no explicit path"
                .to_string(),
        }
    );
}

#[test]
fn tied_function_candidates_surface_a_conflict() {
    let site = parse_site("render();", "src/app.rs", "app::refresh");
    let decision = classify_rename_site(
        &site,
        &[
            function_candidate("fn:ui:render", "ui::render", "src/ui/render.rs"),
            function_candidate(
                "fn:graphics:render",
                "graphics::render",
                "src/graphics/render.rs",
            ),
        ],
    );

    assert_eq!(
        decision,
        RenameDecision::Conflict {
            candidate_ids: vec![
                "fn:graphics:render".to_string(),
                "fn:ui:render".to_string(),
            ],
            message: "top-ranked candidates are tied on syntax-only evidence"
                .to_string(),
        }
    );
}

#[test]
fn same_name_method_candidates_surface_a_conflict_without_type_information() {
    let site = parse_site(
        "widget.render();",
        "src/ui/controller.rs",
        "ui::controller::refresh",
    );
    let decision = classify_rename_site(
        &site,
        &[
            method_candidate(
                "method:widget:render",
                "ui::Widget::render",
                "src/ui/widget.rs",
            ),
            method_candidate(
                "method:overlay:render",
                "ui::Overlay::render",
                "src/ui/overlay.rs",
            ),
        ],
    );

    assert_eq!(
        decision,
        RenameDecision::Conflict {
            candidate_ids: vec![
                "method:overlay:render".to_string(),
                "method:widget:render".to_string(),
            ],
            message: "method receiver types are unavailable in syntax-only ranking"
                .to_string(),
        }
    );
}

fn function_candidate(
    entity_id: &'static str,
    qualified_name: &'static str,
    file_path: &'static str,
) -> Candidate {
    Candidate {
        entity_id,
        qualified_name,
        file_path,
        callable_kind: CallableKind::Function,
    }
}

fn method_candidate(
    entity_id: &'static str,
    qualified_name: &'static str,
    file_path: &'static str,
) -> Candidate {
    Candidate {
        entity_id,
        qualified_name,
        file_path,
        callable_kind: CallableKind::Method,
    }
}

fn parse_site(
    statement: &str,
    file_path: &'static str,
    caller_qualified_name: &'static str,
) -> RenameSite {
    let function: ItemFn = parse_str(&format!("fn probe() {{ {statement} }}"))
        .expect("statement should parse as a function body");
    let mut visitor = FirstCallSite::default();
    visitor.visit_item_fn(&function);
    let mut site = visitor.site.expect("expected one call site");
    site.file_path = file_path;
    site.caller_qualified_name = caller_qualified_name;
    site
}

#[derive(Default)]
struct FirstCallSite {
    site: Option<RenameSite>,
}

impl<'ast> Visit<'ast> for FirstCallSite {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if self.site.is_none() {
            if let Expr::Path(path_expr) = node.func.as_ref() {
                let target_name = path_expr
                    .path
                    .segments
                    .last()
                    .expect("call path should contain one segment")
                    .ident
                    .to_string();
                self.site = Some(RenameSite {
                    file_path: "",
                    caller_qualified_name: "",
                    target_name,
                    target_path: Some(path_label(&path_expr.path)),
                    callable_kind: CallableKind::Function,
                });
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.site.is_none() {
            self.site = Some(RenameSite {
                file_path: "",
                caller_qualified_name: "",
                target_name: node.method.to_string(),
                target_path: None,
                callable_kind: CallableKind::Method,
            });
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn path_label(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn classify_rename_site(
    site: &RenameSite,
    candidates: &[Candidate],
) -> RenameDecision {
    let mut matching_candidates = candidates
        .iter()
        .filter(|candidate| {
            candidate.callable_kind == site.callable_kind
                && candidate_name(candidate) == site.target_name.as_str()
        })
        .collect::<Vec<_>>();

    if matching_candidates.is_empty() {
        return RenameDecision::Conflict {
            candidate_ids: Vec::new(),
            message: "no same-name candidates matched the call shape".to_string(),
        };
    }

    if site.callable_kind == CallableKind::Method && matching_candidates.len() > 1 {
        return RenameDecision::Conflict {
            candidate_ids: sorted_candidate_ids(&matching_candidates),
            message: "method receiver types are unavailable in syntax-only ranking"
                .to_string(),
        };
    }

    let mut scored = matching_candidates
        .drain(..)
        .map(|candidate| ScoredCandidate {
            candidate,
            score: score_candidate(site, candidate),
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| {
                left.candidate
                    .qualified_name
                    .cmp(right.candidate.qualified_name)
            })
            .then_with(|| left.candidate.entity_id.cmp(right.candidate.entity_id))
    });

    let top = &scored[0];
    let tied = scored
        .iter()
        .filter(|entry| entry.score == top.score)
        .collect::<Vec<_>>();
    if tied.len() > 1 {
        return RenameDecision::Conflict {
            candidate_ids: sorted_scored_candidate_ids(&tied),
            message: "top-ranked candidates are tied on syntax-only evidence"
                .to_string(),
        };
    }

    if top.score.exact_path_match {
        RenameDecision::HighConfidence {
            entity_id: top.candidate.entity_id.to_string(),
            reason: "explicit path uniquely identifies the target".to_string(),
        }
    } else {
        RenameDecision::LowConfidence {
            entity_id: top.candidate.entity_id.to_string(),
            reason: "locality breaks the tie, but the syntax has no explicit path"
                .to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CandidateScore {
    exact_path_match: bool,
    same_file: bool,
    shared_module_prefix: usize,
}

struct ScoredCandidate<'a> {
    candidate: &'a Candidate,
    score: CandidateScore,
}

fn score_candidate(site: &RenameSite, candidate: &Candidate) -> CandidateScore {
    CandidateScore {
        exact_path_match: qualified_path_hint(site).is_some_and(|hint| {
            let candidate_name = normalize_qualified_name(candidate.qualified_name);
            candidate_name == hint || candidate_name.ends_with(&format!("::{hint}"))
        }),
        same_file: candidate.file_path == site.file_path,
        shared_module_prefix: shared_module_prefix_len(
            module_segments(site.caller_qualified_name),
            module_segments(candidate.qualified_name),
        ),
    }
}

fn qualified_path_hint(site: &RenameSite) -> Option<&str> {
    site.target_path
        .as_deref()
        .map(normalize_qualified_name)
        .filter(|path| path.contains("::"))
}

fn normalize_qualified_name(name: &str) -> &str {
    name.trim_start_matches("crate::")
}

fn candidate_name(candidate: &Candidate) -> &str {
    candidate
        .qualified_name
        .rsplit("::")
        .next()
        .expect("qualified candidate name should have a leaf segment")
}

fn module_segments(qualified_name: &str) -> Vec<&str> {
    let mut segments = normalize_qualified_name(qualified_name)
        .split("::")
        .collect::<Vec<_>>();
    let _ = segments.pop();
    segments
}

fn shared_module_prefix_len(left: Vec<&str>, right: Vec<&str>) -> usize {
    left.into_iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

fn sorted_candidate_ids(candidates: &[&Candidate]) -> Vec<String> {
    let mut ids = candidates
        .iter()
        .map(|candidate| candidate.entity_id.to_string())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

fn sorted_scored_candidate_ids(candidates: &[&ScoredCandidate<'_>]) -> Vec<String> {
    let mut ids = candidates
        .iter()
        .map(|candidate| candidate.candidate.entity_id.to_string())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}
