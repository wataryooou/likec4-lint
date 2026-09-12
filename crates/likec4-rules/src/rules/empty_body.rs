//! `empty-body`: `{ }` with nothing but whitespace or comments inside.
//!
//! Every node kind that owns a brace pair is checked: top-level blocks (`specification`,
//! `model`, `views`, `deployment`, `global`), specification kind / tag bodies, element,
//! relation, extend, deployment node and instance bodies, view bodies, `style` / `metadata` /
//! `with` blocks, view rule `group` and `rank` blocks, global predicate / style groups and
//! dynamic view sub-flows. Deliberately skipped: `import { ... }` (an import list, not a
//! body), `likec4lib` (the parser already requires its `icons` block) and error recovery
//! nodes.

use likec4_syntax::ast::{self, AstNode};
use likec4_syntax::SyntaxKind::*;
use likec4_syntax::{NodeOrToken, SyntaxNode};
use text_size::TextRange;

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "empty-body",
    default_level: Level::Warning,
    description: "Empty braces (comments only count as empty)",
    options: &[],
};

/// Human readable name of the braces owned by `node`; `None` for braces that are not a body.
fn body_label(node: &SyntaxNode) -> Option<String> {
    let parent = node.parent().map(|p| p.kind());
    let label = match node.kind() {
        IMPORTS | LIKEC4LIB | ERROR_NODE => return None,
        SPECIFICATION => "specification block",
        MODEL => "model block",
        VIEWS => "views block",
        DEPLOYMENT => "deployment block",
        GLOBALS => "global block",
        SPEC_ELEMENT_KIND => "element kind body",
        SPEC_DEPLOYMENT_NODE_KIND => "deploymentNode kind body",
        SPEC_RELATIONSHIP_KIND => "relationship kind body",
        SPEC_TAG => "tag body",
        ELEMENT_BODY => "element body",
        RELATION_BODY | DEPLOYMENT_RELATION_BODY => "relation body",
        EXTEND_ELEMENT_BODY | EXTEND_RELATION_BODY | EXTEND_DEPLOYMENT_BODY => "extend body",
        ELEMENT_VIEW_BODY | DYNAMIC_VIEW_BODY | DEPLOYMENT_VIEW_BODY => "view body",
        DEPLOYMENT_NODE_BODY => "deployment node body",
        DEPLOYED_INSTANCE_BODY => "instance body",
        STYLE_PROPERTY | VIEW_RULE_STYLE | GLOBAL_STYLE => "style block",
        GLOBAL_STYLE_GROUP => "style group block",
        GLOBAL_PREDICATE_GROUP | GLOBAL_DYNAMIC_PREDICATE_GROUP => "predicate group block",
        METADATA_BODY => "metadata block",
        CUSTOM_RELATION_PROPERTIES if matches!(parent, Some(STEP | STEP_SERIES)) => "step body",
        CUSTOM_ELEMENT_PROPERTIES | CUSTOM_RELATION_PROPERTIES => "with block",
        VIEW_RULE_GROUP => "group block",
        VIEW_RULE_RANK => "rank block",
        SUBFLOW_STEP => {
            let keyword = ast::SubflowStep::cast(node.clone()).and_then(|s| s.kind_token());
            let keyword = keyword.map_or("subflow".to_string(), |k| k.text().to_string());
            return Some(format!("{keyword} block"));
        }
        ALT_STEPS => "alt block",
        TRY_BLOCK => "try block",
        STEPS_BLOCK if parent == Some(CATCH_BLOCK) => "catch block",
        STEPS_BLOCK if parent == Some(FINALLY_BLOCK) => "finally block",
        STEPS_BLOCK => "steps block",
        _ => "block",
    };
    Some(label.to_string())
}

/// Range from `{` to `}` when the node's braces contain only trivia (zero-length nodes left
/// behind by error recovery, e.g. the expression list of `rank { }`, do not count).
fn empty_braces(node: &SyntaxNode) -> Option<TextRange> {
    let mut children = node.children_with_tokens();
    let open = children.by_ref().find(|el| el.kind() == L_CURLY)?;
    for element in children {
        match element {
            NodeOrToken::Token(token) if token.kind() == R_CURLY => {
                return Some(TextRange::new(open.text_range().start(), token.text_range().end()));
            }
            NodeOrToken::Token(token) if token.kind().is_trivia() => {}
            NodeOrToken::Node(node) if node.text_range().is_empty() => {}
            _ => return None,
        }
    }
    None
}

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for (doc, document) in ws.documents.iter().enumerate() {
        for node in document.syntax().descendants() {
            let Some(range) = empty_braces(&node) else { continue };
            let Some(label) = body_label(&node) else { continue };
            cx.report_range(
                doc,
                range,
                format!("Empty {label}"),
                Some("remove the empty braces or add content".to_string()),
            );
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
    fn reports_empty_top_level_spec_global_and_view_rule_blocks() {
        let src = "specification {\n  element system {\n  }\n  tag t {\n  }\n  relationship r {\n  }\n  deploymentNode node {\n  }\n}\nspecification {\n}\nmodel {\n}\nviews {\n  view v {\n    group {\n    }\n    rank {\n    }\n  }\n  dynamic view d {\n    parallel {\n    }\n    alt {\n    }\n    try {\n    } catch {\n    }\n    a -> b {\n    }\n  }\n}\nglobal {\n  predicateGroup pg {\n  }\n  dynamicPredicateGroup dpg {\n  }\n  style gs * {\n  }\n  styleGroup sg {\n  }\n}\ndeployment {\n}\nglobal {\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "empty-body"),
            [
                "Empty element kind body",
                "Empty tag body",
                "Empty relationship kind body",
                "Empty deploymentNode kind body",
                "Empty specification block",
                "Empty model block",
                "Empty group block",
                "Empty rank block",
                "Empty parallel block",
                "Empty alt block",
                "Empty try block",
                "Empty catch block",
                "Empty step body",
                "Empty predicate group block",
                "Empty predicate group block",
                "Empty style block",
                "Empty style group block",
                "Empty deployment block",
                "Empty global block",
            ]
        );
    }

    #[test]
    fn import_lists_are_not_bodies() {
        let src = "import { a } from 'p'\nmodel {\n  b = system\n}\n";
        assert!(messages(&lint_one(src), "empty-body").is_empty());
    }

    #[test]
    fn non_empty_bodies_are_fine() {
        let src = "model {\n  a = system {\n    title 'A'\n  }\n  b = system { -> a }\n}\n";
        assert!(messages(&lint_one(src), "empty-body").is_empty());
    }
}
