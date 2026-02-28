use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use phidi_core::semantic_map::{
    Certainty, CertaintyKind, ConfidenceScore, DiagnosticSeverity, EntityId,
    EntityKind, EntityLocation, ProvenanceSource, RelationshipKind,
    RelationshipProvenance, SemanticEntity, SemanticRelationship,
    SnapshotCompleteness, SnapshotDiagnostic, SnapshotKind, SnapshotProvenance,
    WorkspaceSnapshot,
};
use syn::{
    Attribute, Expr, ExprCall, ExprMethodCall, File, ImplItem, Item, ItemFn,
    ItemImpl, ItemMacro, ItemMod, ItemTrait, ItemUse, Path as SynPath, Token,
    TraitItem, Type, UseTree, parse_file,
    punctuated::Punctuated,
    visit::{self, Visit},
};
use walkdir::WalkDir;

pub struct RustSnapshotExtractor;

impl RustSnapshotExtractor {
    pub const fn new() -> Self {
        Self
    }

    pub fn extract_workspace(
        &self,
        workspace_root: &Path,
    ) -> Result<WorkspaceSnapshot> {
        let workspace_root = workspace_root.canonicalize().with_context(|| {
            format!("failed to read workspace root {}", workspace_root.display())
        })?;
        let mut builder = SnapshotBuilder::new(workspace_root);
        builder.extract_workspace()?;
        Ok(builder.finish())
    }
}

impl Default for RustSnapshotExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct ScopeContext {
    file_path: String,
    module_path: Vec<String>,
    container_id: String,
}

#[derive(Clone, Copy)]
enum CallableKind {
    Function,
    Method,
}

struct CallableRecord {
    entity_id: String,
    qualified_name: Option<String>,
}

#[derive(Clone)]
struct CallObservation {
    caller_id: String,
    target_name: String,
    target_path: Option<String>,
    callable_kind: CallableKind,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct RelationshipKey {
    source: String,
    target: String,
    kind: &'static str,
    certainty_kind: &'static str,
    confidence: u8,
    provenance_source: &'static str,
    detail: Option<String>,
}

struct SnapshotBuilder {
    workspace_root: PathBuf,
    snapshot: WorkspaceSnapshot,
    entities: BTreeMap<String, SemanticEntity>,
    relationships: BTreeMap<RelationshipKey, SemanticRelationship>,
    callables_by_name: BTreeMap<String, Vec<CallableRecord>>,
    trait_entities_by_name: BTreeMap<String, Vec<String>>,
    call_observations: Vec<CallObservation>,
}

impl SnapshotBuilder {
    fn new(workspace_root: PathBuf) -> Self {
        let mut snapshot = WorkspaceSnapshot::new(
            SnapshotKind::Working,
            SnapshotProvenance::default(),
        );
        let workspace_id = EntityId("workspace".to_string());
        let workspace_name = workspace_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_string();
        let workspace_entity = SemanticEntity {
            id: workspace_id.clone(),
            kind: EntityKind::Workspace,
            name: workspace_name,
            qualified_name: None,
            location: None,
        };

        let mut entities = BTreeMap::new();
        entities.insert(workspace_id.0.clone(), workspace_entity);
        snapshot.entities = Vec::new();

        Self {
            workspace_root,
            snapshot,
            entities,
            relationships: BTreeMap::new(),
            callables_by_name: BTreeMap::new(),
            trait_entities_by_name: BTreeMap::new(),
            call_observations: Vec::new(),
        }
    }

    fn extract_workspace(&mut self) -> Result<()> {
        for file_path in self.rust_files()? {
            self.extract_file(&file_path);
        }

        self.resolve_inferred_calls();
        Ok(())
    }

    fn finish(mut self) -> WorkspaceSnapshot {
        self.snapshot.entities = self.entities.into_values().collect();
        self.snapshot.relationships = self.relationships.into_values().collect();
        self.snapshot.diagnostics.sort_by(|left, right| {
            let left_path = left
                .location
                .as_ref()
                .map(|location| location.path.as_str())
                .unwrap_or("");
            let right_path = right
                .location
                .as_ref()
                .map(|location| location.path.as_str())
                .unwrap_or("");
            left_path
                .cmp(right_path)
                .then_with(|| left.message.cmp(&right.message))
        });
        self.snapshot
    }

