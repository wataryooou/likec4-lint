//! `unused-tag`: a tag declared but never referenced, as seen from the declaring project
//! (uses in the project's documents and in any importing document count).

use crate::rules::{Cx, Usage};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unused-tag",
    default_level: Level::Warning,
    description: "Tag declared but never used",
    options: &[],
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let usage = Usage::collect(
        ws,
        ws.projects.iter().flat_map(|p| p.tag_refs.iter()).map(|t| (t.site.doc, t.name.as_str())),
    );
    for (id, project) in ws.projects.iter().enumerate() {
        for (name, sites) in &project.spec.tags {
            if let Some(first) = sites.first().filter(|_| !usage.is_used(id, name)) {
                cx.report(
                    *first,
                    format!("Tag '{name}' is declared but never used"),
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
    fn reports_unused_tag() {
        let src = "specification {\n  element system\n  tag used\n  tag unused\n}\nmodel {\n  a = system {\n    #used\n  }\n}\n";
        assert_eq!(messages(&lint_one(src), "unused-tag"), ["Tag 'unused' is declared but never used"]);
    }

    #[test]
    fn use_in_another_project_counts_only_with_imports() {
        let files = [
            ("p/spec.c4", "specification {\n  element system\n  tag t\n}\n"),
            ("q/model.c4", "model {\n  a = system {\n    #t\n  }\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert_eq!(messages(&diags, "unused-tag"), ["Tag 't' is declared but never used"]);
        assert_eq!(messages(&diags, "unknown-tag"), ["Unknown tag '#t'"]);

        let files = [
            ("p/spec.c4", "specification {\n  element system\n  tag t\n}\n"),
            ("q/model.c4", "import { x } from 'p'\nmodel {\n  a = system {\n    #t\n  }\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert!(messages(&diags, "unused-tag").is_empty());
        assert!(messages(&diags, "unknown-tag").is_empty());
    }

    #[test]
    fn tag_used_in_spec_or_predicate_counts() {
        let src = "specification {\n  element system {\n    #a\n  }\n  tag a\n  tag b\n}\nviews {\n  view v {\n    include element.tag = #b\n  }\n}\n";
        assert!(messages(&lint_one(src), "unused-tag").is_empty());
    }
}
