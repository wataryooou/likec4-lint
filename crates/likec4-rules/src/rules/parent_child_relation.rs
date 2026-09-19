//! `parent-child-relation`: a relation between an element and one of its own ancestors or
//! descendants (`a -> a.b`, `a.b -> a`). The official validator rejects these as "Invalid
//! parent-child relationship". `a -> a` (the same element on both sides) is `self-relation`'s
//! concern, not this rule's.
//!
//! Model and deployment relations, including those declared inside an `extend` body (which
//! scopes exactly like an element body), are read from [`crate::model::Project::relations`]
//! and resolved the same way as `self-relation` does
//! ([`crate::model::Project::resolve`]).
//!
//! Dynamic view steps are not relations in the model at all (self-steps are a legitimate
//! sequence-diagram construct, see `RelationDecl`'s doc comment, and the official validator
//! agrees: measured against `likec4 validate`, `a -> a` inside a `dynamic view` is accepted).
//! But the official validator does reject a parent-child pair written as the *first* arrow of
//! a step chain (`a -> a.b` in `a -> a.b -> a.b.c`); a later continuation (`a.b -> a.b.c`
//! above) is left unchecked, also measured against `likec4 validate`. This rule matches that
//! by walking `STEP` syntax nodes directly, the same way `invalid-opacity` walks its own
//! target node kind.

use likec4_syntax::ast::{self, AstNode};
use likec4_syntax::SyntaxKind::STEP;

use crate::model::Namespace;
use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "parent-child-relation",
    default_level: Level::Error,
    description: "Relation between an element and one of its ancestors or descendants",
    options: &[],
};

const HELP: &str = "the official validator rejects this as 'Invalid parent-child relationship'; \
                     relate sibling elements or remove the relation";

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        for relation in &project.relations {
            let resolve = |endpoint: &Option<String>| match endpoint {
                None => relation.enclosing.clone(),
                Some(text) => project.resolve(relation.namespace, relation.enclosing.as_deref(), text),
            };
            let (Some(source), Some(target)) = (resolve(&relation.source), resolve(&relation.target)) else {
                continue;
            };
            if let Some(message) = message(&source, &target) {
                cx.report(relation.site, message, Some(HELP.to_string()));
            }
        }
    }
    for (doc, document) in ws.documents.iter().enumerate() {
        let project = ws.project_of(doc);
        for node in document.syntax().descendants().filter(|n| n.kind() == STEP) {
            let Some(step) = ast::Step::cast(node) else { continue };
            let (Some(source_ref), Some(target_ref)) = (step.source(), step.target()) else { continue };
            let Some(source) = project.resolve(Namespace::Model, None, &source_ref.text()) else { continue };
            let Some(target) = project.resolve(Namespace::Model, None, &target_ref.text()) else { continue };
            if let Some(message) = message(&source, &target) {
                cx.report_range(doc, target_ref.syntax().text_range(), message, Some(HELP.to_string()));
            }
        }
    }
}

/// `source`/`target` as an ancestor/descendant pair, formatted for the diagnostic; `None` when
/// they are unrelated, including when they are the same element (`self-relation`'s concern).
fn message(source: &str, target: &str) -> Option<String> {
    if is_ancestor(source, target) {
        Some(format!("Relation from '{source}' to its descendant '{target}'"))
    } else if is_ancestor(target, source) {
        Some(format!("Relation from '{source}' to its ancestor '{target}'"))
    } else {
        None
    }
}

