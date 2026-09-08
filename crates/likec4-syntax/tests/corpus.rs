//! Parse every `.c4` file of the test corpus: no errors, lossless, no error nodes.

use std::path::{Path, PathBuf};
use std::time::Instant;

use likec4_syntax::{parse, SyntaxKind, SyntaxNode};
use rowan::{NodeOrToken, WalkEvent};

/// Corpus files that are *expected* to fail, with the reason (checked against the Langium grammar).
const EXPECTED_FAILURES: &[(&str, &str)] = &[
    (
        "45-preserves-empty-lines.input.c4",
        "`metadata` is directly followed by `}`; Langium's `MetadataProperty: 'metadata' MetadataBody` requires `{`",
    ),
    (
        "45-preserves-empty-lines.expected.c4",
        "same content as the input (the Langium formatter test applies edits without checking diagnostics)",
    ),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace root")
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "c4") {
            out.push(path);
        }
    }
}

fn corpus_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = Vec::new();
    collect(&root.join("tests/corpus/examples"), &mut files);
    collect(&root.join("tests/corpus/examples-formatted"), &mut files);
    collect(&root.join("tests/fixtures/formatter"), &mut files);
    assert!(files.len() >= 170, "expected the full corpus, found {} files", files.len());
    files
}

fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or_default()
}

fn error_elements(node: &SyntaxNode) -> Vec<(SyntaxKind, rowan::TextRange)> {
    node.preorder_with_tokens()
        .filter_map(|ev| match ev {
            WalkEvent::Enter(NodeOrToken::Node(n)) if n.kind() == SyntaxKind::ERROR_NODE => {
                Some((n.kind(), n.text_range()))
            }
            WalkEvent::Enter(NodeOrToken::Token(t)) if t.kind() == SyntaxKind::ERROR => {
                Some((t.kind(), t.text_range()))
            }
            _ => None,
        })
        .collect()
}

fn context(text: &str, range: rowan::TextRange) -> String {
    let start: usize = range.start().into();
    let line = text[..start].matches('\n').count() + 1;
    let line_start = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = text[start..].find('\n').map(|i| start + i).unwrap_or(text.len());
    format!("line {line}: {}", &text[line_start..line_end])
}

#[test]
fn corpus_parses_losslessly() {
    let files = corpus_files();
    let mut failures = Vec::new();
    let mut passed = 0usize;
    let mut expected_failures_seen = Vec::new();
    let mut total = std::time::Duration::ZERO;

    for path in &files {
        let text = std::fs::read_to_string(path).expect("read corpus file");
        let started = Instant::now();
        let parse = parse(&text);
        total += started.elapsed();
        let tree = parse.syntax();

        let mut problems = Vec::new();
        for err in parse.errors() {
            problems.push(format!(
                "  error {:?} at {:?} ({})",
                err.message,
                err.range,
                context(&text, err.range)
            ));
        }
        for (kind, range) in error_elements(&tree) {
            problems.push(format!("  {kind:?} at {range:?} ({})", context(&text, range)));
        }
        if tree.text().to_string() != text {
            problems.push("  tree text differs from source (not lossless)".to_string());
        }

        let expected = EXPECTED_FAILURES.iter().find(|(name, _)| *name == file_name(path));
        match (problems.is_empty(), expected) {
            (true, None) => passed += 1,
            (true, Some((name, reason))) => {
                failures.push(format!("{}: expected to fail ({reason}) but parsed fine", name))
            }
            (false, Some(_)) => expected_failures_seen.push(path.clone()),
            (false, None) => {
                failures.push(format!("{}:\n{}", path.display(), problems.join("\n")));
            }
        }
    }

    eprintln!(
        "corpus: {passed} passed, {} expected failures, {} unexpected failures, {} files, parse time {:?}",
        expected_failures_seen.len(),
        failures.len(),
        files.len(),
        total
    );
    assert!(failures.is_empty(), "corpus failures:\n{}", failures.join("\n"));
    assert_eq!(expected_failures_seen.len(), EXPECTED_FAILURES.len(), "every expected failure must occur");
}

/// Every prefix of every corpus file must parse without panicking (and losslessly).
#[test]
fn corpus_prefixes_never_panic() {
    for path in corpus_files() {
        let text = std::fs::read_to_string(&path).expect("read corpus file");
        // Bound the number of prefixes per file so the test stays fast on large inputs.
        let step = (text.len() / 150).max(11);
        let mut end = 0usize;
        while end < text.len() {
            end += step;
            while end < text.len() && !text.is_char_boundary(end) {
                end += 1;
            }
            let prefix = &text[..end.min(text.len())];
            let parse = parse(prefix);
            assert_eq!(
                parse.syntax().text().to_string(),
                prefix,
                "{}: prefix {end} not lossless",
                path.display()
            );
        }
    }
}
