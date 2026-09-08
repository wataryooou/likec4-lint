//! `reserved-name`: element, view, deployment node or instance named with a soft reserved word.

use likec4_syntax::SOFT_RESERVED_WORDS;

use crate::model::Site;
use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "reserved-name",
    default_level: Level::Warning,
    description: "Element or view named with a reserved word (element, model, group, node, ...)",
};

fn last_segment(fqn: &str) -> &str {
    fqn.rsplit('.').next().unwrap_or(fqn)
}

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        let named: Vec<(&str, Site, &str)> = project
            .elements
            .iter()
            .map(|e| (e.name.as_str(), e.site, "element"))
            .chain(project.views.iter().map(|v| (v.name.as_str(), v.site, "view")))
            .chain(
                project.deployment_nodes.iter().map(|n| (last_segment(&n.name), n.site, "deployment node")),
            )
            .chain(project.deployment_instances.iter().map(|n| (last_segment(&n.name), n.site, "instance")))
            .collect();
        for (name, site, what) in named {
            if SOFT_RESERVED_WORDS.contains(&name) {
                cx.report(
                    site,
                    format!("Reserved name '{name}'"),
                    Some(format!("'{name}' is a reserved word; rename the {what}")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_reserved_names_not_kinds() {
        let src = "model {\n  model = system\n  a = system {\n    element = container\n  }\n}\nviews {\n  view group {\n    include *\n  }\n}\ndeployment {\n  node node {\n    instance = instanceOf a\n  }\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "reserved-name"),
            [
                "Reserved name 'model'",
                "Reserved name 'element'",
                "Reserved name 'group'",
                "Reserved name 'node'",
                "Reserved name 'instance'",
            ]
        );
    }

    #[test]
    fn ordinary_names_are_fine() {
        let src = "model {\n  a = system\n}\nviews {\n  view index {\n    include *\n  }\n}\ndeployment {\n  node prod\n}\n";
        assert!(messages(&lint_one(src), "reserved-name").is_empty());
    }
}
