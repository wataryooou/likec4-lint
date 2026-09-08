//! `unknown-element-kind`: an element kind that no `specification` of the project declares.

use crate::model::KindScope;
use crate::rules::{declared, Cx};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unknown-element-kind",
    default_level: Level::Error,
    description: "Element kind not declared in any specification of the project",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        for kind in &project.kind_refs {
            let name = kind.name.as_str();
            let known = match kind.scope {
                KindScope::Element => {
                    declared(ws, kind.site.doc, |p| p.spec.element_kinds.contains_key(name))
                }
                KindScope::Node => declared(ws, kind.site.doc, |p| {
                    p.spec.element_kinds.contains_key(name) || p.spec.deployment_node_kinds.contains_key(name)
                }),
                KindScope::DeploymentNode | KindScope::Relationship => continue,
            };
            if !known {
                cx.report(
                    kind.site,
                    format!("Unknown element kind '{name}'"),
                    Some(format!("declare it in a specification block: element {name}")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_files, lint_one, messages};
    use crate::LintConfig;

    #[test]
    fn reports_undeclared_kind() {
        let diags = lint_one("specification {\n  element system\n}\nmodel {\n  a = service\n  system b\n}\n");
        assert_eq!(messages(&diags, "unknown-element-kind"), ["Unknown element kind 'service'"]);
    }

    #[test]
    fn reports_undeclared_kind_in_predicates() {
        let src = "specification {\n  element system\n}\nviews {\n  view v {\n    include element.kind = service\n    include * where kind is gateway\n  }\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "unknown-element-kind"),
            ["Unknown element kind 'service'", "Unknown element kind 'gateway'"]
        );
    }

    #[test]
    fn kind_declared_in_another_file_of_the_project_is_fine() {
        let diags = lint_files(
            &[
                ("p/spec.c4", "specification {\n  element system\n}\n"),
                ("p/model.c4", "model {\n  a = system\n}\n"),
            ],
            &["p"],
            &LintConfig::default(),
        );
        assert!(messages(&diags, "unknown-element-kind").is_empty());
    }

    #[test]
    fn other_project_spec_does_not_count_without_imports() {
        let diags = lint_files(
            &[
                ("p/spec.c4", "specification {\n  element system\n}\n"),
                ("q/model.c4", "model {\n  a = system\n}\n"),
            ],
            &["p", "q"],
            &LintConfig::default(),
        );
        assert_eq!(messages(&diags, "unknown-element-kind"), ["Unknown element kind 'system'"]);
    }

    #[test]
    fn imports_widen_to_the_workspace() {
        let diags = lint_files(
            &[
                ("p/spec.c4", "specification {\n  element system\n}\n"),
                ("q/model.c4", "import { x } from 'p'\nmodel {\n  a = system\n}\n"),
            ],
            &["p", "q"],
            &LintConfig::default(),
        );
        assert!(messages(&diags, "unknown-element-kind").is_empty());
    }
}
