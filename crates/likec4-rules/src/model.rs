//! Project model: partitions documents into projects and collects the declarations and
//! references that the lint rules operate on (see `docs/DESIGN.md`, "Linter").
//!
//! Building the model is a single pre-order walk per document. Fully qualified names come
//! from a stack of the enclosing element / `extend` bodies that the walk maintains, so the
//! cost is linear in the size of the document regardless of nesting depth.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use likec4_syntax::ast::{self, AstNode, LeafProperty, WhereLeaf};
use likec4_syntax::SyntaxKind::*;
use likec4_syntax::{Parse, SyntaxNode, SyntaxToken, WalkEvent};
use rayon::prelude::*;
use text_size::TextRange;

use crate::SourceFile;

/// Index of a document in [`Workspace::documents`].
pub type DocId = usize;
/// Index of a project in [`Workspace::projects`].
pub type ProjectId = usize;

/// A location in a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    pub doc: DocId,
    pub range: TextRange,
}

/// A named declaration or reference with its location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub site: Site,
}

/// What a kind identifier refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KindScope {
    /// An element kind (`system name`).
    Element,
    /// A deployment node kind (`node name` inside `deployment`).
    DeploymentNode,
    /// A relationship kind (`-[kind]->`, `.kind`, `where kind is ...` on relations).
    Relationship,
    /// Either an element or a deployment node kind (`element.kind = x`, `where kind is x`).
    Node,
}

/// A reference to a kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KindRef {
    pub name: String,
    pub site: Site,
    pub scope: KindScope,
}

/// What a name reference must resolve to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefTarget {
    /// A model element.
    Element,
    /// A deployment node, a deployed instance or a model element (deployment relations and
    /// deployment view rules).
    Deployment,
    /// A view.
    View,
    /// A global predicate group (static or dynamic).
    PredicateGroup,
    /// A global style or style group.
    GlobalStyle,
}

/// A reference that must resolve to a declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// Last segment of the referenced name.
    pub name: String,
    /// Full referenced text (`a.b.c`).
    pub text: String,
    pub site: Site,
    pub target: RefTarget,
    /// Human readable role, e.g. `Relation target`.
    pub role: &'static str,
}

/// Declarations of `specification` and `global` blocks: name -> declaration sites in order.
#[derive(Debug, Default)]
pub struct SpecDecls {
    pub element_kinds: BTreeMap<String, Vec<Site>>,
    pub deployment_node_kinds: BTreeMap<String, Vec<Site>>,
    pub relationship_kinds: BTreeMap<String, Vec<Site>>,
    pub tags: BTreeMap<String, Vec<Site>>,
    pub colors: BTreeMap<String, Vec<Site>>,
    /// `predicateGroup` and `dynamicPredicateGroup` share one namespace.
    pub predicate_groups: BTreeMap<String, Vec<Site>>,
    /// `style` and `styleGroup` share one namespace.
    pub styles: BTreeMap<String, Vec<Site>>,
}

/// An element declaration (`kind name` or `name = kind`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementDecl {
    pub fqn: String,
    /// Last segment of `fqn`.
    pub name: String,
    pub kind: Option<String>,
    /// The name token.
    pub site: Site,
    /// Tags declared in the element body.
    pub tags: Vec<String>,
    /// Inline title string or `title` property present.
    pub has_title: bool,
}

/// `extend a.b { ... }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtendDecl {
    pub target: String,
    pub tags: Vec<String>,
}

/// Kind of a view declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewKind {
    Element,
    Dynamic,
    Deployment,
}

/// A named view declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewDecl {
    pub name: String,
    pub kind: ViewKind,
    /// The name token.
    pub site: Site,
}

/// Name space a relation endpoint is declared and resolved in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Namespace {
    /// Model elements.
    Model,
    /// Deployment nodes and deployed instances.
    Deployment,
}

/// A model or deployment relation (dynamic view steps are not relations: self-steps are a
/// legitimate sequence-diagram construct). Endpoints are kept as written; see
/// [`Project::resolve`] for turning them into FQNs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationDecl {
    pub namespace: Namespace,
    /// FQN of the enclosing element (model) or node / instance (deployment); `None` at the
    /// top level of a block.
    pub enclosing: Option<String>,
    /// Source reference text; `None` when the source is omitted or `this` / `it`, i.e. the
    /// enclosing element itself.
    pub source: Option<String>,
    /// Target reference text; `None` for `this` / `it`.
    pub target: Option<String>,
    /// The target reference.
    pub site: Site,
}

/// Name sets used to resolve references.
#[derive(Debug, Default)]
pub struct NameIndex {
    /// Last FQN segments of all elements.
    pub elements: HashSet<String>,
    /// Last FQN segments of all deployment nodes and instances.
    pub deployment: HashSet<String>,
    pub views: HashSet<String>,
    /// Full FQNs of all elements.
    pub element_fqns: HashSet<String>,
    /// Full FQNs of all deployment nodes and instances.
    pub deployment_fqns: HashSet<String>,
}

