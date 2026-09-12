//! `unknown-deployment-node-kind`: a deployment node kind without a `deploymentNode` declaration.

use crate::model::KindScope;
use crate::rules::{declared, Cx};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unknown-deployment-node-kind",
    default_level: Level::Error,
    description: "Deployment node kind not declared in any specification of the project",
    options: &[],
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        for kind in project.kind_refs.iter().filter(|k| k.scope == KindScope::DeploymentNode) {
            let name = kind.name.as_str();
            if !declared(ws, kind.site.doc, |p| p.spec.deployment_node_kinds.contains_key(name)) {
                cx.report(
                    kind.site,
                    format!("Unknown deployment node kind '{name}'"),
                    Some(format!("declare it in a specification block: deploymentNode {name}")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_undeclared_kind() {
        let diags =
            lint_one("specification {\n  deploymentNode node\n}\ndeployment {\n  node prod\n  zone eu\n}\n");
        assert_eq!(messages(&diags, "unknown-deployment-node-kind"), ["Unknown deployment node kind 'zone'"]);
    }

    #[test]
    fn declared_kind_is_fine() {
        let diags = lint_one("specification {\n  deploymentNode node\n}\ndeployment {\n  node prod\n}\n");
        assert!(messages(&diags, "unknown-deployment-node-kind").is_empty());
    }
}
