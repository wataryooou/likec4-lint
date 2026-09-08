//! `invalid-color`: malformed hex colours and out-of-range `rgb()` / `rgba()` components.

use likec4_syntax::ast::{self, AstNode};
use likec4_syntax::SyntaxKind::*;

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo = RuleInfo {
    id: "invalid-color",
    default_level: Level::Error,
    description: "Hex colour with a bad length or rgb/rgba component out of range",
};

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for (doc, document) in ws.documents.iter().enumerate() {
        for node in document.syntax().descendants() {
            match node.kind() {
                HEX_COLOR => {
                    let Some(value) = ast::HexColor::cast(node.clone()).and_then(|h| h.value_token()) else {
                        continue;
                    };
                    let text = value.text();
                    if !text.chars().all(|c| c.is_ascii_hexdigit()) {
                        cx.report_range(
                            doc,
                            node.text_range(),
                            format!("Invalid hex color '#{text}'"),
                            Some("use 3, 6 or 8 hexadecimal digits, e.g. #ff0000".to_string()),
                        );
                    } else if !matches!(text.len(), 3 | 6 | 8) {
                        cx.report_range(
                            doc,
                            node.text_range(),
                            format!("Invalid value \"#{text}\", must be 3, 6 or 8 characters long"),
                            Some("use 3, 6 or 8 hexadecimal digits, e.g. #ff0000".to_string()),
                        );
                    }
                }
                RGBA_COLOR => {
                    let Some(rgba) = ast::RgbaColor::cast(node.clone()) else { continue };
                    for (index, component) in rgba.components().enumerate() {
                        let text = component.text();
                        let (message, help) = match (index, component.kind()) {
                            (0..=2, NUMBER) => {
                                if text.parse::<u32>().is_ok_and(|v| v <= 255) {
                                    continue;
                                }
                                (
                                    "Invalid value, must be between 0 and 255",
                                    "use an integer between 0 and 255",
                                )
                            }
                            (3, PERCENT) => {
                                let percent = text.trim_end_matches('%').parse::<u32>();
                                if percent.is_ok_and(|v| v <= 100) {
                                    continue;
                                }
                                (
                                    "Invalid value, must be between 0% and 100%",
                                    "use an alpha between 0% and 100%",
                                )
                            }
                            (3, NUMBER | FLOAT) => {
                                if text.parse::<f64>().is_ok_and(|v| (0.0..=1.0).contains(&v)) {
                                    continue;
                                }
                                ("Invalid value, must be between 0 and 1", "use an alpha between 0 and 1")
                            }
                            _ => continue,
                        };
                        cx.report_range(doc, component.text_range(), message, Some(help.to_string()));
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, messages};

    #[test]
    fn reports_bad_hex_and_rgba_values() {
        let src = "specification {\n  color a #ff\n  color b #abcdef1\n  color c #zzz\n  color d rgb(256, 0, 0)\n  color e rgba(0, 0, 0, 1.5)\n  color f rgba(0, 0, 0, 120%)\n}\n";
        let diags = lint_one(src);
        assert_eq!(
            messages(&diags, "invalid-color"),
            [
                "Invalid value \"#ff\", must be 3, 6 or 8 characters long",
                "Invalid value \"#abcdef1\", must be 3, 6 or 8 characters long",
                "Invalid hex color '#zzz'",
                "Invalid value, must be between 0 and 255",
                "Invalid value, must be between 0 and 1",
                "Invalid value, must be between 0% and 100%",
            ]
        );
    }

    #[test]
    fn valid_colors_are_fine() {
        let src = "specification {\n  color a #fff\n  color b #123456\n  color c #abcdef80\n  color d rgb(255, 255, 255)\n  color e rgba(0, 0, 0, 0.5)\n  color f rgba(0, 0, 0, 100%)\n  color g rgba(0, 0, 0, 1)\n  tag t {\n    color #000\n  }\n}\n";
        assert!(messages(&lint_one(src), "invalid-color").is_empty());
    }
}