/// All declarations and references of one project.
#[derive(Debug, Default)]
pub struct Project {
    pub docs: Vec<DocId>,
    pub spec: SpecDecls,
    pub elements: Vec<ElementDecl>,
    pub extends: Vec<ExtendDecl>,
    pub views: Vec<ViewDecl>,
    /// Deployment nodes by FQN.
    pub deployment_nodes: Vec<Named>,
    /// Deployed instances by FQN.
    pub deployment_instances: Vec<Named>,
    pub relations: Vec<RelationDecl>,
    pub kind_refs: Vec<KindRef>,
    pub tag_refs: Vec<Named>,
    pub color_refs: Vec<Named>,
    pub references: Vec<Reference>,
    pub names: NameIndex,
}

impl Project {
    fn finish(&mut self) {
        self.names.elements = self.elements.iter().map(|e| e.name.clone()).collect();
        self.names.element_fqns = self.elements.iter().map(|e| e.fqn.clone()).collect();
        let deployment = self.deployment_nodes.iter().chain(&self.deployment_instances);
        self.names.deployment = deployment.clone().map(|n| last_segment(&n.name).to_string()).collect();
        self.names.deployment_fqns = deployment.map(|n| n.name.clone()).collect();
        self.names.views = self.views.iter().map(|v| v.name.clone()).collect();
    }

    /// Resolve a reference written inside `enclosing` the way LikeC4 scopes names: the first
    /// segment is looked up among the children of the enclosing element, then among the
    /// children of each of its ancestors, then at the root; every further segment must be a
    /// direct child of the previous one. Returns the FQN, or `None` when a segment is unknown
    /// in this project.
    pub fn resolve(&self, namespace: Namespace, enclosing: Option<&str>, text: &str) -> Option<String> {
        let fqns = match namespace {
            Namespace::Model => &self.names.element_fqns,
            Namespace::Deployment => &self.names.deployment_fqns,
        };
        let mut segments = text.split('.');
        let first = segments.next()?;
        let mut scope = enclosing;
        let mut fqn = loop {
            let candidate = join_fqn(scope, first);
            if fqns.contains(&candidate) {
                break candidate;
            }
            scope = parent_fqn(scope?);
        };
        for segment in segments {
            fqn = join_fqn(Some(&fqn), segment);
            if !fqns.contains(&fqn) {
                return None;
            }
        }
        Some(fqn)
    }
}

/// A parsed document and the project it belongs to.
pub struct Document<'a> {
    pub file: &'a SourceFile,
    pub parse: Parse,
    pub project: ProjectId,
    /// The document contains an `import ... from '...'` statement.
    pub has_imports: bool,
    /// Element names introduced by `import { a, b } from '...'` statements. References whose
    /// first segment is one of these resolve through the import even when the other project
    /// is not loaded.
    pub imported_names: BTreeSet<String>,
}

impl Document<'_> {
    /// Root of the syntax tree.
    pub fn syntax(&self) -> SyntaxNode {
        self.parse.syntax()
    }

    pub fn path(&self) -> &Path {
        &self.file.path
    }
}

/// All documents grouped into projects.
pub struct Workspace<'a> {
    pub documents: Vec<Document<'a>>,
    /// Index 0 is the default project.
    pub projects: Vec<Project>,
}

impl<'a> Workspace<'a> {
    /// Parse all files (in parallel) and build the per-project model.
    pub fn build(files: &'a [SourceFile], roots: &[PathBuf]) -> Self {
        let (project_roots, assignment) = partition(files, roots);
        let parses: Vec<Parse> = files.par_iter().map(|f| likec4_syntax::parse(&f.text)).collect();
        let mut projects: Vec<Project> = project_roots.iter().map(|_| Project::default()).collect();
        let mut documents = Vec::with_capacity(files.len());
        for (id, (file, parse)) in files.iter().zip(parses).enumerate() {
            let project = assignment[id];
            let syntax = parse.syntax();
            let has_imports = syntax.children().any(|n| n.kind() == IMPORTS);
            let imported_names: BTreeSet<String> = syntax
                .children()
                .filter(|n| n.kind() == IMPORTS)
                .flat_map(|imports| {
                    imports.descendants().filter(|n| n.kind() == IMPORTED).collect::<Vec<_>>()
                })
                .flat_map(|imported| {
                    imported
                        .children_with_tokens()
                        .filter_map(|el| el.into_token())
                        .filter(|t| t.kind() == IDENT)
                        .map(|t| t.text().to_string())
                        .collect::<Vec<_>>()
                })
                .collect();
            Collector::new(id, &mut projects[project]).collect(&syntax);
            projects[project].docs.push(id);
            documents.push(Document { file, parse, project, has_imports, imported_names });
        }
        for project in &mut projects {
            project.finish();
        }
        Workspace { documents, projects }
    }

    pub fn path(&self, doc: DocId) -> &Path {
        self.documents[doc].path()
    }

    /// The project a document belongs to.
    pub fn project_of(&self, doc: DocId) -> &Project {
        &self.projects[self.documents[doc].project]
    }
}