    fn rust_files(&self) -> Result<Vec<PathBuf>> {
        let mut rust_files = Vec::new();
        for entry in WalkDir::new(&self.workspace_root) {
            let entry = entry?;
            if entry.file_type().is_file()
                && entry.path().extension().and_then(|ext| ext.to_str())
                    == Some("rs")
            {
                rust_files.push(entry.path().to_path_buf());
            }
        }
        rust_files.sort();
        Ok(rust_files)
    }

    fn extract_file(&mut self, file_path: &Path) {
        let relative_path = self.relative_path(file_path);
        let file_id = format!("file:{relative_path}");
        self.insert_entity(
            file_id.clone(),
            EntityKind::File,
            file_name(file_path),
            Some(relative_path.clone()),
            Some(relative_path.clone()),
        );
        self.insert_relationship(
            "workspace".to_string(),
            file_id.clone(),
            RelationshipKind::Contains,
            Certainty::observed(),
            ProvenanceSource::SyntaxTree,
            None,
        );

        let source = match fs::read_to_string(file_path) {
            Ok(source) => source,
            Err(error) => {
                self.push_partial_diagnostic(
                    Some("read-file"),
                    format!("failed to read Rust source file: {error}"),
                    &relative_path,
                );
                return;
            }
        };

        let syntax_tree = match parse_file(&source) {
            Ok(syntax_tree) => syntax_tree,
            Err(error) => {
                self.push_partial_diagnostic(
                    Some("parse-file"),
                    format!("failed to parse Rust source file: {error}"),
                    &relative_path,
                );
                return;
            }
        };

        let context = ScopeContext {
            file_path: relative_path.clone(),
            module_path: module_path_for_file(&relative_path),
            container_id: file_id,
        };
        self.extract_items(&syntax_tree, &context);
    }

    fn extract_items(&mut self, syntax_tree: &File, context: &ScopeContext) {
        for item in &syntax_tree.items {
            self.extract_item(item, context);
        }
    }

    fn extract_item(&mut self, item: &Item, context: &ScopeContext) {
        match item {
            Item::Enum(item_enum) => {
                let qualified_name = qualified_name(
                    &context.module_path,
                    &item_enum.ident.to_string(),
                );
                let entity_id =
                    make_entity_id("enum", &context.file_path, &qualified_name);
                self.insert_entity(
                    entity_id.clone(),
                    EntityKind::Enum,
                    item_enum.ident.to_string(),
                    Some(qualified_name),
                    Some(context.file_path.clone()),
                );
                self.define_entity(&context.container_id, &entity_id);
                self.record_macro_references(
                    &entity_id,
                    &context.file_path,
                    &item_enum.attrs,
                );
            }
            Item::Fn(item_fn) => self.extract_function(item_fn, context),
            Item::Impl(item_impl) => self.extract_impl(item_impl, context),
            Item::Macro(item_macro) => self.extract_item_macro(item_macro, context),
            Item::Mod(item_mod) => self.extract_module(item_mod, context),
            Item::Struct(item_struct) => {
                let qualified_name = qualified_name(
                    &context.module_path,
                    &item_struct.ident.to_string(),
                );
                let entity_id =
                    make_entity_id("struct", &context.file_path, &qualified_name);
                self.insert_entity(
                    entity_id.clone(),
                    EntityKind::Struct,
                    item_struct.ident.to_string(),
                    Some(qualified_name),
                    Some(context.file_path.clone()),
                );
                self.define_entity(&context.container_id, &entity_id);
                self.record_macro_references(
                    &entity_id,
                    &context.file_path,
                    &item_struct.attrs,
                );
            }
            Item::Trait(item_trait) => self.extract_trait(item_trait, context),
            Item::Use(item_use) => self.extract_use(item_use, context),
            _ => {}
        }
    }

    fn extract_function(&mut self, item_fn: &ItemFn, context: &ScopeContext) {
        let name = item_fn.sig.ident.to_string();
        let kind = if has_attr(&item_fn.attrs, "test") {
            EntityKind::Test
        } else {
            EntityKind::Function
        };
        let qualified_name = qualified_name(&context.module_path, &name);
        let entity_id =
            make_entity_id("function", &context.file_path, &qualified_name);
        self.insert_entity(
            entity_id.clone(),
            kind,
            name.clone(),
            Some(qualified_name.clone()),
            Some(context.file_path.clone()),
        );
        self.define_entity(&context.container_id, &entity_id);
        self.record_callable(&name, &entity_id, Some(qualified_name));
        self.record_macro_references(&entity_id, &context.file_path, &item_fn.attrs);
        self.collect_calls_from_block(
            &entity_id,
            CallableKind::Function,
            &item_fn.block,
        );
    }

