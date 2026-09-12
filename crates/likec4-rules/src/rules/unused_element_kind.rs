//! `unused-element-kind`: an element kind declared but never used, as seen from the declaring
//! project (uses in the project's documents and in any importing document count).

use crate::model::KindScope;
use crate::rules::{Cx, Usage};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unused-element-kind",
    default_level: Level::Warning,
    description: "Element kind declared but never used",
    options: &[],
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let usage = Usage::collect(
        ws,
        ws.projects
            .iter()
            .flat_map(|p| p.kind_refs.iter())
            .filter(|k| matches!(k.scope, KindScope::Element | KindScope::Node))
            .map(|k| (k.site.doc, k.name.as_str())),
    );
    for (id, project) in ws.projects.iter().enumerate() {
        for (name, sites) in &project.spec.element_kinds {
            if let Some(first) = sites.first().filter(|_| !usage.is_used(id, name)) {
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
    fn use_in_predicates_counts() {
        let files = [
            ("p/spec.c4", "specification {\n  element service\n}\n"),
            ("p/views.c4", "views {\n  view v {\n    include * where kind is service\n  }\n}\n"),
        ];
        let diags = lint_files(&files, &["p"], &LintConfig::default());
        assert!(messages(&diags, "unused-element-kind").is_empty());
    }

    /// Visibility mirrors `unknown-element-kind`: a use in another project counts only when
    /// that document imports, so `unused` and `unknown` are never both silent or both loud.
    #[test]
    fn use_in_another_project_counts_only_with_imports() {
        let files = [
            ("p/spec.c4", "specification {\n  element system\n}\n"),
            ("q/model.c4", "model {\n  a = system\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert_eq!(
            messages(&diags, "unused-element-kind"),
            ["Element kind 'system' is declared but never used"]
        );
        assert_eq!(messages(&diags, "unknown-element-kind"), ["Unknown element kind 'system'"]);

        let files = [
            ("p/spec.c4", "specification {\n  element system\n}\n"),
            ("q/model.c4", "import { x } from 'p'\nmodel {\n  a = system\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert!(messages(&diags, "unused-element-kind").is_empty());
        assert!(messages(&diags, "unknown-element-kind").is_empty());
    }
}