/// Assign every file to the deepest project root that contains it. Returns the project
/// roots (index 0 is the default project, `None`) and the project index of every file.
pub fn partition(files: &[SourceFile], roots: &[PathBuf]) -> (Vec<Option<PathBuf>>, Vec<ProjectId>) {
    let mut sorted: Vec<&PathBuf> = roots.iter().collect();
    sorted.sort_by(|a, b| b.components().count().cmp(&a.components().count()).then_with(|| a.cmp(b)));
    sorted.dedup();
    let mut projects = vec![None];
    projects.extend(sorted.iter().map(|r| Some((*r).clone())));
    let assignment = files
        .iter()
        .map(|f| sorted.iter().position(|r| f.path.starts_with(r)).map_or(0, |i| i + 1))
        .collect();
    (projects, assignment)
}

fn last_segment(fqn: &str) -> &str {
    fqn.rsplit('.').next().unwrap_or(fqn)
}

fn parent_fqn(fqn: &str) -> Option<&str> {
    fqn.rsplit_once('.').map(|(parent, _)| parent)
}

fn join_fqn(prefix: Option<&str>, name: &str) -> String {
    match prefix {
        Some(p) => format!("{p}.{name}"),
        None => name.to_string(),
    }
}

fn text_of(token: &SyntaxToken) -> String {
    token.text().to_string()
}

fn tag_names(tags: Option<ast::Tags>) -> Vec<String> {
    tags.map(|t| t.tag_refs().filter_map(|r| r.name()).collect()).unwrap_or_default()
}

/// Name of a deployed instance: the explicit `name =` or the last segment of the target.
fn instance_name(instance: &ast::DeployedInstance) -> Option<String> {
    instance
        .name_token()
        .map(|t| text_of(&t))
        .or_else(|| instance.target().and_then(|t| t.name_token()).map(|t| text_of(&t)))
}

/// `this` / `it` as a relation endpoint or view rule element refer to the enclosing element.
fn is_self_reference(fqn: &ast::FqnRef) -> bool {
    let segments = fqn.segments();
    segments.len() == 1 && matches!(segments[0].text(), "this" | "it")
}

/// `where kind ...` refers to a relationship kind only directly on a relation expression.
fn where_kind_scope(node: &SyntaxNode, has_participant: bool) -> KindScope {
    if has_participant {
        return KindScope::Node;
    }
    for ancestor in node.ancestors() {
        match ancestor.kind() {
            RELATION_EXPR_WHERE => return KindScope::Relationship,
            FQN_EXPR_WHERE => return KindScope::Node,
            _ => {}
        }
    }
    KindScope::Node
}

/// Target namespace and role of an element reference inside a view rule (include / exclude,
/// style targets, rank), from its enclosing rule and view.
fn view_reference_context(node: &SyntaxNode) -> (RefTarget, &'static str) {
    let mut role = None;
    let mut target = RefTarget::Element;
    for ancestor in node.ancestors() {
        let found = match ancestor.kind() {
            VIEW_RULE_PREDICATE => Some("Predicate element"),
            VIEW_RULE_STYLE | GLOBAL_STYLE => Some("Style target"),
            VIEW_RULE_RANK => Some("Rank target"),
            DEPLOYMENT_VIEW_BODY => {
                target = RefTarget::Deployment;
                None
            }
            _ => None,
        };
        if role.is_none() {
            role = found;
        }
    }
    (target, role.unwrap_or("View rule element"))
}

struct Collector<'p> {
    doc: DocId,
    project: &'p mut Project,
    /// FQN of each enclosing element / `extend` body, innermost last (`None` when the owner
    /// has no usable name).
    model_scope: Vec<Option<String>>,
    /// Same for deployment node / instance / `extend` bodies.
    deployment_scope: Vec<Option<String>>,
}

impl<'p> Collector<'p> {
    fn new(doc: DocId, project: &'p mut Project) -> Self {
        Collector { doc, project, model_scope: Vec::new(), deployment_scope: Vec::new() }
    }

    fn site(&self, token: &SyntaxToken) -> Site {
        Site { doc: self.doc, range: token.text_range() }
    }

    fn node_site(&self, node: &SyntaxNode) -> Site {
        Site { doc: self.doc, range: node.text_range() }
    }

    fn scope(&mut self, namespace: Namespace) -> &mut Vec<Option<String>> {
        match namespace {
            Namespace::Model => &mut self.model_scope,
            Namespace::Deployment => &mut self.deployment_scope,
        }
    }

    fn prefix(&self, namespace: Namespace) -> Option<&str> {
        let scope = match namespace {
            Namespace::Model => &self.model_scope,
            Namespace::Deployment => &self.deployment_scope,
        };
        scope.last().and_then(|s| s.as_deref())
    }

    /// Enter the body of a named declaration: statements inside are scoped to `prefix.name`
    /// (or to the current prefix when the owner has no name).
    fn enter_body(&mut self, namespace: Namespace, owner: Option<String>) {
        let scope = self.scope(namespace);
        let prefix = scope.last().and_then(|s| s.as_deref());
        let fqn = match owner {
            Some(name) => Some(join_fqn(prefix, &name)),
            None => prefix.map(str::to_string),
        };
        scope.push(fqn);
    }

