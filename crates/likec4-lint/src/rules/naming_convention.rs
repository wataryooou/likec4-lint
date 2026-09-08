//! `naming-convention` (off by default): names must match a configured regular expression.
//!
//! Options: `pattern` (regex, default `^[a-z][a-zA-Z0-9]*$`) and `targets` (any of
//! `element`, `view`, `deployment-node`; default `["element", "view"]`).

use regex::Regex;

use crate::model::Site;
use crate::rules::{option_str, option_strings, Cx};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "naming-convention",
    default_level: Level::Off,
    description: "Element/view names must match the configured `pattern` regex",
};

pub const DEFAULT_PATTERN: &str = "^[a-z][a-zA-Z0-9]*$";
const KNOWN_TARGETS: &[&str] = &["element", "view", "deployment-node"];

fn last_segment(fqn: &str) -> &str {
    fqn.rsplit('.').next().unwrap_or(fqn)
}

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let pattern = option_str(cx.options, "pattern").unwrap_or(DEFAULT_PATTERN).to_string();
    let regex = match Regex::new(&pattern) {
        Ok(regex) => regex,
        Err(err) => {
            cx.report_config(
                format!("naming-convention: invalid pattern '{pattern}': {err}"),
                Some("fix the `pattern` option in [lint.rules]".to_string()),
            );
            return;
        }
    };
    let targets = option_strings(cx.options, "targets")
        .unwrap_or_else(|| vec!["element".to_string(), "view".to_string()]);
    for target in targets.iter().filter(|t| !KNOWN_TARGETS.contains(&t.as_str())) {
        cx.report_config(
            format!("naming-convention: unknown target '{target}'"),
            Some(format!("use one of: {}", KNOWN_TARGETS.join(", "))),
        );
    }
    let wants = |t: &str| targets.iter().any(|x| x == t);
    for project in &ws.projects {
        let mut named: Vec<(&str, Site, &str)> = Vec::new();
        if wants("element") {
            named.extend(project.elements.iter().map(|e| (e.name.as_str(), e.site, "Element")));
        }
        if wants("view") {
            named.extend(project.views.iter().map(|v| (v.name.as_str(), v.site, "View")));
        }
        if wants("deployment-node") {
            named.extend(
                project.deployment_nodes.iter().map(|n| (last_segment(&n.name), n.site, "Deployment node")),
            );
        }
        for (name, site, what) in named {
            if !regex.is_match(name) {
                cx.report(
                    site,
                    format!("{what} name '{name}' does not match /{pattern}/"),
                    Some("rename it to match the configured naming convention".to_string()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{enable, lint_files, messages, strings};
    use crate::{Level, OptionValue};

    const SRC: &str = "model {\n  Good = system\n  ok = system {\n    Bad_Name = container\n  }\n}\nviews {\n  view Index {\n    include *\n  }\n}\ndeployment {\n  node Prod\n}\n";

    #[test]
    fn off_by_default() {
        let diags = lint_files(&[("t.c4", SRC)], &[], &crate::LintConfig::default());
        assert!(messages(&diags, "naming-convention").is_empty());
    }

    #[test]
    fn default_pattern_and_targets() {
        let config = enable("naming-convention", Level::Warning, &[]);
        let diags = lint_files(&[("t.c4", SRC)], &[], &config);
        assert_eq!(
            messages(&diags, "naming-convention"),
            [
                "Element name 'Good' does not match /^[a-z][a-zA-Z0-9]*$/",
                "Element name 'Bad_Name' does not match /^[a-z][a-zA-Z0-9]*$/",
                "View name 'Index' does not match /^[a-z][a-zA-Z0-9]*$/",
            ]
        );
    }

    #[test]
    fn custom_pattern_and_targets() {
        let config = enable(
            "naming-convention",
            Level::Error,
            &[
                ("pattern", OptionValue::String("^[A-Z]".into())),
                ("targets", strings(&["deployment-node", "view"])),
            ],
        );
        let diags = lint_files(&[("t.c4", SRC)], &[], &config);
        assert!(messages(&diags, "naming-convention").is_empty());
    }

    #[test]
    fn invalid_pattern_is_reported_as_config_problem() {
        let config =
            enable("naming-convention", Level::Warning, &[("pattern", OptionValue::String("(".into()))]);
        let diags = lint_files(&[("t.c4", SRC)], &[], &config);
        let msgs = messages(&diags, "naming-convention");
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].starts_with("naming-convention: invalid pattern '('"));
    }
}
