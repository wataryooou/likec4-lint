//! `unknown-tag`: `#tag` without a `tag` declaration in the project.

use crate::rules::{declared, Cx};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unknown-tag",
    default_level: Level::Error,
    description: "Tag not declared in any specification of the project",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        for tag in &project.tag_refs {
            let name = tag.name.as_str();
            if !declared(ws, tag.site.doc, |p| p.spec.tags.contains_key(name)) {
                cx.report(
                    tag.site,
                    format!("Unknown tag '#{name}'"),
                    Some(format!("declare it in a specification block: tag {name}")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_undeclared_tags_everywhere() {
        let src = "specification {\n  element system\n  tag known\n  element service {\n    #legacy\n  }\n}\nmodel {\n  a = system {\n    #known #internal\n  }\n  a -> a #api\n}\nviews {\n  view v {\n    include * where tag is #core\n  }\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "unknown-tag"),
            ["Unknown tag '#legacy'", "Unknown tag '#internal'", "Unknown tag '#api'", "Unknown tag '#core'"]
        );
    }

    #[test]
    fn declared_tag_is_fine() {
        let src = "specification {\n  element system\n  tag known\n}\nmodel {\n  a = system {\n    #known\n  }\n}\n";
        assert!(messages(&lint_one(src), "unknown-tag").is_empty());
    }
}