    fn extract_impl(&mut self, item_impl: &ItemImpl, context: &ScopeContext) {
        let self_type = type_label(&item_impl.self_ty);
        let implemented_trait = item_impl
            .trait_
            .as_ref()
            .map(|(_, path, _)| path_label(path));
        let impl_name = match &implemented_trait {
            Some(trait_name) => format!("{self_type} as {trait_name}"),
            None => self_type.clone(),
        };
        let qualified_name = qualified_name(&context.module_path, &impl_name);
        let entity_id = make_entity_id("impl", &context.file_path, &qualified_name);
        self.insert_entity(
            entity_id.clone(),
            EntityKind::ImplBlock,
            "impl".to_string(),
            Some(qualified_name),
            Some(context.file_path.clone()),
        );
        self.define_entity(&context.container_id, &entity_id);
        self.record_macro_references(
            &entity_id,
            &context.file_path,
            &item_impl.attrs,
        );

        if let Some(trait_name) = implemented_trait {
            let trait_entity = self.ensure_trait_entity(&trait_name);
            self.insert_relationship(
                entity_id.clone(),
                trait_entity,
                RelationshipKind::Implements,
                Certainty::observed(),
                ProvenanceSource::SyntaxTree,
                Some("explicit trait impl".to_string()),
            );
        }

        for impl_item in &item_impl.items {
            if let ImplItem::Fn(method) = impl_item {
                let name = method.sig.ident.to_string();
                let method_qualified_name = format!("{impl_name}::{name}");
                let method_id = make_entity_id(
                    "method",
                    &context.file_path,
                    &method_qualified_name,
                );
                self.insert_entity(
                    method_id.clone(),
                    EntityKind::Method,
                    name.clone(),
                    Some(method_qualified_name.clone()),
                    Some(context.file_path.clone()),
                );
                self.define_entity(&entity_id, &method_id);
                self.record_callable(&name, &method_id, Some(method_qualified_name));
                self.record_macro_references(
                    &method_id,
                    &context.file_path,
                    &method.attrs,
                );
                self.collect_calls_from_block(
                    &method_id,
                    CallableKind::Method,
                    &method.block,
                );
            }
        }
    }

    fn extract_item_macro(
        &mut self,
        item_macro: &ItemMacro,
        context: &ScopeContext,
    ) {
        if item_macro.mac.path.is_ident("macro_rules") {
            if let Some(ident) = &item_macro.ident {
                let macro_entity =
                    self.ensure_macro_entity(&context.file_path, &ident.to_string());
                self.define_entity(&context.container_id, &macro_entity);
            }
            return;
        }

        let macro_entity = self.ensure_macro_entity(
            &context.file_path,
            &path_label(&item_macro.mac.path),
        );
        self.insert_relationship(
            context.container_id.clone(),
            macro_entity,
            RelationshipKind::References,
            Certainty::observed(),
            ProvenanceSource::SyntaxTree,
            Some("item macro invocation".to_string()),
        );
    }

    fn extract_module(&mut self, item_mod: &ItemMod, context: &ScopeContext) {
        let name = item_mod.ident.to_string();
        let qualified_name = qualified_name(&context.module_path, &name);
        let entity_id =
            make_entity_id("module", &context.file_path, &qualified_name);
        self.insert_entity(
            entity_id.clone(),
            EntityKind::Module,
            name.clone(),
            Some(qualified_name),
            Some(context.file_path.clone()),
        );
        self.define_entity(&context.container_id, &entity_id);
        self.record_macro_references(
            &entity_id,
            &context.file_path,
            &item_mod.attrs,
        );

        if let Some((_, items)) = &item_mod.content {
            let nested_context = ScopeContext {
                file_path: context.file_path.clone(),
                module_path: extend_path(&context.module_path, &name),
                container_id: entity_id,
            };
            for item in items {
                self.extract_item(item, &nested_context);
            }
        }
    }