/// True when `ancestor` is a proper `.`-separated prefix of `fqn` (`a` is an ancestor of
/// `a.b`, but not of `ab`; `a` is not an ancestor of itself).
fn is_ancestor(ancestor: &str, fqn: &str) -> bool {
    fqn.strip_prefix(ancestor).is_some_and(|rest| rest.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    /// The scenario measured against `likec4 validate` (LikeC4 1.59.3): every ancestor/
    /// descendant pair, in both directions, in `model` and `deployment`, reported in order.
    #[test]
    fn matches_the_official_validator_on_the_measured_file() {
        let src = "specification {\n  element system\n  element container\n  deploymentNode node\n}\nmodel {\n  a = system {\n    b = container {\n      c = container\n    }\n    -> b\n    b -> this\n    c -> a\n    b -> c\n  }\n  a -> a.b.c\n  a.b.c -> a\n  x = system\n  a -> x\n}\ndeployment {\n  node n1 {\n    node n2 {\n      i = instanceOf a\n    }\n  }\n  n1 -> n1.n2\n  n1.n2 -> n1\n  n1.n2.i -> n1\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "parent-child-relation"),
            [
                "Relation from 'a' to its descendant 'a.b'",
                "Relation from 'a.b' to its ancestor 'a'",
                "Relation from 'a.b.c' to its ancestor 'a'",
                "Relation from 'a.b' to its descendant 'a.b.c'",
                "Relation from 'a' to its descendant 'a.b.c'",
                "Relation from 'a.b.c' to its ancestor 'a'",
                "Relation from 'n1' to its descendant 'n1.n2'",
                "Relation from 'n1.n2' to its ancestor 'n1'",
                "Relation from 'n1.n2.i' to its ancestor 'n1'",
            ]
        );
        // `a -> x`, a sibling relation, and the self-relation inside it (`self-relation`'s
        // concern) must not also be reported here.
        assert!(!messages(&diags, "parent-child-relation").iter().any(|m| m.contains("'x'")));
        assert!(messages(&diags, "self-relation").is_empty());
    }

    #[test]
    fn sibling_and_self_relations_are_not_reported() {
        let src = "model {\n  a = system {\n    b = system\n  }\n  x = system\n  a.b -> x\n  a -> a\n}\n";
        assert!(messages(&lint_one(src), "parent-child-relation").is_empty());
    }

    #[test]
    fn unresolved_endpoints_are_skipped() {
        let src = "model {\n  a = system\n  ghost -> ghost.b\n  a.nope -> a.nope.b\n}\n";
        assert!(messages(&lint_one(src), "parent-child-relation").is_empty());
    }

    #[test]
    fn explicit_fqn_endpoints_on_both_sides_are_reported() {
        // Measured against `likec4 validate`: both directions of a fully-qualified
        // ancestor/descendant pair are rejected, same as a bare-name endpoint.
        let src = "model {\n  a = system {\n    b = system {\n      c = system\n    }\n  }\n  a.b -> a.b.c\n  a.b.c -> a.b\n}\n";
        assert_eq!(
            messages(&lint_one(src), "parent-child-relation"),
            ["Relation from 'a.b' to its descendant 'a.b.c'", "Relation from 'a.b.c' to its ancestor 'a.b'"]
        );
    }

    #[test]
    fn extend_body_relations_are_reported() {
        // Measured against `likec4 validate`: `extend a { -> a.b }` scopes exactly like an
        // element body, so it is rejected the same way `a = system { -> a.b }` would be.
        let src = "model {\n  a = system {\n    b = system\n  }\n  x = system\n  extend a {\n    -> a.b\n    -> x\n  }\n}\n";
        assert_eq!(
            messages(&lint_one(src), "parent-child-relation"),
            ["Relation from 'a' to its descendant 'a.b'"]
        );
    }

    #[test]
    fn dynamic_view_self_steps_are_not_reported_but_parent_child_steps_are() {
        // Measured against `likec4 validate`: `a -> a` inside a dynamic view is valid, but
        // `a -> a.b` and `a.b -> a` there are rejected, same as outside a view.
        let src = "model {\n  a = system {\n    b = system\n  }\n}\nviews {\n  dynamic view d {\n    a -> a 'self call'\n    a -> a.b\n    a.b -> a\n  }\n}\n";
        assert_eq!(
            messages(&lint_one(src), "parent-child-relation"),
            ["Relation from 'a' to its descendant 'a.b'", "Relation from 'a.b' to its ancestor 'a'"]
        );
        assert!(messages(&lint_one(src), "self-relation").is_empty());
    }

    #[test]
    fn only_the_first_arrow_of_a_dynamic_view_step_chain_is_checked() {
        // Measured against `likec4 validate`: only the leading `Step` node of a chain (the
        // first arrow) is checked; a parent-child pair introduced only by a later
        // continuation arrow is not, in either direction.
        let src = "model {\n  a = system {\n    b = system\n  }\n  x = system\n}\nviews {\n  dynamic view d {\n    a -> x -> a.b\n    a.b -> a -> x\n  }\n}\n";
        assert_eq!(
            messages(&lint_one(src), "parent-child-relation"),
            ["Relation from 'a.b' to its ancestor 'a'"]
        );
    }
}
