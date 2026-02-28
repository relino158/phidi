use std::collections::BTreeSet;

use syn::{
    Attribute, File, ItemFn, ItemMacro, ItemStruct, Path, Token, parse_file,
    punctuated::Punctuated,
    visit::{self, Visit},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BaselinePolicy {
    Certain,
    OmitFromA05Baseline,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PolicyDecision {
    policy: BaselinePolicy,
    subject: String,
    relation: &'static str,
    target: String,
    reason: &'static str,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ProbeSummary {
    direct_items: BTreeSet<String>,
    decisions: BTreeSet<PolicyDecision>,
}

struct MacroCertaintyProbe<'a> {
    summary: ProbeSummary,
    macro_attributes: &'a BTreeSet<&'a str>,
}

impl<'a> MacroCertaintyProbe<'a> {
    fn new(macro_attributes: &'a BTreeSet<&'a str>) -> Self {
        Self {
            summary: ProbeSummary::default(),
            macro_attributes,
        }
    }

    fn finish(self) -> ProbeSummary {
        self.summary
    }

    fn push_direct_item(&mut self, kind: &'static str, name: &str) {
        self.summary.direct_items.insert(format!("{kind} {name}"));
    }

    fn push_decision(
        &mut self,
        policy: BaselinePolicy,
        subject: impl Into<String>,
        relation: &'static str,
        target: impl Into<String>,
        reason: &'static str,
    ) {
        self.summary.decisions.insert(PolicyDecision {
            policy,
            subject: subject.into(),
            relation,
            target: target.into(),
            reason,
        });
    }

    fn record_attrs(&mut self, item_name: &str, attrs: &[Attribute]) {
        for attr in attrs {
            let attr_name = path_label(attr.path());
            if attr.path().is_ident("derive") {
                let derive_targets = attr
                    .parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
                    .expect("derive attribute should parse");
                for derive_target in derive_targets {
                    let derive_target = path_label(&derive_target);
                    self.push_decision(
                        BaselinePolicy::Certain,
                        item_name,
                        "uses_derive_macro",
                        derive_target.clone(),
                        "derive paths are explicit in the source file",
                    );
                    self.push_decision(
                        BaselinePolicy::OmitFromA05Baseline,
                        item_name,
                        "implements",
                        derive_target,
                        "derive expansion is not visible in a parser-only pass",
                    );
                }
                continue;
            }

            if self.macro_attributes.contains(attr_name.as_str()) {
                self.push_decision(
                    BaselinePolicy::Certain,
                    item_name,
                    "invokes_attribute_macro",
                    attr_name,
                    "attribute attachment is explicit syntax",
                );
                self.push_decision(
                    BaselinePolicy::OmitFromA05Baseline,
                    item_name,
                    "expands_to",
                    "<generated items>",
                    "attribute macro expansion needs compiler-backed analysis",
                );
            }
        }
    }
}

impl<'ast> Visit<'ast> for MacroCertaintyProbe<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let item_name = node.sig.ident.to_string();
        self.push_direct_item("fn", &item_name);
        self.record_attrs(&item_name, &node.attrs);
        visit::visit_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast ItemStruct) {
        let item_name = node.ident.to_string();
        self.push_direct_item("struct", &item_name);
        self.record_attrs(&item_name, &node.attrs);
        visit::visit_item_struct(self, node);
    }

    fn visit_item_macro(&mut self, node: &'ast ItemMacro) {
        if node.mac.path.is_ident("macro_rules") {
            if let Some(ident) = &node.ident {
                self.push_direct_item("macro", &ident.to_string());
            }
            return;
        }

        let macro_name = path_label(&node.mac.path);
        self.push_decision(
            BaselinePolicy::Certain,
            macro_name.clone(),
            "invoked_at_item_scope",
            "file",
            "macro invocation sites are explicit tokens in the source file",
        );
        self.push_decision(
            BaselinePolicy::OmitFromA05Baseline,
            macro_name,
            "expands_to",
            "<generated items>",
            "expanded items are not materialized in a parser-only AST",
        );
    }
}