    /// Enter an `extend` body: statements inside are scoped to the extended FQN.
    fn enter_extend(&mut self, namespace: Namespace, target: Option<ast::FqnRef>) {
        self.scope(namespace).push(target.map(|t| t.text()));
    }

    fn kind_ref(&mut self, token: Option<SyntaxToken>, scope: KindScope) {
        if let Some(token) = token {
            self.project.kind_refs.push(KindRef { name: text_of(&token), site: self.site(&token), scope });
        }
    }

    fn fqn_reference(&mut self, fqn: &ast::FqnRef, target: RefTarget, role: &'static str) {
        if let Some(name) = fqn.name_token() {
            self.project.references.push(Reference {
                name: text_of(&name),
                text: fqn.text(),
                site: self.node_site(fqn.syntax()),
                target,
                role,
            });
        }
    }

    fn name_reference(&mut self, token: Option<SyntaxToken>, target: RefTarget, role: &'static str) {
        if let Some(token) = token {
            self.project.references.push(Reference {
                name: text_of(&token),
                text: text_of(&token),
                site: self.site(&token),
                target,
                role,
            });
        }
    }

    /// Text of a relation endpoint: `None` for `this` / `it` (the enclosing element, no
    /// reference is recorded), otherwise the reference text (recorded as a reference).
    fn endpoint(&mut self, fqn: &ast::FqnRef, target: RefTarget, role: &'static str) -> Option<String> {
        if is_self_reference(fqn) {
            return None;
        }
        self.fqn_reference(fqn, target, role);
        Some(fqn.text())
    }

    /// Record a model or deployment relation declared in the current scope.
    fn relation(&mut self, namespace: Namespace, source: Option<ast::FqnRef>, target: Option<ast::FqnRef>) {
        let ref_target = match namespace {
            Namespace::Model => RefTarget::Element,
            Namespace::Deployment => RefTarget::Deployment,
        };
        let enclosing = self.prefix(namespace).map(str::to_string);
        let source = source.and_then(|s| self.endpoint(&s, ref_target, "Relation source"));
        let Some(target) = target else { return };
        let site = self.node_site(target.syntax());
        let target = self.endpoint(&target, ref_target, "Relation target");
        self.project.relations.push(RelationDecl { namespace, enclosing, source, target, site });
    }

    fn collect(&mut self, root: &SyntaxNode) {
        for event in root.preorder() {
            match event {
                WalkEvent::Enter(node) => self.enter(node),
                WalkEvent::Leave(node) => self.leave(&node),
            }
        }
    }

    fn leave(&mut self, node: &SyntaxNode) {
        match node.kind() {
            ELEMENT_BODY | EXTEND_ELEMENT_BODY => {
                self.model_scope.pop();
            }
            DEPLOYMENT_NODE_BODY | DEPLOYED_INSTANCE_BODY | EXTEND_DEPLOYMENT_BODY => {
                self.deployment_scope.pop();
            }
            _ => {}
        }
    }

