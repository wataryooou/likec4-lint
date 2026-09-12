//! `unresolved-reference`: a reference whose target name is declared nowhere.
//!
//! Element references (relation endpoints, `extend` / `instanceOf` targets, view `of`, and the
//! elements named in view rules: include / exclude expressions, style targets, rank) are
//! resolved conservatively by their last FQN segment: they are reported only when no element
//! (or, in deployment relations and deployment views, no deployment node or instance) with
//! that name exists in the project, or in the workspace when the document imports from
//! another project.

use crate::model::{Project, RefTarget, Reference};
use crate::rules::{declared, Cx};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unresolved-reference",
    default_level: Level::Error,
    description: "Relation endpoint, extend target, navigateTo, view of/extends, view rule element or global reference that does not exist",
    options: &[],
};

fn resolves(project: &Project, reference: &Reference) -> bool {
    let name = reference.name.as_str();
    let names = &project.names;
    match reference.target {
        RefTarget::Element => names.elements.contains(name),
        RefTarget::Deployment => names.deployment.contains(name) || names.elements.contains(name),
        RefTarget::View => names.views.contains(name),
        RefTarget::PredicateGroup => project.spec.predicate_groups.contains_key(name),
        RefTarget::GlobalStyle => project.spec.styles.contains_key(name),
    }
}

/// `import { boutique } from 'shop'` makes `boutique` (and `boutique.*`) referenceable in
/// the importing document even when the other project is not part of this run.
fn resolved_by_import(ws: &crate::model::Workspace<'_>, reference: &Reference) -> bool {
    if !matches!(reference.target, RefTarget::Element | RefTarget::Deployment) {
        return false;
    }
    let first_segment = reference.text.split('.').next().unwrap_or("");
    ws.documents[reference.site.doc].imported_names.contains(first_segment)
}

