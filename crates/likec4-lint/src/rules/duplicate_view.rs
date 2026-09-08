//! `duplicate-view`: the same view name declared more than once in a project.

use std::collections::BTreeMap;

use crate::model::Site;
use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "duplicate-view",
    default_level: Level::Error,
    description: "Same view name declared twice in a project",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        let mut first: BTreeMap<&str, Site> = BTreeMap::new();
        for view in &project.views {
            match first.get(view.name.as_str()) {
                None => {
                    first.insert(&view.name, view.site);
                }
                Some(original) => cx.report(
                    view.site,
                    format!("Duplicate view '{}'", view.name),
                    Some(format!("first declared at {}", ws.location(*original))),
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, of_rule};

    #[test]
    fn reports_duplicates_across_view_kinds() {
        let src = "views {\n  view index {\n    include *\n  }\n  dynamic view index {\n  }\n  deployment view prod {\n    include *\n  }\n  view prod {\n    include *\n  }\n}\n";
        let diags = of_rule(&lint_one(src), "duplicate-view");
        let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages, ["Duplicate view 'index'", "Duplicate view 'prod'"]);
        assert_eq!(diags[0].help.as_deref(), Some("first declared at test.c4:2:8"));
    }

    #[test]
    fn anonymous_and_distinct_views_are_fine() {
        let src = "views {\n  view {\n    include *\n  }\n  view {\n    include *\n  }\n  view a {\n    include *\n  }\n}\n";
        assert!(of_rule(&lint_one(src), "duplicate-view").is_empty());
    }
}
