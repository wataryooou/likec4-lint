//! `syntax-error`: parser diagnostics, reported as they are.

use crate::rules::Cx;
use crate::{Level, RuleInfo};

pub const INFO: RuleInfo =
    RuleInfo { id: "syntax-error", default_level: Level::Error, description: "Parser diagnostics" };

pub fn check(cx: &mut Cx<'_, '_>) {
    let ws = cx.ws;
    for (doc, document) in ws.documents.iter().enumerate() {
        for error in document.parse.errors() {
            cx.report_range(doc, error.range, error.message.clone(), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::testing::{lint_one, of_rule};
    use crate::Severity;

    #[test]
    fn reports_parser_errors() {
        let diags = of_rule(&lint_one("model {\n  a = system {\n}\n"), "syntax-error");
        assert!(!diags.is_empty());
        assert!(diags.iter().all(|d| d.severity == Severity::Error));
    }

    #[test]
    fn clean_document_has_none() {
        assert!(of_rule(&lint_one("model {\n  a = system\n}\n"), "syntax-error").is_empty());
    }
}
