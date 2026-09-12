//! `deprecated-element-predicate`: `element.kind = x` / `element.tag = #x` expressions.

use likec4_syntax::ast::{self, AstNode};
use likec4_syntax::SyntaxKind::*;

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "deprecated-element-predicate",
    default_level: Level::Warning,
    description: "`element.kind` / `element.tag` expressions (kept for backwards compatibility)",
    options: &[],
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for (doc, document) in ws.documents.iter().enumerate() {
        for node in document.syntax().descendants() {
            let (message, help) = match node.kind() {
                ELEMENT_KIND_EXPRESSION => {
                    let expr = ast::ElementKindExpression::cast(node.clone());
                    let kind = expr
                        .as_ref()
                        .and_then(|e| e.kind_token())
                        .map_or("x".to_string(), |t| t.text().to_string());
                    let op = if expr.is_some_and(|e| e.is_negated()) { "is not" } else { "is" };
                    (
                        "'element.kind' predicates are deprecated".to_string(),
                        format!("use a where clause instead: * where kind {op} {kind}"),
                    )
                }
                ELEMENT_TAG_EXPRESSION => {
                    let expr = ast::ElementTagExpression::cast(node.clone());
                    let tag = expr
                        .as_ref()
                        .and_then(|e| e.tag_ref())
                        .and_then(|t| t.name())
                        .unwrap_or_else(|| "x".to_string());
                    let op = if expr.is_some_and(|e| e.is_negated()) { "is not" } else { "is" };
                    (
                        "'element.tag' predicates are deprecated".to_string(),
                        format!("use a where clause instead: * where tag {op} #{tag}"),
                    )
                }
                _ => continue,
            };
            cx.report_range(doc, node.text_range(), message, Some(help));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, of_rule};

    #[test]
    fn reports_legacy_expressions() {
        let src = "views {\n  view v {\n    include element.kind = system\n    exclude element.tag != #legacy\n  }\n}\n";
        let diags = of_rule(&lint_one(src), "deprecated-element-predicate");
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].help.as_deref(), Some("use a where clause instead: * where kind is system"));
        assert_eq!(diags[1].help.as_deref(), Some("use a where clause instead: * where tag is not #legacy"));
    }

    #[test]
    fn where_clauses_are_fine() {
        let src = "views {\n  view v {\n    include * where kind is system and tag is #legacy\n  }\n}\n";
        assert!(of_rule(&lint_one(src), "deprecated-element-predicate").is_empty());
    }
}
