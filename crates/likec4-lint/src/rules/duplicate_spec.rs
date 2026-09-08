//! `duplicate-spec`: the same kind / tag / colour / global declared twice in a project.

use std::collections::BTreeMap;

use crate::model::Site;
use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "duplicate-spec",
    default_level: Level::Error,
    description: "Same kind, tag, colour, predicate group or global style declared twice",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        let spec = &project.spec;
        let namespaces: [(&BTreeMap<String, Vec<Site>>, &str); 7] = [
            (&spec.element_kinds, "element kind"),
            (&spec.deployment_node_kinds, "deploymentNode kind"),
            (&spec.relationship_kinds, "relationship kind"),
            (&spec.tags, "tag"),
            (&spec.colors, "color"),
            (&spec.predicate_groups, "predicate group"),
            (&spec.styles, "global style"),
        ];
        for (map, what) in namespaces {
            for (name, sites) in map {
                let Some((first, rest)) = sites.split_first() else { continue };
                for site in rest {
                    cx.report(
                        *site,
                        format!("Duplicate {what} '{name}'"),
                        Some(format!("first declared at {}", ws.location(*first))),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_files, lint_one, messages};
    use crate::LintConfig;

    #[test]
    fn reports_duplicates_in_every_namespace() {
        let src = "specification {\n  element system\n  element system\n  deploymentNode node\n  deploymentNode node\n  relationship uses\n  relationship uses\n  tag t\n  tag t\n  color c #fff\n  color c #000\n}\nglobal {\n  predicateGroup pg {\n    include *\n  }\n  dynamicPredicateGroup pg {\n    include *\n  }\n  style gs * {\n    color red\n  }\n  styleGroup gs {\n    style * {\n      color red\n    }\n  }\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "duplicate-spec"),
            [
                "Duplicate element kind 'system'",
                "Duplicate deploymentNode kind 'node'",
                "Duplicate relationship kind 'uses'",
                "Duplicate tag 't'",
                "Duplicate color 'c'",
                "Duplicate predicate group 'pg'",
                "Duplicate global style 'gs'",
            ]
        );
    }

    #[test]
    fn same_name_in_different_projects_is_fine() {
        let diags = lint_files(
            &[
                ("p/spec.c4", "specification {\n  element system\n}\n"),
                ("q/spec.c4", "specification {\n  element system\n}\n"),
            ],
            &["p", "q"],
            &LintConfig::default(),
        );
        assert!(messages(&diags, "duplicate-spec").is_empty());
    }
}
