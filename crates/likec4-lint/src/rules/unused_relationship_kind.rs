//! `unused-relationship-kind`: a relationship kind declared but never used in the workspace.

use std::collections::HashSet;

use crate::model::KindScope;
use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unused-relationship-kind",
    default_level: Level::Warning,
    description: "Relationship kind declared but never used",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let used: HashSet<&str> = ws
        .projects
        .iter()
        .flat_map(|p| p.kind_refs.iter())
        .filter(|k| k.scope == KindScope::Relationship)
        .map(|k| k.name.as_str())
        .collect();
    for project in &ws.projects {
        for (name, sites) in &project.spec.relationship_kinds {
            if let Some(first) = sites.first().filter(|_| !used.contains(name.as_str())) {
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
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_unused_relationship_kind() {
        let src = "specification {\n  element system\n  relationship uses\n  relationship unused\n}\nmodel {\n  a = system\n  a -[uses]-> a\n}\n";
        assert_eq!(
            messages(&lint_one(src), "unused-relationship-kind"),
            ["Relationship kind 'unused' is declared but never used"]
        );
    }

    #[test]
    fn use_in_where_clause_counts() {
        let src = "specification {\n  relationship uses\n}\nviews {\n  view v {\n    include * -> * where kind is uses\n  }\n}\n";
        assert!(messages(&lint_one(src), "unused-relationship-kind").is_empty());
    }
}
