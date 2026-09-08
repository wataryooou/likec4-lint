//! Port of `LikeC4Formatter.normalizeQuotes` / `quotesNormalizerFactory` /
//! `escapeQuotesInternalQuotes` / `getAutoQuoteStyle`.

use likec4_syntax::SyntaxKind::*;
use likec4_syntax::{SyntaxNode, SyntaxToken};
use text_size::TextRange;

use crate::engine::TextEdit;
use crate::QuoteStyle;

fn strings(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|el| el.into_token())
        .filter(|t| matches!(t.kind(), STRING | MARKDOWN_STRING))
        .collect()
}

/// String tokens targeted by `normalizeQuotes` for one AST node.
fn targets(node: &SyntaxNode) -> Vec<SyntaxToken> {
    match node.kind() {
        METADATA_ATTRIBUTE => {
            if let Some(array) = node.children().find(|n| n.kind() == METADATA_ARRAY) {
                strings(&array)
            } else {
                strings(node)
            }
        }
        // ast.isStringProperty: every `key value` string property, in any context.
        STRING_PROPERTY | SPEC_STRING_PROPERTY => strings(node),
        // ast.isElement: props (title, summary, technology, tags)
        ELEMENT => strings(node),
        // ast.isImportsFromPoject: project
        IMPORTS => strings(node),
        // ast.isRelation / ast.isDeploymentRelation: title and technology (not description)
        RELATION | DEPLOYMENT_RELATION => {
            let s = strings(node);
            let mut out = Vec::new();
            if let Some(t) = s.first() {
                out.push(t.clone());
            }
            if let Some(t) = s.get(2) {
                out.push(t.clone());
            }
            out
        }
        // ast.isViewRuleGroup / ast.isStepStatement / ast.isLinkProperty: title
        VIEW_RULE_GROUP | STEP | STEP_SERIES | SUBFLOW_STEP | ALT_STEPS | TRY_BLOCK | CATCH_BLOCK
        | FINALLY_BLOCK | LINK_PROPERTY => strings(node),
        // ast.isDeploymentNode / ast.isDeployedInstance: title only (not summary)
        DEPLOYMENT_NODE | DEPLOYED_INSTANCE => strings(node).into_iter().take(1).collect(),
        _ => Vec::new(),
    }
}

/// Compute the quote-normalisation edits for the whole document.
pub fn normalize(root: &SyntaxNode, text: &str, style: QuoteStyle) -> Vec<TextEdit> {
    if style == QuoteStyle::Ignore {
        return Vec::new();
    }
    let mut nodes: Vec<SyntaxToken> = Vec::new();
    for node in root.descendants() {
        nodes.extend(targets(&node));
    }
    let style = match style {
        QuoteStyle::Auto => {
            let double = nodes.iter().filter(|t| t.text().starts_with('"')).count();
            if double * 2 >= nodes.len() {
                QuoteStyle::Double
            } else {
                QuoteStyle::Single
            }
        }
        other => other,
    };
    let quote_to_insert = if style == QuoteStyle::Single { '\'' } else { '"' };
    let markdown_fence = quote_to_insert.to_string().repeat(3);
    let plain_fence = quote_to_insert.to_string();

    nodes
        .into_iter()
        .map(|node| {
            let src = node.text();
            let fence = if src.starts_with("\"\"\"") || src.starts_with("'''") {
                &markdown_fence
            } else {
                &plain_fence
            };
            let inner = &src[fence.len()..src.len() - fence.len()];
            let new_text = format!("{fence}{}{fence}", escape_internal_quotes(inner, quote_to_insert));
            let _ = text;
            TextEdit { range: node.text_range(), new_text }
        })
        .filter(|e| !e.new_text.is_empty())
        .collect()
}

/// `escapeQuotesInternalQuotes`: escape every unescaped occurrence of `quote`.
fn escape_internal_quotes(text: &str, quote: char) -> String {
    let bytes = text.as_bytes();
    let q = quote as u8;
    let mut result = String::with_capacity(text.len() + 4);
    let mut start = 0usize;
    loop {
        let Some(rel) = bytes[start..].iter().position(|&b| b == q) else {
            result.push_str(&text[start..]);
            break;
        };
        let mut pos = start + rel;
        result.push_str(&text[start..pos]);
        start = pos + 1;
        let mut escaped = false;
        while pos > 0 && bytes[pos - 1] == b'\\' {
            escaped = !escaped;
            pos -= 1;
        }
        if escaped {
            result.push(quote);
        } else {
            result.push('\\');
            result.push(quote);
        }
    }
    result
}

#[allow(dead_code)]
fn _range(_: TextRange) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_unescaped_quotes_only() {
        assert_eq!(escape_internal_quotes("it's", '\''), "it\\'s");
        assert_eq!(escape_internal_quotes("it\\'s", '\''), "it\\'s");
        assert_eq!(escape_internal_quotes("a\\\\'b", '\''), "a\\\\\\'b");
        assert_eq!(escape_internal_quotes("say \"hi\"", '"'), "say \\\"hi\\\"");
        assert_eq!(escape_internal_quotes("plain", '"'), "plain");
    }
}
