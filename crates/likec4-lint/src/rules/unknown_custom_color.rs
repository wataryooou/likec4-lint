//! `unknown-custom-color`: `color x` that is neither a theme colour nor a declared custom colour.

use crate::rules::{declared, Cx};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unknown-custom-color",
    default_level: Level::Error,
    description: "Colour that is neither a theme colour nor a declared custom colour",
};

/// Theme colours of the grammar (`ThemeColor`).
pub const THEME_COLORS: &[&str] =
    &["primary", "secondary", "muted", "slate", "blue", "indigo", "sky", "red", "gray", "green", "amber"];

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        for color in &project.color_refs {
            let name = color.name.as_str();
            if THEME_COLORS.contains(&name) {
                continue;
            }
            if !declared(ws, color.site.doc, |p| p.spec.colors.contains_key(name)) {
                cx.report(
                    color.site,
                    format!("Unknown color '{name}'"),
                    Some(format!(
                        "use a theme color ({}) or declare a custom color in a specification block: color {name} #rrggbb",
                        THEME_COLORS.join(", ")
                    )),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_unknown_colors() {
        let src = "specification {\n  element system {\n    style {\n      color brand\n    }\n  }\n  color brand #ff0000\n  tag t {\n    color pink\n  }\n  relationship uses {\n    color teal\n  }\n}\nmodel {\n  a = system {\n    style {\n      color red\n      iconColor mint\n    }\n  }\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "unknown-custom-color"),
            ["Unknown color 'pink'", "Unknown color 'teal'", "Unknown color 'mint'"]
        );
    }

    #[test]
    fn theme_and_declared_colors_are_fine() {
        let src = "specification {\n  color brand #ff0000\n}\nviews {\n  view v {\n    include *\n    style * {\n      color brand\n    }\n    style * {\n      color amber\n    }\n  }\n}\n";
        assert!(messages(&lint_one(src), "unknown-custom-color").is_empty());
    }
}
