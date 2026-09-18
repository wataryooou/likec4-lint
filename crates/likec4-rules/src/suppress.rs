//! Suppressing diagnostics from source comments (`likec4-lint-disable-*`), the
//! `eslint-disable-next-line` equivalent for this linter. See `docs/DESIGN.md`, "Linter",
//! and the "Suppressing diagnostics in the source" section of the README for the
//! user-facing behaviour.
//!
//! Applied once, in [`apply`], after every rule has run and before the result is sorted:
//! each document's comment tokens are scanned for a directive, and each diagnostic whose
//! `file` names that document is dropped when a directive covers its rule and line.
//! Diagnostics about the configuration itself (empty `file`) are never suppressible.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use likec4_syntax::SyntaxKind::*;
use text_size::TextSize;

use crate::model::{DocId, Workspace};
use crate::{Diagnostic, Severity};

const NEXT_LINE: &str = "likec4-lint-disable-next-line";
const LINE: &str = "likec4-lint-disable-line";
const FILE: &str = "likec4-lint-disable-file";

/// Which of the three suppression keywords a directive used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    NextLine,
    Line,
    File,
}

/// A suppression directive parsed out of one comment, before it is resolved to a target
/// line.
#[derive(Debug, PartialEq, Eq)]
struct Directive {
    kind: Kind,
    /// `None` suppresses every rule; `Some` names the rules explicitly listed.
    rules: Option<Vec<String>>,
}

/// A comment's text with its `//`, `/*` and `*/` markers stripped (not yet trimmed).
fn comment_body(kind: likec4_syntax::SyntaxKind, text: &str) -> &str {
    match kind {
        LINE_COMMENT => text.strip_prefix("//").unwrap_or(text),
        BLOCK_COMMENT => text.strip_prefix("/*").and_then(|s| s.strip_suffix("*/")).unwrap_or(text),
        _ => text,
    }
}

/// Matches `body` against `keyword` at a word boundary: `keyword` alone, or `keyword`
/// followed by whitespace and the rest of the directive. Rejects a longer identifier that
/// merely starts with `keyword` (e.g. `likec4-lint-disable-next-line-typo`), returning the
/// text after the keyword with leading whitespace trimmed.
fn strip_keyword<'a>(body: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = body.strip_prefix(keyword)?;
    match rest.chars().next() {
        None => Some(rest),
        Some(c) if c.is_whitespace() => Some(rest.trim_start()),
        _ => None,
    }
}

/// Splits the text after the keyword into rule ids: commas and whitespace both separate
/// them, and a standalone `--` token starts a free-form reason that is ignored (together
/// with everything after it). An empty result (nothing before `--`, or nothing at all) means
/// "every rule".
fn parse_rules(rest: &str) -> Option<Vec<String>> {
    let rules: Vec<String> = rest
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .take_while(|&s| s != "--")
        .map(str::to_string)
        .collect();
    if rules.is_empty() {
        None
    } else {
        Some(rules)
    }
}

/// Parses a comment body (already stripped of its `//`/`/* */` markers) into a directive, or
/// `None` when it does not start with one of the three suppression keywords.
fn parse_directive(body: &str) -> Option<Directive> {
    let body = body.trim();
    if let Some(rest) = strip_keyword(body, NEXT_LINE) {
        return Some(Directive { kind: Kind::NextLine, rules: parse_rules(rest) });
    }
    if let Some(rest) = strip_keyword(body, LINE) {
        return Some(Directive { kind: Kind::Line, rules: parse_rules(rest) });
    }
    if let Some(rest) = strip_keyword(body, FILE) {
        return Some(Directive { kind: Kind::File, rules: parse_rules(rest) });
    }
    None
}

/// Byte offsets of the line starts of one document's text, so that a byte offset can be
/// turned into a 0-based line number without rescanning the text for every diagnostic.
struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        LineIndex { line_starts }
    }

    /// 0-based line containing byte `offset`.
    fn line_of(&self, offset: TextSize) -> usize {
        let offset: usize = offset.into();
        // `line_starts[0] == 0`, so at least one start is `<= offset`.
        self.line_starts.partition_point(|&start| start <= offset) - 1
    }
}

/// A directive resolved to the line(s) of its own document that it covers.
struct Suppression {
    /// `None` covers the whole document (`likec4-lint-disable-file`); `Some(line)` covers
    /// only that 0-based line.
    line: Option<usize>,
    rules: Option<Vec<String>>,
}

impl Suppression {
    fn covers(&self, line: usize, rule: &str) -> bool {
        let in_scope = self.line.is_none_or(|target| target == line);
        in_scope && self.rules.as_ref().is_none_or(|rules| rules.iter().any(|r| r == rule))
    }
}

fn unknown_rule_diagnostic(file: &Path, range: text_size::TextRange, rule: &str) -> Diagnostic {
    Diagnostic {
        rule: "unknown-rule".to_string(),
        severity: Severity::Warning,
        message: format!("Unknown rule '{rule}' in suppression comment"),
        file: file.to_path_buf(),
        range,
        help: Some("run likec4-lint lint --list-rules to see the known rules".to_string()),
        related: None,
    }
}

