//! `require-title` (off by default): every element must have a title.

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "require-title",
    default_level: Level::Off,
    description: "Elements must have a title (inline string or `title` property)",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        for element in project.elements.iter().filter(|e| !e.has_title) {
            let kind = element.kind.as_deref().unwrap_or("kind");
            cx.report(
                element.site,
                format!("Element '{}' has no title", element.fqn),
                Some(format!("add a title: {} = {kind} 'Title'", element.name)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{enable, lint_files, messages};
    use crate::Level;

    const SRC: &str = "model {\n  a = system 'A'\n  b = system {\n    title 'B'\n  }\n  c = system\n  system d {\n    e = container\n  }\n}\n";

    #[test]
    fn reports_elements_without_title_when_enabled() {
        let config = enable("require-title", Level::Warning, &[]);
        let diags = lint_files(&[("t.c4", SRC)], &[], &config);
        assert_eq!(
            messages(&diags, "require-title"),
            ["Element 'c' has no title", "Element 'd' has no title", "Element 'd.e' has no title"]
        );
    }

    #[test]
    fn off_by_default() {
        let diags = lint_files(&[("t.c4", SRC)], &[], &crate::LintConfig::default());
        assert!(messages(&diags, "require-title").is_empty());
    }
}