    fn enter(&mut self, node: SyntaxNode) {
        match node.kind() {
            ELEMENT_BODY => {
                let owner = node.parent().and_then(ast::Element::cast).and_then(|e| e.name());
                self.enter_body(Namespace::Model, owner);
            }
            EXTEND_ELEMENT_BODY => {
                let target = node.parent().and_then(ast::ExtendElement::cast).and_then(|e| e.target());
                self.enter_extend(Namespace::Model, target);
            }
            DEPLOYMENT_NODE_BODY => {
                let owner = node.parent().and_then(ast::DeploymentNode::cast).and_then(|n| n.name());
                self.enter_body(Namespace::Deployment, owner);
            }
            DEPLOYED_INSTANCE_BODY => {
                let owner =
                    node.parent().and_then(ast::DeployedInstance::cast).and_then(|i| instance_name(&i));
                self.enter_body(Namespace::Deployment, owner);
            }
            EXTEND_DEPLOYMENT_BODY => {
                let target = node.parent().and_then(ast::ExtendDeployment::cast).and_then(|e| e.target());
                self.enter_extend(Namespace::Deployment, target);
            }
            SPEC_ELEMENT_KIND => {
                let name = ast::SpecElementKind::cast(node).and_then(|n| n.name_token());
                self.declare_element_kind(name);
            }
            SPEC_DEPLOYMENT_NODE_KIND => {
                let name = ast::SpecDeploymentNodeKind::cast(node).and_then(|n| n.name_token());
                self.declare_deployment_node_kind(name);
            }
            SPEC_RELATIONSHIP_KIND => {
                let name = ast::SpecRelationshipKind::cast(node).and_then(|n| n.name_token());
                self.declare_relationship_kind(name);
            }
            SPEC_TAG => {
                if let Some(tag) = ast::SpecTag::cast(node) {
                    self.declare_tag(tag.name_token());
                    if let Some(color) = tag.color_name_token() {
                        self.project
                            .color_refs
                            .push(Named { name: text_of(&color), site: self.site(&color) });
                    }
                }
            }
            SPEC_COLOR => {
                let name = ast::SpecColor::cast(node).and_then(|n| n.name_token());
                self.declare_color(name);
            }
            GLOBAL_PREDICATE_GROUP => {
                let name = ast::GlobalPredicateGroup::cast(node).and_then(|n| n.name_token());
                self.declare_predicate_group(name);
            }
            GLOBAL_DYNAMIC_PREDICATE_GROUP => {
                let name = ast::GlobalDynamicPredicateGroup::cast(node).and_then(|n| n.name_token());
                self.declare_predicate_group(name);
            }
            GLOBAL_STYLE => {
                let name = ast::GlobalStyle::cast(node).and_then(|n| n.name_token());
                self.declare_style(name);
            }
            GLOBAL_STYLE_GROUP => {
                let name = ast::GlobalStyleGroup::cast(node).and_then(|n| n.name_token());
                self.declare_style(name);
            }
            ELEMENT => {
                if let Some(el) = ast::Element::cast(node) {
                    self.element(&el);
                }
            }
            EXTEND_ELEMENT => {
                if let Some(ext) = ast::ExtendElement::cast(node) {
                    if let Some(target) = ext.target() {
                        self.fqn_reference(&target, RefTarget::Element, "Extend target");
                        let tags = tag_names(ext.body().and_then(|b| b.tags()));
                        self.project.extends.push(ExtendDecl { target: target.text(), tags });
                    }
                }
            }
            RELATION => {
                if let Some(rel) = ast::Relation::cast(node) {
                    self.kind_ref(rel.kind_ref(), KindScope::Relationship);
                    self.relation(Namespace::Model, rel.source(), rel.target());
                }
            }
            EXTEND_RELATION => {
                if let Some(ext) = ast::ExtendRelation::cast(node) {
                    if let Some(source) = ext.source() {
                        self.fqn_reference(&source, RefTarget::Element, "Relation source");
                    }
                    if let Some(target) = ext.target() {
                        self.fqn_reference(&target, RefTarget::Element, "Relation target");
                    }
                    self.kind_ref(ext.kind_ref(), KindScope::Relationship);
                }
            }
            ELEMENT_VIEW => {
                if let Some(view) = ast::ElementView::cast(node) {
                    self.view(view.name_token(), ViewKind::Element);
                    if let Some(of) = view.of() {
                        self.fqn_reference(&of, RefTarget::Element, "View scope");
                    }
                    let extends = view.extends().and_then(|r| r.name_token());
                    self.name_reference(extends, RefTarget::View, "Extended view");
                }
            }
            DYNAMIC_VIEW => {
                let name = ast::DynamicView::cast(node).and_then(|v| v.name_token());
                self.view(name, ViewKind::Dynamic);
            }
            DEPLOYMENT_VIEW => {
                let name = ast::DeploymentView::cast(node).and_then(|v| v.name_token());
                self.view(name, ViewKind::Deployment);
            }
            NAVIGATE_TO_PROPERTY => {
                let name = ast::NavigateToProperty::cast(node)
                    .and_then(|n| n.view_ref())
                    .and_then(|r| r.name_token());
                self.name_reference(name, RefTarget::View, "Navigation target");
            }
            VIEW_RULE_GLOBAL_PREDICATE_REF => {
                let name = ast::ViewRuleGlobalPredicateRef::cast(node).and_then(|n| n.name_token());
                self.name_reference(name, RefTarget::PredicateGroup, "Global predicate group");
            }
            VIEW_RULE_GLOBAL_STYLE => {
                let name = ast::ViewRuleGlobalStyle::cast(node).and_then(|n| n.name_token());
                self.name_reference(name, RefTarget::GlobalStyle, "Global style");
            }
            FQN_REF_EXPRESSION => {
                // Element references in include / exclude, style targets and rank; the
                // `._` / `.*` / `.**` selector is a separate token, so the FQN is the base.
                let (target, role) = view_reference_context(&node);
                if let Some(fqn) = ast::FqnRefExpression::cast(node).and_then(|e| e.fqn_ref()) {
                    if !is_self_reference(&fqn) {
                        self.fqn_reference(&fqn, target, role);
                    }
                }
            }
            STEP => {
                if let Some(step) = ast::Step::cast(node) {
                    if let Some(source) = step.source() {
                        self.fqn_reference(&source, RefTarget::Element, "Step source");
                    }
                    if let Some(target) = step.target() {
                        self.fqn_reference(&target, RefTarget::Element, "Step target");
                    }
                    self.kind_ref(step.kind_ref(), KindScope::Relationship);
                }
            }
            STEP_SERIES => {
                if let Some(series) = ast::StepSeries::cast(node) {
                    if let Some(target) = series.target() {
                        self.fqn_reference(&target, RefTarget::Element, "Step target");
                    }
                    self.kind_ref(series.kind_ref(), KindScope::Relationship);
                }
            }
            OUTGOING_RELATION_EXPR => {
                let kind = ast::OutgoingRelationExpr::cast(node).and_then(|e| e.kind_ref());
                self.kind_ref(kind, KindScope::Relationship);
            }
            ELEMENT_KIND_EXPRESSION => {
                let kind = ast::ElementKindExpression::cast(node).and_then(|e| e.kind_token());
                self.kind_ref(kind, KindScope::Node);
            }
            WHERE_KIND => {
                if let Some(where_kind) = ast::WhereKind::cast(node) {
                    let scope =
                        where_kind_scope(where_kind.syntax(), where_kind.participant_token().is_some());
                    self.kind_ref(where_kind.value_token(), scope);
                }
            }
            TAG_REF => {
                if let Some(name) = ast::TagRef::cast(node).and_then(|t| t.name_token()) {
                    self.project.tag_refs.push(Named { name: text_of(&name), site: self.site(&name) });
                }
            }
            COLOR_PROPERTY => {
                let value = ast::ColorProperty::cast(node).and_then(|c| c.value_token());
                self.color_ref(value);
            }
            ICON_COLOR_PROPERTY => {
                let value = ast::IconColorProperty::cast(node).and_then(|c| c.value_token());
                self.color_ref(value);
            }
            DEPLOYMENT_NODE => {
                if let Some(dn) = ast::DeploymentNode::cast(node) {
                    self.kind_ref(dn.kind_token(), KindScope::DeploymentNode);
                    if let Some(name) = dn.name_token() {
                        let fqn = join_fqn(self.prefix(Namespace::Deployment), name.text());
                        self.project.deployment_nodes.push(Named { name: fqn, site: self.site(&name) });
                    }
                }
            }
            DEPLOYED_INSTANCE => {
                if let Some(instance) = ast::DeployedInstance::cast(node) {
                    if let Some(target) = instance.target() {
                        self.fqn_reference(&target, RefTarget::Element, "Instance target");
                    }
                    let site = instance
                        .name_token()
                        .map(|t| self.site(&t))
                        .or_else(|| instance.target().map(|t| self.node_site(t.syntax())));
                    if let (Some(site), Some(name)) = (site, instance_name(&instance)) {
                        let fqn = join_fqn(self.prefix(Namespace::Deployment), &name);
                        self.project.deployment_instances.push(Named { name: fqn, site });
                    }
                }
            }
            DEPLOYMENT_RELATION => {
                if let Some(rel) = ast::DeploymentRelation::cast(node) {
                    self.kind_ref(rel.kind_ref(), KindScope::Relationship);
                    self.relation(Namespace::Deployment, rel.source(), rel.target());
                }
            }
            EXTEND_DEPLOYMENT => {
                if let Some(target) = ast::ExtendDeployment::cast(node).and_then(|e| e.target()) {
                    self.fqn_reference(&target, RefTarget::Deployment, "Extend target");
                }
            }
            _ => {}
        }
    }