fn help(reference: &Reference) -> String {
    let name = &reference.name;
    match reference.target {
        RefTarget::Element => {
            format!("declare an element named '{name}' in a model block or fix the reference")
        }
        RefTarget::Deployment => {
            format!(
                "declare a deployment node, an instance or an element named '{name}' or fix the reference"
            )
        }
        RefTarget::View => format!("declare a view named '{name}' in a views block or fix the reference"),
        RefTarget::PredicateGroup => format!("declare it in a global block: predicateGroup {name} {{ ... }}"),
        RefTarget::GlobalStyle => format!("declare it in a global block: style {name} * {{ ... }}"),
    }
}

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        for reference in &project.references {
            if resolved_by_import(ws, reference) {
                continue;
            }
            if !declared(ws, reference.site.doc, |p| resolves(p, reference)) {
                cx.report(
                    reference.site,
                    format!("{} '{}' not resolved", reference.role, reference.text),
                    Some(help(reference)),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_files, lint_one, messages};
    use crate::LintConfig;

    const SPEC: &str = "specification {\n  element system\n  deploymentNode node\n}\n";

    #[test]
    fn reports_unresolved_model_references() {
        let src = format!(
            "{SPEC}model {{\n  a = system {{\n    -> ghost\n  }}\n  a -> b\n  extend nope {{\n  }}\n  extend a -> nowhere {{\n  }}\n}}\n"
        );
        let diags = lint_one(&src);
        assert_eq!(
            messages(&diags, "unresolved-reference"),
            [
                "Relation target 'ghost' not resolved",
                "Relation target 'b' not resolved",
                "Extend target 'nope' not resolved",
                "Relation target 'nowhere' not resolved",
            ]
        );
    }

    #[test]
    fn reports_unresolved_view_references() {
        let src = format!(
            "{SPEC}model {{\n  a = system\n}}\nviews {{\n  view v of missing {{\n    include *\n    global predicate pg\n    global style gs\n  }}\n  view w extends other {{\n  }}\n  dynamic view d {{\n    a -> a {{\n      navigateTo nowhere\n    }}\n  }}\n}}\n"
        );
        let diags = lint_one(&src);
        assert_eq!(
            messages(&diags, "unresolved-reference"),
            [
                "View scope 'missing' not resolved",
                "Global predicate group 'pg' not resolved",
                "Global style 'gs' not resolved",
                "Extended view 'other' not resolved",
                "Navigation target 'nowhere' not resolved",
            ]
        );
    }

    #[test]
    fn reports_unresolved_view_rule_references() {
        let src = format!(
            "{SPEC}model {{\n  a = system\n}}\nviews {{\n  view v {{\n    include ghost1\n    exclude ghost2.*\n    include -> ghost3\n    include ghost4 -> a\n    include ghost5 where kind is system\n    include ghost6 with {{ }}\n    group {{\n      include ghost7\n    }}\n    style ghost8 {{\n      color red\n    }}\n    rank {{\n      ghost9\n    }}\n  }}\n  deployment view d {{\n    include ghost10\n  }}\n  style ghost11 {{\n    color red\n  }}\n}}\nglobal {{\n  predicateGroup pg {{\n    include ghost12\n  }}\n  style gs ghost13 {{\n    color red\n  }}\n}}\n"
        );
        let diags = lint_one(&src);
        assert_eq!(
            messages(&diags, "unresolved-reference"),
            [
                "Predicate element 'ghost1' not resolved",
                "Predicate element 'ghost2' not resolved",
                "Predicate element 'ghost3' not resolved",
                "Predicate element 'ghost4' not resolved",
                "Predicate element 'ghost5' not resolved",
                "Predicate element 'ghost6' not resolved",
                "Predicate element 'ghost7' not resolved",
                "Style target 'ghost8' not resolved",
                "Rank target 'ghost9' not resolved",
                "Predicate element 'ghost10' not resolved",
                "Style target 'ghost11' not resolved",
                "Predicate element 'ghost12' not resolved",
                "Style target 'ghost13' not resolved",
            ]
        );
    }

    #[test]
    fn view_rule_references_to_existing_names_are_fine() {
        let src = format!(
            "{SPEC}model {{\n  a = system {{\n    b = system\n  }}\n}}\ndeployment {{\n  node prod {{\n    api = instanceOf a\n  }}\n}}\nviews {{\n  view v of a {{\n    include a, a.b, a.*, a._, a.**, -> a, a -> b, a -> *, *, element.kind = system, element.tag = #t\n    include b where kind is system, a.b with {{ }}, * -> a.b\n    style a.b {{\n      color red\n    }}\n    rank {{\n      a, b\n    }}\n  }}\n  deployment view d {{\n    include prod, prod.api, a\n  }}\n}}\n"
        );
        assert!(messages(&lint_one(&src), "unresolved-reference").is_empty());
    }

    #[test]
    fn deployment_relations_accept_nodes_instances_and_elements() {
        let src = format!(
            "{SPEC}model {{\n  a = system\n  b = system\n}}\ndeployment {{\n  node prod {{\n    instanceOf a\n    api = instanceOf b\n    prod.a -> prod.api\n    prod.api -> prod.ghost\n  }}\n  extend prod {{\n    instanceOf missing\n  }}\n}}\n"
        );
        let diags = lint_one(&src);
        assert_eq!(
            messages(&diags, "unresolved-reference"),
            ["Relation target 'prod.ghost' not resolved", "Instance target 'missing' not resolved"]
        );
    }

    #[test]
    fn last_segment_resolution_is_conservative() {
        let src = format!("{SPEC}model {{\n  a = system {{\n    b = system\n  }}\n  a -> x.y.b\n}}\n");
        assert!(messages(&lint_one(&src), "unresolved-reference").is_empty());
    }

    #[test]
    fn this_and_it_are_not_references() {
        let src = format!(
            "{SPEC}model {{\n  a = system {{\n    b = system\n    this -> b\n    it -> b\n    b -> this\n    -> it\n  }}\n}}\ndeployment {{\n  node n {{\n    n -> this\n  }}\n}}\n"
        );
        assert!(messages(&lint_one(&src), "unresolved-reference").is_empty());
    }

    #[test]
    fn imported_names_resolve_without_the_other_project() {
        // Only project `q` is loaded; `shop` is introduced by the import statement.
        let files = [(
            "q/model.c4",
            "import { shop } from 'p'\nmodel {\n  app = system\n  app -> shop.frontend\n  app -> other.x\n}\n",
        )];
        let diags = lint_files(&files, &["q"], &LintConfig::default());
        assert_eq!(messages(&diags, "unresolved-reference"), ["Relation target 'other.x' not resolved"]);
    }

    #[test]
    fn imports_resolve_against_the_workspace() {
        let files = [
            ("p/model.c4", "model {\n  shop = system\n}\n"),
            ("q/model.c4", "import { shop } from 'p'\nmodel {\n  app = system\n  app -> shop\n}\n"),
            ("r/model.c4", "model {\n  other = system\n  other -> shop\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q", "r"], &LintConfig::default());
        assert_eq!(messages(&diags, "unresolved-reference"), ["Relation target 'shop' not resolved"]);
        assert_eq!(
            diags.iter().find(|d| d.rule == "unresolved-reference").map(|d| d.file.to_str()),
            Some(Some("r/model.c4"))
        );
    }
}
