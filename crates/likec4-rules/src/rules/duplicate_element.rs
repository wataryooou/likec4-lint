//! `duplicate-element`: the same FQN declared more than once in a project. Model elements
//! and the deployment namespace (nodes and deployed instances) are checked separately: an
//! element and a deployment node may share a name.

use std::collections::BTreeMap;

use crate::model::Site;
use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "duplicate-element",
    default_level: Level::Error,
    description: "Same element or deployment node FQN declared twice in a project",
    options: &[],
};

const HELP: &str = "rename or remove one of the declarations";

/// Report every declaration after the first of each name; `decls` must be in source order.
fn report_duplicates<'d>(
    cx: &mut Cx<'_, '_>,
    decls: impl IntoIterator<Item = (&'d str, Site, &'static str)>,
) {
    let mut first: BTreeMap<&str, Site> = BTreeMap::new();
    for (name, site, what) in decls {
        match first.get(name) {
            None => {
                first.insert(name, site);
            }
            Some(original) => cx.report_related(
                site,
                format!("Duplicate {what} '{name}'"),
                Some(HELP.to_string()),
                *original,
                "first declared here",
            ),
        }
    }
}

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        report_duplicates(cx, project.elements.iter().map(|e| (e.fqn.as_str(), e.site, "element")));
        let nodes = project.deployment_nodes.iter().map(|n| (n.name.as_str(), n.site, "deployment node"));
        let instances =
            project.deployment_instances.iter().map(|n| (n.name.as_str(), n.site, "deployment instance"));
        let mut deployment: Vec<_> = nodes.chain(instances).collect();
        deployment.sort_by_key(|(_, site, _)| (site.doc, site.range.start()));
        report_duplicates(cx, deployment);
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use text_size::TextRange;

    use crate::rules::testing::{lint_files, lint_one, of_rule};
    use crate::{LintConfig, RelatedLocation};

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
        assert_eq!(diags[1].file.to_str(), Some("p/b.c4"));
        assert_eq!(diags[1].help.as_deref(), Some("rename or remove one of the declarations"));
        assert_eq!(
            diags[1].related,
            Some(RelatedLocation {
                file: PathBuf::from("p/a.c4"),
                range: TextRange::new(10.into(), 11.into()),
                message: "first declared here".to_string(),
            })
        );
    }

    #[test]
    fn reports_duplicate_deployment_nodes_and_instances() {
        // The deployment namespace is separate from the model: element `prod` and node `prod` do
        // not clash, but two nodes / instances with the same deployment FQN do.
        let src = "model {\n  a = system\n  prod = system\n}\ndeployment {\n  node prod {\n    node zone\n    api = instanceOf a\n  }\n  node prod\n  extend prod {\n    node zone\n    api = instanceOf a\n  }\n}\n";
        let diags = of_rule(&lint_one(src), "duplicate-element");
        let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "Duplicate deployment node 'prod'",
                "Duplicate deployment node 'prod.zone'",
                "Duplicate deployment instance 'prod.api'",
            ]
        );
        assert!(diags.iter().all(|d| d.related.as_ref().is_some_and(|r| r.message == "first declared here")));
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
