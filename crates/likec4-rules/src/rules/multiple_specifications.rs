//! `multiple-specifications`: more than one `specification` block in a document.

use likec4_syntax::ast::support;
use likec4_syntax::SyntaxKind::*;

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "multiple-specifications",
    default_level: Level::Warning,
    description: "More than one specification block in a document",
    options: &[],
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for (doc, document) in ws.documents.iter().enumerate() {
        for spec in document.syntax().children().filter(|n| n.kind() == SPECIFICATION).skip(1) {
            let range = support::token(&spec, SPECIFICATION_KW).map_or(spec.text_range(), |t| t.text_range());
            cx.report_range(
                doc,
                range,
                "Prefer one specification per document",
                Some("merge the blocks into a single specification { ... }".to_string()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, of_rule};

    #[test]
    fn reports_second_and_later_blocks() {
        let src = "specification {\n  element a\n}\nspecification {\n  element b\n}\nspecification {\n  element c\n}\n";
        let diags = of_rule(&lint_one(src), "multiple-specifications");
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].message, "Prefer one specification per document");
        // Offset of the second `specification` keyword.
        assert_eq!(u32::from(diags[0].range.start()), 30);
    }

    #[test]
    fn single_block_is_fine() {
        assert!(of_rule(&lint_one("specification {\n  element a\n}\n"), "multiple-specifications").is_empty());
    }
}