fn path_label(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn run_probe(source: &str, macro_attributes: &[&str]) -> ProbeSummary {
    let syntax_tree: File = parse_file(source).expect("fixture should parse");
    let macro_attributes = BTreeSet::from_iter(macro_attributes.iter().copied());
    let mut probe = MacroCertaintyProbe::new(&macro_attributes);
    probe.visit_file(&syntax_tree);
    probe.finish()
}

#[test]
fn macro_heavy_rust_only_yields_certain_links_for_syntax_visible_edges() {
    let summary = run_probe(
        r#"
macro_rules! make_helper {
    ($name:ident) => {
        fn $name() {}
    };
}

#[derive(Debug, Clone)]
struct DerivedThing {
    field: u32,
}

#[instrument]
fn traced() {}

make_helper!(generated_helper);
"#,
        &["instrument"],
    );

    assert_eq!(
        summary.direct_items,
        BTreeSet::from([
            "fn traced".to_string(),
            "macro make_helper".to_string(),
            "struct DerivedThing".to_string(),
        ]),
    );

    assert_eq!(
        summary.decisions,
        BTreeSet::from([
            PolicyDecision {
                policy: BaselinePolicy::Certain,
                subject: "DerivedThing".to_string(),
                relation: "uses_derive_macro",
                target: "Clone".to_string(),
                reason: "derive paths are explicit in the source file",
            },
            PolicyDecision {
                policy: BaselinePolicy::Certain,
                subject: "DerivedThing".to_string(),
                relation: "uses_derive_macro",
                target: "Debug".to_string(),
                reason: "derive paths are explicit in the source file",
            },
            PolicyDecision {
                policy: BaselinePolicy::Certain,
                subject: "make_helper".to_string(),
                relation: "invoked_at_item_scope",
                target: "file".to_string(),
                reason: "macro invocation sites are explicit tokens in the source file",
            },
            PolicyDecision {
                policy: BaselinePolicy::Certain,
                subject: "traced".to_string(),
                relation: "invokes_attribute_macro",
                target: "instrument".to_string(),
                reason: "attribute attachment is explicit syntax",
            },
            PolicyDecision {
                policy: BaselinePolicy::OmitFromA05Baseline,
                subject: "DerivedThing".to_string(),
                relation: "implements",
                target: "Clone".to_string(),
                reason: "derive expansion is not visible in a parser-only pass",
            },
            PolicyDecision {
                policy: BaselinePolicy::OmitFromA05Baseline,
                subject: "DerivedThing".to_string(),
                relation: "implements",
                target: "Debug".to_string(),
                reason: "derive expansion is not visible in a parser-only pass",
            },
            PolicyDecision {
                policy: BaselinePolicy::OmitFromA05Baseline,
                subject: "make_helper".to_string(),
                relation: "expands_to",
                target: "<generated items>".to_string(),
                reason: "expanded items are not materialized in a parser-only AST",
            },
            PolicyDecision {
                policy: BaselinePolicy::OmitFromA05Baseline,
                subject: "traced".to_string(),
                relation: "expands_to",
                target: "<generated items>".to_string(),
                reason: "attribute macro expansion needs compiler-backed analysis",
            },
        ]),
    );
}

#[test]
fn generated_macro_output_is_absent_from_direct_item_inventory() {
    let summary = run_probe(
        r#"
macro_rules! make_helper {
    ($name:ident) => {
        fn $name() {}
    };
}

make_helper!(generated_helper);
"#,
        &[],
    );

    assert!(summary.direct_items.contains("macro make_helper"));
    assert!(!summary.direct_items.contains("fn generated_helper"));
    assert!(summary.decisions.contains(&PolicyDecision {
        policy: BaselinePolicy::Certain,
        subject: "make_helper".to_string(),
        relation: "invoked_at_item_scope",
        target: "file".to_string(),
        reason: "macro invocation sites are explicit tokens in the source file",
    }));
    assert!(summary.decisions.contains(&PolicyDecision {
        policy: BaselinePolicy::OmitFromA05Baseline,
        subject: "make_helper".to_string(),
        relation: "expands_to",
        target: "<generated items>".to_string(),
        reason: "expanded items are not materialized in a parser-only AST",
    }));
}
