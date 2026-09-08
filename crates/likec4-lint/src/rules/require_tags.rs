//! `require-tags` (off by default): elements of the configured kinds must carry the
//! configured tags. Tags added through `extend` count.
//!
//! Options: `kinds` (list; empty or missing means every kind) and `tags` (list).

use std::collections::{BTreeSet, HashMap};

use crate::rules::{option_strings, Cx};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "require-tags",
    default_level: Level::Off,
    description: "Elements of the configured `kinds` must carry the configured `tags`",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let kinds = option_strings(cx.options, "kinds").unwrap_or_default();
    let required = option_strings(cx.options, "tags").unwrap_or_default();
    if required.is_empty() {
        cx.report_config(
            "require-tags: no `tags` configured".to_string(),
            Some(
                "set the `tags` option, e.g. require-tags = { level = \"error\", tags = [\"owner\"] }"
                    .to_string(),
            ),
        );
        return;
    }
    for project in &ws.projects {
        let mut tags_by_fqn: HashMap<&str, BTreeSet<&str>> = HashMap::new();
        for element in &project.elements {
            tags_by_fqn.entry(&element.fqn).or_default().extend(element.tags.iter().map(String::as_str));
        }
        for extend in &project.extends {
            tags_by_fqn.entry(&extend.target).or_default().extend(extend.tags.iter().map(String::as_str));
        }
        let mut seen = BTreeSet::new();
        for element in &project.elements {
            if !seen.insert(element.fqn.as_str()) {
                continue;
            }
            let applies = kinds.is_empty() || element.kind.as_ref().is_some_and(|k| kinds.contains(k));
            if !applies {
                continue;
            }
            let present = tags_by_fqn.get(element.fqn.as_str());
            let missing: Vec<String> = required
                .iter()
                .filter(|t| !present.is_some_and(|p| p.contains(t.as_str())))
                .map(|t| format!("#{t}"))
                .collect();
            if !missing.is_empty() {
                let list = missing.join(", ");
                cx.report(
                    element.site,
                    format!("Element '{}' is missing required tag(s): {list}", element.fqn),
                    Some(format!("add the tag(s) to the element body: {list}")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{enable, lint_files, messages, strings};
    use crate::Level;

    const SRC: &str = "specification {\n  element system\n  element container\n  tag owner\n  tag team\n}\nmodel {\n  a = system {\n    #owner #team\n  }\n  b = system {\n    #owner\n  }\n  c = system\n  d = container\n  extend c {\n    #owner #team\n  }\n}\n";

    #[test]
    fn reports_missing_tags_for_configured_kinds() {
        let config = enable(
            "require-tags",
            Level::Error,
            &[("kinds", strings(&["system"])), ("tags", strings(&["owner", "team"]))],
        );
        let diags = lint_files(&[("t.c4", SRC)], &[], &config);
        assert_eq!(messages(&diags, "require-tags"), ["Element 'b' is missing required tag(s): #team"]);
    }

    #[test]
    fn empty_kinds_means_all_elements() {
        let config = enable("require-tags", Level::Warning, &[("tags", strings(&["owner"]))]);
        let diags = lint_files(&[("t.c4", SRC)], &[], &config);
        assert_eq!(messages(&diags, "require-tags"), ["Element 'd' is missing required tag(s): #owner"]);
    }

    #[test]
    fn missing_tags_option_is_a_config_problem() {
        let config = enable("require-tags", Level::Warning, &[]);
        let diags = lint_files(&[("t.c4", SRC)], &[], &config);
        assert_eq!(messages(&diags, "require-tags"), ["require-tags: no `tags` configured"]);
    }
}
