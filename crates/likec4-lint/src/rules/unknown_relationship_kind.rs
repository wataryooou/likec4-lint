//! `unknown-relationship-kind`: `-[kind]->` / `.kind` without a `relationship` declaration.

use crate::model::KindScope;
use crate::rules::{declared, Cx};
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "unknown-relationship-kind",
    default_level: Level::Error,
    description: "Relationship kind not declared in any specification of the project",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for project in &ws.projects {
        for kind in project.kind_refs.iter().filter(|k| k.scope == KindScope::Relationship) {
            let name = kind.name.as_str();
            if !declared(ws, kind.site.doc, |p| p.spec.relationship_kinds.contains_key(name)) {
                cx.report(
                    kind.site,
                    format!("Unknown relationship kind '{name}'"),
                    Some(format!("declare it in a specification block: relationship {name}")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    const SPEC: &str = "specification {\n  element system\n  relationship uses\n}\n";

    #[test]
    fn reports_every_connector_form() {
        let src = format!(
            "{SPEC}model {{\n  a = system\n  b = system\n  a -[calls]-> b\n  a .queries b\n  a -[uses]-> b\n}}\nviews {{\n  view v {{\n    include a -[reads]-> b\n    include a -> b where kind is writes\n  }}\n  dynamic view d {{\n    a -[pings]-> b\n  }}\n}}\n"
        );
        let diags = lint_one(&src);
        assert_eq!(
            messages(&diags, "unknown-relationship-kind"),
            [
                "Unknown relationship kind 'calls'",
                "Unknown relationship kind 'queries'",
                "Unknown relationship kind 'reads'",
                "Unknown relationship kind 'writes'",
                "Unknown relationship kind 'pings'",
            ]
        );
    }

    #[test]
    fn declared_kind_is_fine() {
        let src = format!("{SPEC}model {{\n  a = system\n  b = system\n  a -[uses]-> b\n}}\n");
        assert!(messages(&lint_one(&src), "unknown-relationship-kind").is_empty());
    }
}