    fn declare_element_kind(&mut self, name: Option<SyntaxToken>) {
        if let Some(token) = name {
            let site = self.site(&token);
            self.project.spec.element_kinds.entry(text_of(&token)).or_default().push(site);
        }
    }

    fn declare_deployment_node_kind(&mut self, name: Option<SyntaxToken>) {
        if let Some(token) = name {
            let site = self.site(&token);
            self.project.spec.deployment_node_kinds.entry(text_of(&token)).or_default().push(site);
        }
    }

    fn declare_relationship_kind(&mut self, name: Option<SyntaxToken>) {
        if let Some(token) = name {
            let site = self.site(&token);
            self.project.spec.relationship_kinds.entry(text_of(&token)).or_default().push(site);
        }
    }

    fn declare_tag(&mut self, name: Option<SyntaxToken>) {
        if let Some(token) = name {
            let site = self.site(&token);
            self.project.spec.tags.entry(text_of(&token)).or_default().push(site);
        }
    }

    fn declare_color(&mut self, name: Option<SyntaxToken>) {
        if let Some(token) = name {
            let site = self.site(&token);
            self.project.spec.colors.entry(text_of(&token)).or_default().push(site);
        }
    }

    fn declare_predicate_group(&mut self, name: Option<SyntaxToken>) {
        if let Some(token) = name {
            let site = self.site(&token);
            self.project.spec.predicate_groups.entry(text_of(&token)).or_default().push(site);
        }
    }

    fn declare_style(&mut self, name: Option<SyntaxToken>) {
        if let Some(token) = name {
            let site = self.site(&token);
            self.project.spec.styles.entry(text_of(&token)).or_default().push(site);
        }
    }

    fn color_ref(&mut self, value: Option<SyntaxToken>) {
        if let Some(token) = value.filter(|t| t.kind() == IDENT) {
            self.project.color_refs.push(Named { name: text_of(&token), site: self.site(&token) });
        }
    }

    fn view(&mut self, name: Option<SyntaxToken>, kind: ViewKind) {
        if let Some(token) = name {
            let site = self.site(&token);
            self.project.views.push(ViewDecl { name: text_of(&token), kind, site });
        }
    }

