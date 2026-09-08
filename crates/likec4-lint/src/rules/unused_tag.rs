//! `unused-tag`: a tag declared but never referenced anywhere in the workspace.

use std::collections::HashSet;

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo =
    RuleInfo { id: "unused-tag", default_level: Level::Warning, description: "Tag declared but never used" };

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let used: HashSet<&str> =
        ws.projects.iter().flat_map(|p| p.tag_refs.iter()).map(|t| t.name.as_str()).collect();
    for project in &ws.projects {
        for (name, sites) in &project.spec.tags {
            if let Some(first) = sites.first().filter(|_| !used.contains(name.as_str())) {
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
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_unused_tag() {
        let src = "specification {\n  element system\n  tag used\n  tag unused\n}\nmodel {\n  a = system {\n    #used\n  }\n}\n";
        assert_eq!(messages(&lint_one(src), "unused-tag"), ["Tag 'unused' is declared but never used"]);
    }

    #[test]
    fn tag_used_in_spec_or_predicate_counts() {
        let src = "specification {\n  element system {\n    #a\n  }\n  tag a\n  tag b\n}\nviews {\n  view v {\n    include element.tag = #b\n  }\n}\n";
        assert!(messages(&lint_one(src), "unused-tag").is_empty());
    }
}