    fn extract_trait(&mut self, item_trait: &ItemTrait, context: &ScopeContext) {
        let name = item_trait.ident.to_string();
        let qualified_name = qualified_name(&context.module_path, &name);
        let entity_id = make_entity_id("trait", &context.file_path, &qualified_name);
        self.insert_entity(
            entity_id.clone(),
            EntityKind::Trait,
            name.clone(),
            Some(qualified_name.clone()),
            Some(context.file_path.clone()),
        );
        self.define_entity(&context.container_id, &entity_id);
        self.record_trait_entity(&name, &entity_id);
        self.record_macro_references(
            &entity_id,
            &context.file_path,
            &item_trait.attrs,
        );

        for trait_item in &item_trait.items {
            if let TraitItem::Fn(method) = trait_item {
                let method_name = method.sig.ident.to_string();
                let method_qualified_name =
                    format!("{qualified_name}::{method_name}");
                let method_id = make_entity_id(
                    "method",
                    &context.file_path,
                    &method_qualified_name,
                );
                self.insert_entity(
                    method_id.clone(),
                    EntityKind::Method,
                    method_name.clone(),
                    Some(method_qualified_name.clone()),
                    Some(context.file_path.clone()),
                );
                self.define_entity(&entity_id, &method_id);
                self.record_callable(
                    &method_name,
                    &method_id,
                    Some(method_qualified_name),
                );
                self.record_macro_references(
                    &method_id,
                    &context.file_path,
                    &method.attrs,
                );
                if let Some(default) = &method.default {
                    self.collect_calls_from_block(
                        &method_id,
                        CallableKind::Method,
                        default,
                    );
                }
            }
        }
    }

    fn extract_use(&mut self, item_use: &ItemUse, context: &ScopeContext) {
        let mut imports = BTreeSet::new();
        collect_use_paths(&item_use.tree, String::new(), &mut imports);
        for import_path in imports {
            let import_id =
                make_entity_id("import", &context.file_path, &import_path);
            self.insert_entity(
                import_id.clone(),
                EntityKind::Import,
                import_path.clone(),
                Some(import_path.clone()),
                Some(context.file_path.clone()),
            );
            self.insert_relationship(
                context.container_id.clone(),
                import_id,
                RelationshipKind::Imports,
                Certainty::observed(),
                ProvenanceSource::SyntaxTree,
                None,
            );
        }
    }

    fn record_macro_references(
        &mut self,
        owner_entity_id: &str,
        file_path: &str,
        attrs: &[Attribute],
    ) {
        for attr in attrs {
            let attr_name = path_label(attr.path());
            if attr_name == "test" || is_builtin_attribute(&attr_name) {
                continue;
            }

            if attr.path().is_ident("derive") {
                match attr.parse_args_with(
                    Punctuated::<SynPath, Token![,]>::parse_terminated,
                ) {
                    Ok(paths) => {
                        for derive_path in paths {
                            let derive_name = path_label(&derive_path);
                            let macro_entity =
                                self.ensure_macro_entity(file_path, &derive_name);
                            self.insert_relationship(
                                owner_entity_id.to_string(),
                                macro_entity,
                                RelationshipKind::References,
                                Certainty::observed(),
                                ProvenanceSource::SyntaxTree,
                                Some("derive macro attachment".to_string()),
                            );
                        }
                    }
                    Err(error) => {
                        self.push_partial_diagnostic(
                            Some("derive-parse"),
                            format!("failed to parse derive arguments: {error}"),
                            file_path,
                        );
                    }
                }
                continue;
            }

            let macro_entity = self.ensure_macro_entity(file_path, &attr_name);
            self.insert_relationship(
                owner_entity_id.to_string(),
                macro_entity,
                RelationshipKind::References,
                Certainty::observed(),
                ProvenanceSource::SyntaxTree,
                Some("attribute macro attachment".to_string()),
            );
        }
    }

    fn collect_calls_from_block(
        &mut self,
        caller_id: &str,
        callable_kind: CallableKind,
        block: &syn::Block,
    ) {
        let mut collector = CallCollector {
            caller_id: caller_id.to_string(),
            callable_kind,
            observations: Vec::new(),
        };
        collector.visit_block(block);
        self.call_observations.extend(collector.observations);
    }

