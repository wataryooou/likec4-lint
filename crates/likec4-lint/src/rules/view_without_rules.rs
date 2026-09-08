//! `view-without-rules`: an element or deployment view with no include/exclude rule.

use likec4_syntax::ast::{self, support, AstNode};
use likec4_syntax::SyntaxKind::*;
use likec4_syntax::SyntaxNode;

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "view-without-rules",
    default_level: Level::Warning,
    description: "Element or deployment view without include/exclude or global predicate rules",
};

fn has_predicate(body: &SyntaxNode) -> bool {
    body.descendants().any(|n| matches!(n.kind(), VIEW_RULE_PREDICATE | VIEW_RULE_GLOBAL_PREDICATE_REF))
}

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for (doc, document) in ws.documents.iter().enumerate() {
        for node in document.syntax().descendants() {
            let (name, body) = match node.kind() {
                ELEMENT_VIEW => {
                    let Some(view) = ast::ElementView::cast(node.clone()) else { continue };
                    if view.extends().is_some() {
                        continue;
                    }
                    (view.name_token(), view.body().map(|b| b.syntax().clone()))
                }
                DEPLOYMENT_VIEW => {
                    let Some(view) = ast::DeploymentView::cast(node.clone()) else { continue };
                    (view.name_token(), view.body().map(|b| b.syntax().clone()))
                }
                _ => continue,
            };
            if body.is_some_and(|b| has_predicate(&b)) {
                continue;
            }
            let Some(anchor) = name.clone().or_else(|| support::token(&node, VIEW_KW)) else { continue };
            let label = name.map_or_else(|| "Anonymous view".to_string(), |n| format!("View '{}'", n.text()));
            cx.report_range(
                doc,
                anchor.text_range(),
                format!("{label} has no include/exclude rules"),
                Some("add a predicate, e.g. include *".to_string()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_views_without_predicates() {
        let src = "views {\n  view a {\n    title 'A'\n    style * {\n      color red\n    }\n  }\n  view {\n  }\n  deployment view d {\n    autoLayout TopBottom\n  }\n  view e of x\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "view-without-rules"),
            [
                "View 'a' has no include/exclude rules",
                "Anonymous view has no include/exclude rules",
                "View 'd' has no include/exclude rules",
                "View 'e' has no include/exclude rules",
            ]
        );
    }

    #[test]
    fn predicates_groups_extends_and_dynamic_views_are_fine() {
        let src = "views {\n  view a {\n    include *\n  }\n  view b {\n    group {\n      exclude *\n    }\n  }\n  view c {\n    global predicate pg\n  }\n  view d extends a {\n  }\n  dynamic view e {\n  }\n  deployment view f {\n    include *\n  }\n}\n";
        assert!(messages(&lint_one(src), "view-without-rules").is_empty());
    }
}
