//! `empty-body`: `{ }` with nothing but whitespace or comments inside.

use likec4_syntax::SyntaxKind::{self, *};
use likec4_syntax::{NodeOrToken, SyntaxNode};
use text_size::TextRange;

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "empty-body",
    default_level: Level::Warning,
    description: "Empty braces (comments only count as empty)",
};

fn body_label(kind: SyntaxKind) -> Option<&'static str> {
    Some(match kind {
        ELEMENT_BODY => "element body",
        RELATION_BODY | DEPLOYMENT_RELATION_BODY => "relation body",
        EXTEND_ELEMENT_BODY | EXTEND_RELATION_BODY | EXTEND_DEPLOYMENT_BODY => "extend body",
        ELEMENT_VIEW_BODY | DYNAMIC_VIEW_BODY | DEPLOYMENT_VIEW_BODY => "view body",
        DEPLOYMENT_NODE_BODY => "deployment node body",
        DEPLOYED_INSTANCE_BODY => "instance body",
        STYLE_PROPERTY | VIEW_RULE_STYLE => "style block",
        METADATA_BODY => "metadata block",
        CUSTOM_ELEMENT_PROPERTIES | CUSTOM_RELATION_PROPERTIES => "with block",
        _ => return None,
    })
}

/// Range from `{` to `}` when the node's braces contain only trivia.
fn empty_braces(node: &SyntaxNode) -> Option<TextRange> {
    let mut children = node.children_with_tokens();
    let open = children.by_ref().find(|el| el.kind() == L_CURLY)?;
    for element in children {
        match element {
            NodeOrToken::Token(token) if token.kind() == R_CURLY => {
                return Some(TextRange::new(open.text_range().start(), token.text_range().end()));
            }
            NodeOrToken::Token(token) if token.kind().is_trivia() => {}
            _ => return None,
        }
    }
    None
}

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for (doc, document) in ws.documents.iter().enumerate() {
        for node in document.syntax().descendants() {
            let Some(label) = body_label(node.kind()) else { continue };
            if let Some(range) = empty_braces(&node) {
                cx.report_range(
                    doc,
                    range,
                    format!("Empty {label}"),
                    Some("remove the empty braces or add content".to_string()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_empty_bodies_of_every_kind() {
        let src = "model {\n  a = system {\n  }\n  b = system {\n    // only a comment\n  }\n  c = system {\n    style { }\n    metadata {\n    }\n  }\n  a -> b {\n  }\n  extend a {\n  }\n}\nviews {\n  view v {\n  }\n  view w {\n    include * with { }\n    style * {\n    }\n  }\n}\ndeployment {\n  node n {\n  }\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "empty-body"),
            [
                "Empty element body",
                "Empty element body",
                "Empty style block",
                "Empty metadata block",
                "Empty relation body",
                "Empty extend body",
                "Empty view body",
                "Empty with block",
                "Empty style block",
                "Empty deployment node body",
            ]
        );
    }

    #[test]
    fn non_empty_bodies_are_fine() {
        let src = "model {\n  a = system {\n    title 'A'\n  }\n  b = system { -> a }\n}\n";
        assert!(messages(&lint_one(src), "empty-body").is_empty());
    }
}
