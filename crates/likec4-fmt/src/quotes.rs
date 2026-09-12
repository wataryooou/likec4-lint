//! Port of `LikeC4Formatter.normalizeQuotes` / `quotesNormalizerFactory` /
//! `escapeQuotesInternalQuotes` / `getAutoQuoteStyle`.

use likec4_syntax::SyntaxKind::*;
use likec4_syntax::{SyntaxNode, SyntaxToken};

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
///
/// Deviation from the official formatter: a markdown string (`'''...'''` / `"""..."""`) whose
/// content ends with the quote character being switched to is left untouched. The
/// official formatter escapes that quote and produces e.g. `"""say \"hi\""""`, which does not
/// lex (the fence is matched without regard to escapes). A leading quote is harmless:
/// `"""\"hi\" said"""` still lexes, so it is escaped and normalised like the official formatter does.
pub fn normalize(root: &SyntaxNode, style: QuoteStyle) -> Vec<TextEdit> {
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
        .filter_map(|node| {
            let src = node.text();
            let markdown = src.starts_with("\"\"\"") || src.starts_with("'''");
            let fence = if markdown { &markdown_fence } else { &plain_fence };
            let inner = &src[fence.len()..src.len() - fence.len()];
            if markdown && inner.ends_with(quote_to_insert) {
                return None;
            }
            let new_text = format!("{fence}{}{fence}", escape_internal_quotes(inner, quote_to_insert));
            Some(TextEdit { range: node.text_range(), new_text })
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{format, FormatOptions};

    #[test]
    fn escapes_unescaped_quotes_only() {
        assert_eq!(escape_internal_quotes("it's", '\''), "it\\'s");
        assert_eq!(escape_internal_quotes("it\\'s", '\''), "it\\'s");
        assert_eq!(escape_internal_quotes("a\\\\'b", '\''), "a\\\\\\'b");
        assert_eq!(escape_internal_quotes("say \"hi\"", '"'), "say \\\"hi\\\"");
        assert_eq!(escape_internal_quotes("plain", '"'), "plain");
    }

    /// A document whose `auto` vote resolves to double quotes (`"A"` vs one markdown string).
    fn doc(markdown: &str) -> String {
        format!(
            "specification {{\n  element el\n}}\nmodel {{\n  a = el \"A\" {{\n    description {markdown}\n  }}\n}}\n"
        )
    }

    #[test]
    fn markdown_string_ending_with_the_new_quote_is_left_alone() {
        // The official formatter turns this into `"""say \"hi\""""`, which does not parse.
        let input = doc("'''say \"hi\"'''");
        assert_eq!(format(&input, &FormatOptions::default()).unwrap(), input);
    }

    #[test]
    fn markdown_string_starting_with_the_new_quote_is_normalized() {
        // Only a trailing quote collides with the closing fence; a leading one is escaped
        // exactly like the official formatter does, and the result still lexes.
        let input = doc("'''\"quoted'''");
        let expected = doc("\"\"\"\\\"quoted\"\"\"");
        assert_eq!(format(&input, &FormatOptions::default()).unwrap(), expected);
        // A string that also ends with the quote is still left alone.
        let input = doc("'''\"quoted\"'''");
        assert_eq!(format(&input, &FormatOptions::default()).unwrap(), input);
    }

    #[test]
    fn markdown_string_with_inner_quotes_is_normalized() {
        let input = doc("'''say \"hi\" ok'''");
        let expected = doc("\"\"\"say \\\"hi\\\" ok\"\"\"");
        assert_eq!(format(&input, &FormatOptions::default()).unwrap(), expected);
    }
}