    fn element(&mut self, el: &ast::Element) {
        let Some(name_token) = el.name_token() else { return };
        self.kind_ref(el.kind_token(), KindScope::Element);
        let (tags, has_title_property) = match el.body() {
            Some(body) => {
                let has_title = body.properties().any(|p| match p {
                    ast::ElementProperty::StringProperty(s) => s.key() == Some("title"),
                    _ => false,
                });
                (tag_names(body.tags()), has_title)
            }
            None => (Vec::new(), false),
        };
        let name = text_of(&name_token);
        let fqn = join_fqn(self.prefix(Namespace::Model), &name);
        let site = self.site(&name_token);
        self.project.elements.push(ElementDecl {
            fqn,
            name,
            kind: el.kind_token().map(|t| text_of(&t)),
            site,
            tags,
            has_title: el.title().is_some() || has_title_property,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, text: &str) -> SourceFile {
        SourceFile { path: PathBuf::from(path), text: text.to_string() }
    }

    fn workspace(files: &[SourceFile]) -> Workspace<'_> {
        Workspace::build(files, &[])
    }

    /// `(enclosing, source, target)` of every relation.
    fn endpoints(project: &Project) -> Vec<(Option<&str>, Option<&str>, Option<&str>)> {
        project
            .relations
            .iter()
            .map(|r| (r.enclosing.as_deref(), r.source.as_deref(), r.target.as_deref()))
            .collect()
    }

    #[test]
    fn partition_uses_deepest_root_and_default_project() {
        let files = [
            file("ws/a/x.c4", ""),
            file("ws/a/nested/y.c4", ""),
            file("ws/b/z.c4", ""),
            file("ws/orphan.c4", ""),
        ];
        let roots = [PathBuf::from("ws/a"), PathBuf::from("ws/a/nested"), PathBuf::from("ws/b")];
        let (projects, assignment) = partition(&files, &roots);
        assert_eq!(projects[0], None);
        let root_of = |i: usize| projects[assignment[i]].clone();
        assert_eq!(root_of(0), Some(PathBuf::from("ws/a")));
        assert_eq!(root_of(1), Some(PathBuf::from("ws/a/nested")));
        assert_eq!(root_of(2), Some(PathBuf::from("ws/b")));
        assert_eq!(root_of(3), None);
    }

    #[test]
    fn partition_does_not_match_path_prefixes_by_string() {
        let files = [file("ws/abc/x.c4", "")];
        let (projects, assignment) = partition(&files, &[PathBuf::from("ws/a")]);
        assert_eq!(projects[assignment[0]], None);
    }

    #[test]
    fn element_fqns_follow_nesting_and_extend() {
        let files = [file(
            "m.c4",
            "model {\n  a = system {\n    b = container {\n      c = component\n    }\n  }\n  extend a.b {\n    d = component\n  }\n}\n",
        )];
        let ws = workspace(&files);
        let fqns: Vec<&str> = ws.projects[0].elements.iter().map(|e| e.fqn.as_str()).collect();
        assert_eq!(fqns, ["a", "a.b", "a.b.c", "a.b.d"]);
        assert!(ws.projects[0].names.elements.contains("d"));
        assert!(ws.projects[0].names.element_fqns.contains("a.b.d"));
    }

    #[test]
    fn scopes_are_left_when_bodies_close() {
        // Siblings after a nested body must not inherit its prefix; a second model block
        // starts at the root again.
        let files = [file(
            "m.c4",
            "model {\n  a = system {\n    b = container\n  }\n  c = system\n  c -> a\n}\nmodel {\n  d = system\n}\n",
        )];
        let ws = workspace(&files);
        let project = &ws.projects[0];
        let fqns: Vec<&str> = project.elements.iter().map(|e| e.fqn.as_str()).collect();
        assert_eq!(fqns, ["a", "a.b", "c", "d"]);
        assert_eq!(endpoints(project), [(None, Some("c"), Some("a"))]);
    }

    #[test]
    fn omitted_relation_source_is_enclosing_element() {
        let files = [file("m.c4", "model {\n  a = system {\n    -> b\n    b -> a\n  }\n}\n")];
        let ws = workspace(&files);
        assert_eq!(
            endpoints(&ws.projects[0]),
            [(Some("a"), None, Some("b")), (Some("a"), Some("b"), Some("a"))]
        );
        assert!(ws.projects[0].relations.iter().all(|r| r.namespace == Namespace::Model));
    }

    #[test]
    fn deployment_fqns_and_instance_names() {
        let files = [file(
            "d.c4",
            "deployment {\n  node prod {\n    node zone {\n      api = instanceOf cloud.api\n      instanceOf cloud.ui\n      -> zone.api\n    }\n  }\n  extend prod.zone {\n    node extra\n  }\n}\n",
        )];
        let ws = workspace(&files);
        let project = &ws.projects[0];
        let nodes: Vec<&str> = project.deployment_nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(nodes, ["prod", "prod.zone", "prod.zone.extra"]);
        let instances: Vec<&str> = project.deployment_instances.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(instances, ["prod.zone.api", "prod.zone.ui"]);
        assert_eq!(endpoints(project), [(Some("prod.zone"), None, Some("zone.api"))]);
        assert_eq!(project.relations[0].namespace, Namespace::Deployment);
        assert!(project.names.deployment.contains("ui"));
        assert!(project.names.deployment_fqns.contains("prod.zone.ui"));
        assert!(project.references.iter().any(|r| r.role == "Instance target" && r.text == "cloud.api"));
    }

    #[test]
    fn resolve_prefers_children_then_ancestors_then_root() {
        let files = [file(
            "m.c4",
            "model {\n  a = system {\n    a = container {\n      c = component\n    }\n    d = container\n  }\n  e = system\n}\ndeployment {\n  node n {\n    api = instanceOf e\n  }\n}\n",
        )];
        let ws = workspace(&files);
        let project = &ws.projects[0];
        let resolve =
            |enclosing: Option<&str>, text: &str| project.resolve(Namespace::Model, enclosing, text);
        assert_eq!(resolve(Some("a"), "a").as_deref(), Some("a.a"));
        assert_eq!(resolve(Some("a.a"), "a").as_deref(), Some("a.a"));
        assert_eq!(resolve(Some("a.a.c"), "d").as_deref(), Some("a.d"));
        assert_eq!(resolve(Some("a.a.c"), "e").as_deref(), Some("e"));
        assert_eq!(resolve(Some("a.a.c"), "a.c").as_deref(), Some("a.a.c"));
        assert_eq!(resolve(None, "a.a.c").as_deref(), Some("a.a.c"));
        assert_eq!(resolve(None, "a"), Some("a".to_string()));
        assert_eq!(resolve(None, "a.c"), None);
        assert_eq!(resolve(Some("a"), "ghost"), None);
        assert_eq!(project.resolve(Namespace::Deployment, Some("n"), "api").as_deref(), Some("n.api"));
        assert_eq!(project.resolve(Namespace::Deployment, None, "n.api").as_deref(), Some("n.api"));
        assert_eq!(project.resolve(Namespace::Deployment, None, "e"), None);
    }

    #[test]
    fn where_kind_scope_depends_on_expression() {
        let files = [file(
            "v.c4",
            "views {\n  view v {\n    include * where kind is system\n    include * -> * where kind is uses\n    include * -> * where source.kind is system\n  }\n}\n",
        )];
        let ws = workspace(&files);
        let scopes: Vec<(&str, KindScope)> =
            ws.projects[0].kind_refs.iter().map(|k| (k.name.as_str(), k.scope)).collect();
        assert_eq!(
            scopes,
            [("system", KindScope::Node), ("uses", KindScope::Relationship), ("system", KindScope::Node)]
        );
    }

    #[test]
    fn steps_record_references_but_no_relations() {
        let files = [file("v.c4", "views {\n  dynamic view d {\n    a -> b -[uses]-> c -> c\n  }\n}\n")];
        let ws = workspace(&files);
        let project = &ws.projects[0];
        assert!(project.relations.is_empty());
        let mut refs: Vec<(&str, &str)> =
            project.references.iter().map(|r| (r.role, r.text.as_str())).collect();
        refs.sort();
        assert_eq!(
            refs,
            [("Step source", "a"), ("Step target", "b"), ("Step target", "c"), ("Step target", "c")]
        );
        assert_eq!(project.kind_refs[0].name, "uses");
    }

    #[test]
    fn view_rule_element_references_are_collected() {
        let files = [file(
            "v.c4",
            "views {\n  view v of a {\n    include b.*, -> c, this\n    style d {\n      color red\n    }\n    rank {\n      e\n    }\n  }\n  deployment view dv {\n    include f\n  }\n}\nglobal {\n  style gs g {\n    color red\n  }\n}\n",
        )];
        let ws = workspace(&files);
        let refs: Vec<(&str, &str, RefTarget)> =
            ws.projects[0].references.iter().map(|r| (r.role, r.text.as_str(), r.target)).collect();
        assert_eq!(
            refs,
            [
                ("View scope", "a", RefTarget::Element),
                ("Predicate element", "b", RefTarget::Element),
                ("Predicate element", "c", RefTarget::Element),
                ("Style target", "d", RefTarget::Element),
                ("Rank target", "e", RefTarget::Element),
                ("Predicate element", "f", RefTarget::Deployment),
                ("Style target", "g", RefTarget::Element),
            ]
        );
    }

    #[test]
    fn this_and_it_refer_to_the_enclosing_element() {
        let files = [file(
            "m.c4",
            "model {\n  a = system {\n    this -> b\n    it .uses b\n    b -> this\n    -> it\n  }\n}\ndeployment {\n  node n {\n    this -> m\n    m -> it\n  }\n}\n",
        )];
        let ws = workspace(&files);
        let project = &ws.projects[0];
        assert_eq!(
            endpoints(project),
            [
                (Some("a"), None, Some("b")),
                (Some("a"), None, Some("b")),
                (Some("a"), Some("b"), None),
                (Some("a"), None, None),
                (Some("n"), None, Some("m")),
                (Some("n"), Some("m"), None),
            ]
        );
        assert!(project.references.iter().all(|r| r.name != "this" && r.name != "it"));
    }

    #[test]
    fn imports_are_detected_per_document() {
        let files = [file("a.c4", "import { x } from 'other'\nmodel { }\n"), file("b.c4", "model { }\n")];
        let ws = workspace(&files);
        assert!(ws.documents[0].has_imports);
        assert!(!ws.documents[1].has_imports);
    }
}
