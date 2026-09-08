//! `duplicate-element`: the same FQN declared more than once in a project.

use std::collections::BTreeMap;

use crate::model::Site;
use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "duplicate-element",
    default_level: Level::Error,
    description: "Same element FQN declared twice in a project",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        let mut first: BTreeMap<&str, Site> = BTreeMap::new();
        for element in &project.elements {
            match first.get(element.fqn.as_str()) {
                None => {
                    first.insert(&element.fqn, element.site);
                }
                Some(original) => cx.report(
                    element.site,
                    format!("Duplicate element '{}'", element.fqn),
                    Some(format!("first declared at {}", ws.location(*original))),
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_files, lint_one, of_rule};
    use crate::LintConfig;

    #[test]
    fn reports_second_declaration_with_first_location() {
        let diags = lint_files(
            &[
                ("p/a.c4", "model {\n  a = system\n}\n"),
                ("p/b.c4", "model {\n  b = system {\n    c = container\n  }\n  extend b {\n    c = container\n  }\n  a = system\n}\n"),
            ],
            &["p"],
            &LintConfig::default(),
        );
        let diags = of_rule(&diags, "duplicate-element");
        let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(messages, ["Duplicate element 'b.c'", "Duplicate element 'a'"]);
        assert_eq!(diags[1].help.as_deref(), Some("first declared at p/a.c4:2:3"));
        assert_eq!(diags[1].file.to_str(), Some("p/b.c4"));
    }

    #[test]
    fn extend_and_same_name_in_different_parents_are_fine() {
        let src = "model {\n  a = system {\n    db = container\n  }\n  b = system {\n    db = container\n  }\n  extend a {\n  }\n}\n";
        assert!(of_rule(&lint_one(src), "duplicate-element").is_empty());
    }

    #[test]
    fn separate_projects_do_not_conflict() {
        let diags = lint_files(
            &[("p/a.c4", "model {\n  a = system\n}\n"), ("q/a.c4", "model {\n  a = system\n}\n")],
            &["p", "q"],
            &LintConfig::default(),
        );
        assert!(of_rule(&diags, "duplicate-element").is_empty());
    }
}