    fn resolve_inferred_calls(&mut self) {
        for observation in self.call_observations.clone() {
            let candidates = if let Some(target_path) = &observation.target_path {
                let exact_matches: Vec<String> = self
                    .callables_by_name
                    .get(&observation.target_name)
                    .into_iter()
                    .flatten()
                    .filter(|candidate| {
                        candidate
                            .qualified_name
                            .as_deref()
                            .map(|qualified_name| {
                                qualified_name == target_path
                                    || qualified_name.ends_with(target_path)
                            })
                            .unwrap_or(false)
                    })
                    .map(|candidate| candidate.entity_id.clone())
                    .collect();
                if exact_matches.is_empty() {
                    self.callables_by_name
                        .get(&observation.target_name)
                        .map(|candidates| {
                            candidates
                                .iter()
                                .map(|candidate| candidate.entity_id.clone())
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    exact_matches
                }
            } else {
                self.callables_by_name
                    .get(&observation.target_name)
                    .map(|candidates| {
                        candidates
                            .iter()
                            .map(|candidate| candidate.entity_id.clone())
                            .collect()
                    })
                    .unwrap_or_default()
            };

            if candidates.len() != 1 {
                continue;
            }

            let confidence =
                match (observation.callable_kind, observation.target_path.is_some())
                {
                    (CallableKind::Function, true) => {
                        ConfidenceScore::new(85).unwrap()
                    }
                    (CallableKind::Function, false) => {
                        ConfidenceScore::new(72).unwrap()
                    }
                    (CallableKind::Method, true) => {
                        ConfidenceScore::new(74).unwrap()
                    }
                    (CallableKind::Method, false) => {
                        ConfidenceScore::new(64).unwrap()
                    }
                };

            self.insert_relationship(
                observation.caller_id,
                candidates[0].clone(),
                RelationshipKind::Calls,
                Certainty::inferred(confidence),
                ProvenanceSource::Heuristic,
                Some(format!(
                    "resolved unique callable named {}",
                    observation.target_name
                )),
            );
        }
    }

    fn define_entity(&mut self, container_id: &str, entity_id: &str) {
        self.insert_relationship(
            container_id.to_string(),
            entity_id.to_string(),
            RelationshipKind::Defines,
            Certainty::observed(),
            ProvenanceSource::SyntaxTree,
            None,
        );
    }

    fn ensure_macro_entity(&mut self, file_path: &str, macro_name: &str) -> String {
        let entity_id = make_entity_id("macro", file_path, macro_name);
        self.insert_entity(
            entity_id.clone(),
            EntityKind::Macro,
            macro_name.to_string(),
            Some(macro_name.to_string()),
            Some(file_path.to_string()),
        );
        entity_id
    }

    fn ensure_trait_entity(&mut self, trait_name: &str) -> String {
        if let Some(entity_id) = self
            .trait_entities_by_name
            .get(trait_name)
            .and_then(|entities| entities.first())
        {
            return entity_id.clone();
        }

        let entity_id = format!("trait:external:{trait_name}");
        self.insert_entity(
            entity_id.clone(),
            EntityKind::Trait,
            trait_name.to_string(),
            Some(trait_name.to_string()),
            None,
        );
        self.record_trait_entity(trait_name, &entity_id);
        entity_id
    }

    fn insert_entity(
        &mut self,
        entity_id: String,
        kind: EntityKind,
        name: String,
        qualified_name: Option<String>,
        path: Option<String>,
    ) {
        let entity = SemanticEntity {
            id: EntityId(entity_id.clone()),
            kind,
            name: name.clone(),
            qualified_name,
            location: path.map(|path| EntityLocation { path, span: None }),
        };

        if matches!(
            kind,
            EntityKind::Function | EntityKind::Method | EntityKind::Test
        ) {
            self.record_callable(&name, &entity_id, entity.qualified_name.clone());
        }
        if kind == EntityKind::Trait {
            self.record_trait_entity(&name, &entity_id);
        }

        self.entities.entry(entity_id).or_insert(entity);
    }

    fn insert_relationship(
        &mut self,
        source: String,
        target: String,
        kind: RelationshipKind,
        certainty: Certainty,
        provenance_source: ProvenanceSource,
        detail: Option<String>,
    ) {
        let key = RelationshipKey {
            source: source.clone(),
            target: target.clone(),
            kind: relationship_kind_label(kind),
            certainty_kind: certainty_kind_label(certainty.kind),
            confidence: certainty.confidence.get(),
            provenance_source: provenance_source_label(provenance_source),
            detail: detail.clone(),
        };
        let relationship = SemanticRelationship {
            source: EntityId(source),
            target: EntityId(target),
            kind,
            certainty,
            provenance: RelationshipProvenance {
                source: provenance_source,
                detail,
            },
        };
        self.relationships.entry(key).or_insert(relationship);
    }

    fn push_partial_diagnostic(
        &mut self,
        code: Option<&str>,
        message: String,
        path: &str,
    ) {
        self.snapshot.completeness = SnapshotCompleteness::Partial;
        self.snapshot.diagnostics.push(SnapshotDiagnostic {
            code: code.map(str::to_string),
            severity: DiagnosticSeverity::Warning,
            message,
            location: Some(EntityLocation {
                path: path.to_string(),
                span: None,
            }),
        });
    }

    fn record_callable(
        &mut self,
        name: &str,
        entity_id: &str,
        qualified_name: Option<String>,
    ) {
        let candidates = self.callables_by_name.entry(name.to_string()).or_default();
        if candidates
            .iter()
            .any(|candidate| candidate.entity_id == entity_id)
        {
            return;
        }
        candidates.push(CallableRecord {
            entity_id: entity_id.to_string(),
            qualified_name,
        });
    }

    fn record_trait_entity(&mut self, trait_name: &str, entity_id: &str) {
        let candidates = self
            .trait_entities_by_name
            .entry(trait_name.to_string())
            .or_default();
        if candidates.iter().any(|candidate| candidate == entity_id) {
            return;
        }
        candidates.push(entity_id.to_string());
    }

    fn relative_path(&self, path: &Path) -> String {
        path.strip_prefix(&self.workspace_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

struct CallCollector {
    caller_id: String,
    callable_kind: CallableKind,
    observations: Vec<CallObservation>,
}

impl<'ast> Visit<'ast> for CallCollector {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Expr::Path(path_expr) = node.func.as_ref() {
            let target_name = path_expr
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string());
            if let Some(target_name) = target_name {
                self.observations.push(CallObservation {
                    caller_id: self.caller_id.clone(),
                    target_name,
                    target_path: Some(path_label(&path_expr.path)),
                    callable_kind: self.callable_kind,
                });
            }
        }

        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        self.observations.push(CallObservation {
            caller_id: self.caller_id.clone(),
            target_name: node.method.to_string(),
            target_path: None,
            callable_kind: CallableKind::Method,
        });
        visit::visit_expr_method_call(self, node);
    }
}

fn collect_use_paths(tree: &UseTree, prefix: String, paths: &mut BTreeSet<String>) {
    match tree {
        UseTree::Name(name) => {
            paths.insert(join_use_path(&prefix, &name.ident.to_string()));
        }
        UseTree::Path(path) => {
            collect_use_paths(
                &path.tree,
                join_use_path(&prefix, &path.ident.to_string()),
                paths,
            );
        }
        UseTree::Rename(rename) => {
            paths.insert(join_use_path(&prefix, &rename.ident.to_string()));
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_paths(item, prefix.clone(), paths);
            }
        }
        UseTree::Glob(_) => {
            paths.insert(join_use_path(&prefix, "*"));
        }
    }
}

fn make_entity_id(kind: &str, file_path: &str, name: &str) -> String {
    format!("{kind}:{file_path}:{name}")
}

fn extend_path(path: &[String], name: &str) -> Vec<String> {
    let mut next = path.to_vec();
    next.push(name.to_string());
    next
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file")
        .to_string()
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn is_builtin_attribute(attr_name: &str) -> bool {
    matches!(
        attr_name,
        "allow"
            | "cfg"
            | "cfg_attr"
            | "deny"
            | "doc"
            | "inline"
            | "must_use"
            | "path"
            | "repr"
            | "warn"
    )
}

fn join_use_path(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() {
        segment.to_string()
    } else {
        format!("{prefix}::{segment}")
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

fn path_label(path: &SynPath) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

fn qualified_name(path: &[String], name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{}::{name}", path.join("::"))
    }
}

fn relationship_kind_label(kind: RelationshipKind) -> &'static str {
    match kind {
        RelationshipKind::Calls => "calls",
        RelationshipKind::Contains => "contains",
        RelationshipKind::Defines => "defines",
        RelationshipKind::Implements => "implements",
        RelationshipKind::Imports => "imports",
        RelationshipKind::References => "references",
    }
}

fn certainty_kind_label(kind: CertaintyKind) -> &'static str {
    match kind {
        CertaintyKind::Observed => "observed",
        CertaintyKind::Inferred => "inferred",
    }
}

fn provenance_source_label(source: ProvenanceSource) -> &'static str {
    match source {
        ProvenanceSource::Heuristic => "heuristic",
        ProvenanceSource::SymbolResolution => "symbol-resolution",
        ProvenanceSource::SyntaxTree => "syntax-tree",
    }
}

fn type_label(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) => path_label(&type_path.path),
        Type::Reference(reference) => type_label(&reference.elem),
        _ => "unknown-type".to_string(),
    }
}
