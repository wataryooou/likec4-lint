//! `reserved-word`: name the official validator rejects (`this`, `it`, `self`, `super`).

use std::collections::BTreeMap;

use crate::model::Site;
use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "reserved-word",
    default_level: Level::Error,
    description: "Name the official validator rejects (this, it, self, super)",
    options: &[],
};

const RESERVED_WORDS: [&str; 4] = ["this", "it", "self", "super"];

fn last_segment(fqn: &str) -> &str {
    fqn.rsplit('.').next().unwrap_or(fqn)
}

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        let spec = &project.spec;
        let kind_namespaces: [(&BTreeMap<String, Vec<Site>>, &str); 3] = [
            (&spec.element_kinds, "element kind"),
            (&spec.deployment_node_kinds, "deployment node kind"),
            (&spec.relationship_kinds, "relationship kind"),
        ];
        for (map, what) in kind_namespaces {
            for (name, sites) in map {
                if !RESERVED_WORDS.contains(&name.as_str()) {
                    continue;
                }
                for site in sites {
                    cx.report(
                        *site,
                        format!("Reserved word '{name}'"),
                        Some(format!("'{name}' is rejected by the official validator; rename the {what}")),
                    );
                }
            }
        }

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
            if RESERVED_WORDS.contains(&name) {
                cx.report(
                    site,
                    format!("Reserved word '{name}'"),
                    Some(format!("'{name}' is rejected by the official validator; rename the {what}")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_names_the_official_validator_rejects() {
        let src = "specification {\n  element system\n  element this\n  deploymentNode node\n  deploymentNode it\n  tag this\n  relationship self\n  color super #ff0000\n}\nmodel {\n  this = system\n  a = system {\n    it = system\n    self = system {\n      super = system\n    }\n  }\n}\nviews {\n  view this {\n    include *\n  }\n  view it of a {\n    include *\n  }\n}\ndeployment {\n  node this\n  node super {\n    it = instanceOf a\n    self = instanceOf a.it\n  }\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "reserved-word"),
            [
                "Reserved word 'this'",
                "Reserved word 'it'",
                "Reserved word 'self'",
                "Reserved word 'this'",
                "Reserved word 'it'",
                "Reserved word 'self'",
                "Reserved word 'super'",
                "Reserved word 'this'",
                "Reserved word 'it'",
                "Reserved word 'this'",
                "Reserved word 'super'",
                "Reserved word 'it'",
                "Reserved word 'self'",
            ]
        );
    }

    #[test]
    fn ordinary_names_are_fine() {
        let src = "specification {\n  element system\n}\nmodel {\n  a = system {\n    b = system\n  }\n}\nviews {\n  view index {\n    include *\n  }\n}\ndeployment {\n  node prod {\n    web = instanceOf a\n  }\n}\n";
        assert!(messages(&lint_one(src), "reserved-word").is_empty());
    }
}
