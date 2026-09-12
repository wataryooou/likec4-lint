//! `invalid-opacity`: `opacity` outside 0%..100%.

use likec4_syntax::ast::{self, AstNode, LeafProperty};
use likec4_syntax::SyntaxKind::*;

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "invalid-opacity",
    default_level: Level::Warning,
    description: "Opacity outside 0%..100%",
    options: &[],
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for (doc, document) in ws.documents.iter().enumerate() {
        for node in document.syntax().descendants().filter(|n| n.kind() == OPACITY_PROPERTY) {
            let Some(value) = ast::OpacityProperty::cast(node).and_then(|o| o.value_token()) else {
                continue;
            };
            if value.kind() != PERCENT {
                continue;
            }
            let percent = value.text().trim_end_matches('%').parse::<u32>();
            if !percent.is_ok_and(|v| v <= 100) {
                cx.report_range(
                    doc,
                    value.text_range(),
                    "Value ignored, must be between 0% and 100%",
                    Some("use an opacity between 0% and 100%".to_string()),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_out_of_range_opacity() {
        let src = "model {\n  a = system {\n    style {\n      opacity 120%\n    }\n  }\n}\n";
        assert_eq!(
            messages(&lint_one(src), "invalid-opacity"),
            ["Value ignored, must be between 0% and 100%"]
        );
    }

    #[test]
    fn valid_opacity_is_fine() {
        let src = "model {\n  a = system {\n    style {\n      opacity 0%\n    }\n  }\n  b = system {\n    style {\n      opacity 100%\n    }\n  }\n}\n";
        assert!(messages(&lint_one(src), "invalid-opacity").is_empty());
    }
}