/// Scans every document for suppression comments and drops the diagnostics they cover. Also
/// reports `unknown-rule` for a rule id named in a directive that isn't a known rule (at the
/// comment's location); that diagnostic is itself subject to suppression, like any other.
///
/// Diagnostics with an empty `file` (about the configuration itself) pass through untouched,
/// as do diagnostics whose `file` does not match any document (should not happen in
/// practice, since every diagnostic's `file` comes from a document in the same workspace).
pub(crate) fn apply(workspace: &Workspace<'_>, mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    // `unknown-rule` is not a registered rule, but a directive may name it to silence the
    // diagnostic another directive produced, so it counts as known here.
    let known: HashSet<&str> = crate::rules().iter().map(|r| r.id).chain(["unknown-rule"]).collect();
    let mut doc_by_path: HashMap<&Path, DocId> = HashMap::with_capacity(workspace.documents.len());
    let mut line_indexes: Vec<LineIndex> = Vec::with_capacity(workspace.documents.len());
    let mut suppressions: Vec<Vec<Suppression>> = Vec::with_capacity(workspace.documents.len());

    for (doc_id, document) in workspace.documents.iter().enumerate() {
        doc_by_path.insert(document.path(), doc_id);
        let line_index = LineIndex::new(&document.file.text);
        let mut doc_suppressions = Vec::new();
        let comments = document
            .syntax()
            .descendants_with_tokens()
            .filter_map(|el| el.into_token())
            .filter(|t| t.kind().is_comment());
        for token in comments {
            let Some(directive) = parse_directive(comment_body(token.kind(), token.text())) else {
                continue;
            };
            for rule in directive.rules.iter().flatten() {
                if !known.contains(rule.as_str()) {
                    diagnostics.push(unknown_rule_diagnostic(document.path(), token.text_range(), rule));
                }
            }
            let line = match directive.kind {
                Kind::File => None,
                Kind::Line => Some(line_index.line_of(token.text_range().start())),
                Kind::NextLine => Some(line_index.line_of(token.text_range().end()) + 1),
            };
            doc_suppressions.push(Suppression { line, rules: directive.rules });
        }
        line_indexes.push(line_index);
        suppressions.push(doc_suppressions);
    }

    diagnostics.retain(|d| {
        if d.file.as_os_str().is_empty() {
            return true;
        }
        let Some(&doc_id) = doc_by_path.get(d.file.as_path()) else { return true };
        let line = line_indexes[doc_id].line_of(d.range.start());
        !suppressions[doc_id].iter().any(|s| s.covers(line, &d.rule))
    });
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(body: &str) -> Option<Vec<String>> {
        parse_directive(body).expect("directive").rules
    }

    #[test]
    fn parses_all_three_keywords() {
        assert_eq!(parse_directive("likec4-lint-disable-next-line").unwrap().kind, Kind::NextLine);
        assert_eq!(parse_directive("likec4-lint-disable-line").unwrap().kind, Kind::Line);
        assert_eq!(parse_directive("likec4-lint-disable-file").unwrap().kind, Kind::File);
    }

    #[test]
    fn line_comment_and_block_comment_markers_are_stripped() {
        assert_eq!(comment_body(LINE_COMMENT, "// likec4-lint-disable-line"), " likec4-lint-disable-line");
        assert_eq!(
            comment_body(BLOCK_COMMENT, "/* likec4-lint-disable-line */"),
            " likec4-lint-disable-line "
        );
    }

    #[test]
    fn missing_rule_list_means_every_rule() {
        assert_eq!(rules("likec4-lint-disable-next-line"), None);
        assert_eq!(rules("likec4-lint-disable-next-line   "), None);
    }

    #[test]
    fn rule_list_accepts_commas_and_whitespace_separators() {
        assert_eq!(
            rules("likec4-lint-disable-next-line unused-tag, unused-element-kind  reserved-name"),
            Some(vec!["unused-tag".into(), "unused-element-kind".into(), "reserved-name".into()])
        );
        assert_eq!(
            rules("likec4-lint-disable-next-line unused-tag,unused-element-kind"),
            Some(vec!["unused-tag".into(), "unused-element-kind".into()])
        );
    }

    #[test]
    fn a_double_dash_starts_an_ignored_reason() {
        assert_eq!(
            rules("likec4-lint-disable-next-line unused-tag -- kept for the next release"),
            Some(vec!["unused-tag".into()])
        );
        // No rule list at all, just a reason: still "every rule", not a rule named "--".
        assert_eq!(rules("likec4-lint-disable-next-line -- todo"), None);
    }

    #[test]
    fn unrelated_comments_are_ignored() {
        assert!(parse_directive("just a comment").is_none());
        assert!(parse_directive("TODO: likec4-lint-disable-next-line later").is_none());
    }

    #[test]
    fn a_longer_identifier_is_not_mistaken_for_the_keyword() {
        assert!(parse_directive("likec4-lint-disable-next-line-typo unused-tag").is_none());
        assert!(parse_directive("likec4-lint-disable-liner unused-tag").is_none());
        assert!(parse_directive("likec4-lint-disable-filesystem").is_none());
    }

    #[test]
    fn leading_and_trailing_whitespace_around_the_body_is_ignored() {
        assert_eq!(parse_directive("   likec4-lint-disable-file   ").unwrap().kind, Kind::File);
    }

    #[test]
    fn line_index_finds_the_zero_based_line_of_an_offset() {
        let index = LineIndex::new("aaa\nbb\nc");
        assert_eq!(index.line_of(TextSize::from(0)), 0);
        assert_eq!(index.line_of(TextSize::from(3)), 0);
        assert_eq!(index.line_of(TextSize::from(4)), 1);
        assert_eq!(index.line_of(TextSize::from(7)), 2);
    }

    // Integration tests through `lint()`, using `unused-tag` (a warning enabled by default,
    // reported at the declaration site) and `reserved-name` as the two rules to play off
    // each other.
    mod filtering {
        use crate::rules::testing::{lint_one, messages};

        #[test]
        fn next_line_suppresses_only_the_line_after_the_comment() {
            let src = "specification {\n  element system\n  tag a\n  // likec4-lint-disable-next-line unused-tag\n  tag b\n  tag c\n}\nmodel {\n  x = system\n}\n";
            let diags = lint_one(src);
            assert_eq!(
                messages(&diags, "unused-tag"),
                ["Tag 'a' is declared but never used", "Tag 'c' is declared but never used"]
            );
        }

        #[test]
        fn disable_line_suppresses_only_its_own_line() {
            let src = "specification {\n  element system\n  tag a\n  tag b // likec4-lint-disable-line unused-tag\n  tag c\n}\nmodel {\n  x = system\n}\n";
            let diags = lint_one(src);
            assert_eq!(
                messages(&diags, "unused-tag"),
                ["Tag 'a' is declared but never used", "Tag 'c' is declared but never used"]
            );
        }

        #[test]
        fn disable_file_suppresses_the_whole_document_regardless_of_position() {
            let src = "specification {\n  element system\n  tag a\n  tag b\n}\nmodel {\n  x = system\n  // likec4-lint-disable-file unused-tag\n}\n";
            let diags = lint_one(src);
            assert!(messages(&diags, "unused-tag").is_empty(), "{diags:?}");
        }

        #[test]
        fn disable_file_at_the_top_of_the_document_is_the_canonical_placement() {
            let src = "// likec4-lint-disable-file unused-tag\nspecification {\n  element system\n  tag a\n  tag b\n}\nmodel {\n  x = system\n}\n";
            let diags = lint_one(src);
            assert!(messages(&diags, "unused-tag").is_empty(), "{diags:?}");
        }

        #[test]
        fn a_rule_scoped_directive_does_not_suppress_other_rules() {
            let src = "specification {\n  element system\n  // likec4-lint-disable-next-line reserved-name\n  tag a\n}\nmodel {\n  x = system\n}\n";
            let diags = lint_one(src);
            assert_eq!(messages(&diags, "unused-tag"), ["Tag 'a' is declared but never used"]);
        }

        #[test]
        fn an_unknown_rule_id_is_reported_and_does_not_suppress_anything() {
            let src = "specification {\n  element system\n  // likec4-lint-disable-next-line no-such-rule\n  tag a\n}\nmodel {\n  x = system\n}\n";
            let diags = lint_one(src);
            assert_eq!(messages(&diags, "unused-tag"), ["Tag 'a' is declared but never used"]);
            assert_eq!(
                messages(&diags, "unknown-rule"),
                ["Unknown rule 'no-such-rule' in suppression comment"]
            );
        }

        #[test]
        fn a_known_rule_still_suppresses_next_to_an_unknown_one_in_the_same_list() {
            let src = "specification {\n  element system\n  // likec4-lint-disable-next-line no-such-rule, unused-tag\n  tag a\n}\nmodel {\n  x = system\n}\n";
            let diags = lint_one(src);
            assert!(messages(&diags, "unused-tag").is_empty(), "{diags:?}");
            assert_eq!(
                messages(&diags, "unknown-rule"),
                ["Unknown rule 'no-such-rule' in suppression comment"]
            );
        }

        #[test]
        fn a_multiline_block_comments_next_line_is_the_line_after_its_closing_marker() {
            let src = "specification {\n  element system\n  /* likec4-lint-disable-next-line\n     unused-tag */\n  tag a\n  tag b\n}\nmodel {\n  x = system\n}\n";
            let diags = lint_one(src);
            assert_eq!(messages(&diags, "unused-tag"), ["Tag 'b' is declared but never used"]);
        }
        #[test]
        fn unknown_rule_itself_can_be_named_and_suppressed() {
            let src = "// likec4-lint-disable-file unknown-rule\nspecification {\n  element system\n  // likec4-lint-disable-next-line no-such-rule\n  tag a\n}\nmodel {\n  x = system\n}\n";
            let diags = lint_one(src);
            assert!(messages(&diags, "unknown-rule").is_empty(), "{diags:?}");
            assert_eq!(messages(&diags, "unused-tag"), ["Tag 'a' is declared but never used"]);
        }
    }
}
