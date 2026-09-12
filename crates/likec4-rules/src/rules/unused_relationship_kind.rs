//! `unused-relationship-kind`: a relationship kind declared but never used, as seen from the
//! declaring project (uses in the project's documents and in any importing document count).

use crate::model::KindScope;
use crate::rules::{Cx, Usage};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unused-relationship-kind",
    default_level: Level::Warning,
    description: "Relationship kind declared but never used",
    options: &[],
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let usage = Usage::collect(
        ws,
        ws.projects
            .iter()
            .flat_map(|p| p.kind_refs.iter())
            .filter(|k| k.scope == KindScope::Relationship)
            .map(|k| (k.site.doc, k.name.as_str())),
    );
    for (id, project) in ws.projects.iter().enumerate() {
        for (name, sites) in &project.spec.relationship_kinds {
            if let Some(first) = sites.first().filter(|_| !usage.is_used(id, name)) {
                cx.report(
                    *first,
                    format!("Relationship kind '{name}' is declared but never used"),
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
    fn reports_unused_relationship_kind() {
        let src = "specification {\n  element system\n  relationship uses\n  relationship unused\n}\nmodel {\n  a = system\n  a -[uses]-> a\n}\n";
        assert_eq!(
            messages(&lint_one(src), "unused-relationship-kind"),
            ["Relationship kind 'unused' is declared but never used"]
        );
    }

    #[test]
    fn use_in_another_project_counts_only_with_imports() {
        let files = [
            ("p/spec.c4", "specification {\n  element system\n  relationship uses\n}\n"),
            ("q/model.c4", "model {\n  a = system\n  a -[uses]-> a\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert_eq!(
            messages(&diags, "unused-relationship-kind"),
            ["Relationship kind 'uses' is declared but never used"]
        );
        assert_eq!(messages(&diags, "unknown-relationship-kind"), ["Unknown relationship kind 'uses'"]);

        let files = [
            ("p/spec.c4", "specification {\n  element system\n  relationship uses\n}\n"),
            ("q/model.c4", "import { x } from 'p'\nmodel {\n  a = system\n  a -[uses]-> a\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert!(messages(&diags, "unused-relationship-kind").is_empty());
        assert!(messages(&diags, "unknown-relationship-kind").is_empty());
    }

    #[test]
    fn use_in_where_clause_counts() {
        let src = "specification {\n  relationship uses\n}\nviews {\n  view v {\n    include * -> * where kind is uses\n  }\n}\n";
        assert!(messages(&lint_one(src), "unused-relationship-kind").is_empty());
    }
}
