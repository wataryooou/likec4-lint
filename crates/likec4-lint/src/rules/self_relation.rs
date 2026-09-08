//! `self-relation`: a relation whose source and target are the same reference.

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "self-relation",
    default_level: Level::Warning,
    description: "Relation from an element to itself",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        for relation in &project.relations {
            if relation.source.as_deref() == Some(relation.target.as_str()) {
                cx.report(
                    relation.site,
                    format!("Relation from '{}' to itself", relation.target),
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
        let src = "model {\n  a = system {\n    -> a\n    this -> a\n    -> it\n  }\n  a.b -> a.b\n}\ndeployment {\n  node n\n  n -> n\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "self-relation"),
            [
                "Relation from 'a' to itself",
                "Relation from 'a' to itself",
                "Relation from 'a' to itself",
                "Relation from 'a.b' to itself",
                "Relation from 'n' to itself",
            ]
        );
    }

    #[test]
    fn distinct_endpoints_and_dynamic_view_self_steps_are_fine() {
        let src = "model {\n  a = system {\n    -> b\n  }\n  b = system\n  a.x -> a.y\n}\nviews {\n  dynamic view d {\n    a -> a 'self call'\n    a -> b -> b\n  }\n}\n";
        assert!(messages(&lint_one(src), "self-relation").is_empty());
    }
}
