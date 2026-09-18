//! `unused-custom-color`: a custom colour declared but never used, as seen from the declaring
//! project (uses in the project's documents and in any importing document count).

use crate::rules::{Cx, Usage};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unused-custom-color",
    default_level: Level::Warning,
    description: "Custom colour declared but never used",
    options: &[],
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let usage = Usage::collect(
        ws,
        ws.projects.iter().flat_map(|p| p.color_refs.iter()).map(|c| (c.site.doc, c.name.as_str())),
    );
    for (id, project) in ws.projects.iter().enumerate() {
        for (name, sites) in &project.spec.colors {
            if let Some(first) = sites.first().filter(|_| !usage.is_used(id, name)) {
                cx.report(
                    *first,
                    format!("Color '{name}' is declared but never used"),
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
    fn reports_unused_color() {
        let src = "specification {\n  element system\n  color brand #ff0000\n  color unused #00ff00\n}\nmodel {\n  a = system {\n    style {\n      color brand\n    }\n  }\n}\n";
        assert_eq!(
            messages(&lint_one(src), "unused-custom-color"),
            ["Color 'unused' is declared but never used"]
        );
    }

    #[test]
    fn use_as_icon_color_counts() {
        let src = "specification {\n  element system\n  color brand #ff0000\n}\nmodel {\n  a = system {\n    style {\n      iconColor brand\n    }\n  }\n}\n";
        assert!(messages(&lint_one(src), "unused-custom-color").is_empty());
    }

    /// Visibility mirrors `unknown-custom-color`: a use in another project counts only when
    /// that document imports, so `unused` and `unknown` are never both silent or both loud.
    #[test]
    fn use_in_another_project_counts_only_with_imports() {
        let files = [
            ("p/spec.c4", "specification {\n  element system\n  color brand #ff0000\n}\n"),
            ("q/model.c4", "model {\n  a = system {\n    style {\n      color brand\n    }\n  }\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert_eq!(messages(&diags, "unused-custom-color"), ["Color 'brand' is declared but never used"]);
        assert_eq!(messages(&diags, "unknown-custom-color"), ["Unknown color 'brand'"]);

        let files = [
            ("p/spec.c4", "specification {\n  element system\n  color brand #ff0000\n}\n"),
            (
                "q/model.c4",
                "import { x } from 'p'\nmodel {\n  a = system {\n    style {\n      color brand\n    }\n  }\n}\n",
            ),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert!(messages(&diags, "unused-custom-color").is_empty());
        assert!(messages(&diags, "unknown-custom-color").is_empty());
    }
}
