//! `self-relation`: a relation whose source and target resolve to the same element.
//!
//! Both endpoints are resolved from the enclosing element with the model's scoping rules
//! ([`crate::model::Project::resolve`]): a name refers to a child of the enclosing element
//! first, then to a child of an ancestor, then to a root element; `this` / `it` and an
//! omitted source are the enclosing element itself. Endpoints that do not resolve within
//! the project are skipped (`unresolved-reference` reports those).

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "self-relation",
    default_level: Level::Warning,
    description: "Relation from an element to itself",
    options: &[],
};

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
            if source == target {
                cx.report(
                    relation.site,
                    format!("Relation from '{target}' to itself"),
                    Some("remove the relation or change one of its endpoints".to_string()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_self_relations_in_model_and_deployment() {
        let src = "model {\n  a = system {\n    b = container\n    -> a\n    this -> a\n    -> it\n    b -> b\n  }\n  a.b -> a.b\n}\ndeployment {\n  node n\n  n -> n\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "self-relation"),
            [
                "Relation from 'a' to itself",
                "Relation from 'a' to itself",
                "Relation from 'a' to itself",
                "Relation from 'a.b' to itself",
                "Relation from 'a.b' to itself",
                "Relation from 'n' to itself",
            ]
        );
    }

    #[test]
    fn same_name_child_is_not_a_self_relation() {
        // Inside `x`, the name `x` resolves to the child `x.x` first, not to `x` itself.
        let src = "model {\n  x = system {\n    x = container\n    -> x\n    this -> x\n  }\n}\n";
        assert!(messages(&lint_one(src), "self-relation").is_empty());
    }

    #[test]
    fn unresolved_endpoints_are_not_reported() {
        let src = "model {\n  a = system\n  ghost -> ghost\n  a.nope -> a.nope\n}\n";
        assert!(messages(&lint_one(src), "self-relation").is_empty());
    }

    #[test]
    fn resolution_climbs_to_ancestors_and_root() {
        let src = "model {\n  a = system {\n    b = container {\n      c = component {\n        -> b\n        b -> this\n        -> a\n      }\n    }\n  }\n  a -> a.b.c\n}\n";
        assert!(messages(&lint_one(src), "self-relation").is_empty());
        let src = "model {\n  a = system {\n    b = container {\n      c = component {\n        -> c\n        a.b.c -> it\n      }\n    }\n  }\n}\n";
        assert_eq!(
            messages(&lint_one(src), "self-relation"),
            ["Relation from 'a.b.c' to itself", "Relation from 'a.b.c' to itself"]
        );
    }

    #[test]
    fn distinct_endpoints_and_dynamic_view_self_steps_are_fine() {
        let src = "model {\n  a = system {\n    -> b\n  }\n  b = system\n  a.x -> a.y\n}\nviews {\n  dynamic view d {\n    a -> a 'self call'\n    a -> b -> b\n  }\n}\n";
        assert!(messages(&lint_one(src), "self-relation").is_empty());
    }
}
