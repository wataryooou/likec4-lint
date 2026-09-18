//! `unused-deployment-node-kind`: a deployment node kind declared but never used, as seen from
//! the declaring project (uses in the project's documents and in any importing document count).

use crate::model::KindScope;
use crate::rules::{Cx, Usage};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unused-deployment-node-kind",
    default_level: Level::Warning,
    description: "Deployment node kind declared but never used",
    options: &[],
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    let usage = Usage::collect(
        ws,
        ws.projects
            .iter()
            .flat_map(|p| p.kind_refs.iter())
            .filter(|k| matches!(k.scope, KindScope::DeploymentNode | KindScope::Node))
            .map(|k| (k.site.doc, k.name.as_str())),
    );
    for (id, project) in ws.projects.iter().enumerate() {
        for (name, sites) in &project.spec.deployment_node_kinds {
            if let Some(first) = sites.first().filter(|_| !usage.is_used(id, name)) {
                cx.report(
                    *first,
                    format!("Deployment node kind '{name}' is declared but never used"),
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
    fn reports_unused_kind() {
        let src =
            "specification {\n  deploymentNode node\n  deploymentNode zone\n}\ndeployment {\n  node prod\n}\n";
        assert_eq!(
            messages(&lint_one(src), "unused-deployment-node-kind"),
            ["Deployment node kind 'zone' is declared but never used"]
        );
    }

    #[test]
    fn use_in_predicates_counts() {
        let files = [
            ("p/spec.c4", "specification {\n  deploymentNode node\n}\n"),
            ("p/views.c4", "views {\n  view v {\n    include * where kind is node\n  }\n}\n"),
        ];
        let diags = lint_files(&files, &["p"], &LintConfig::default());
        assert!(messages(&diags, "unused-deployment-node-kind").is_empty());
        assert!(messages(&diags, "unknown-deployment-node-kind").is_empty());
    }

    /// Visibility mirrors `unknown-deployment-node-kind`: a use in another project counts only
    /// when that document imports, so `unused` and `unknown` are never both silent or both loud.
    #[test]
    fn use_in_another_project_counts_only_with_imports() {
        let files = [
            ("p/spec.c4", "specification {\n  deploymentNode node\n}\n"),
            ("q/deploy.c4", "deployment {\n  node prod\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert_eq!(
            messages(&diags, "unused-deployment-node-kind"),
            ["Deployment node kind 'node' is declared but never used"]
        );
        assert_eq!(messages(&diags, "unknown-deployment-node-kind"), ["Unknown deployment node kind 'node'"]);

        let files = [
            ("p/spec.c4", "specification {\n  deploymentNode node\n}\n"),
            ("q/deploy.c4", "import { x } from 'p'\ndeployment {\n  node prod\n}\n"),
        ];
        let diags = lint_files(&files, &["p", "q"], &LintConfig::default());
        assert!(messages(&diags, "unused-deployment-node-kind").is_empty());
        assert!(messages(&diags, "unknown-deployment-node-kind").is_empty());
    }
}
