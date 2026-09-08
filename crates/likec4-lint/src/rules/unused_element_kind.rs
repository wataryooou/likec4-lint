//! `unused-element-kind`: an element kind declared but never used anywhere in the workspace.

use std::collections::HashSet;

use crate::model::KindScope;
use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unused-element-kind",
    default_level: Level::Warning,
    description: "Element kind declared but never used",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let used: HashSet<&str> = ws
        .projects
        .iter()
        .flat_map(|p| p.kind_refs.iter())
        .filter(|k| matches!(k.scope, KindScope::Element | KindScope::Node))
        .map(|k| k.name.as_str())
        .collect();
    for project in &ws.projects {
        for (name, sites) in &project.spec.element_kinds {
            if let Some(first) = sites.first().filter(|_| !used.contains(name.as_str())) {
                cx.report(
                    *first,
                    format!("Element kind '{name}' is declared but never used"),
                    Some("remove the declaration if it is not needed".to_string()),
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
    fn reports_unused_kind_once() {
        let src = "specification {\n  element system\n  element unused\n  element unused\n}\nmodel {\n  a = system\n}\n";
        assert_eq!(
            messages(&lint_one(src), "unused-element-kind"),
            ["Element kind 'unused' is declared but never used"]
        );
    }

    #[test]
    fn use_in_predicates_or_other_projects_counts() {
        let files = [
            ("p/spec.c4", "specification {\n  element system\n  element service\n}\n"),
            ("p/views.c4", "views {\n  view v {\n    include * where kind is service\n  }\n}\n"),
            ("q/model.c4", "model {\n  a = system\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert!(messages(&diags, "unused-element-kind").is_empty());
    }
}
