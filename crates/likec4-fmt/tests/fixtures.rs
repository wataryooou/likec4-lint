//! Compatibility tests against the official formatter output.
//!
//! - `tests/fixtures/formatter/*`: pairs extracted from `LikeC4Formatter.spec.ts` (quoteStyle single)
//! - `tests/fixtures/formatter-cli/*`: pairs generated with `likec4 format` (quoteStyle auto)
//! - `tests/corpus/examples` vs `tests/corpus/examples-formatted`: official examples (quoteStyle auto)

use std::fs;
use std::path::{Path, PathBuf};

use likec4_fmt::{format, FormatOptions, QuoteStyle};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn diff(expected: &str, actual: &str) -> String {
    similar::TextDiff::from_lines(expected, actual)
        .unified_diff()
        .context_radius(3)
        .header("expected", "actual")
        .to_string()
}

struct Case {
    name: String,
    input: String,
    expected: String,
}

fn pairs(dir: &Path, suffix_in: &str, suffix_out: &str) -> Vec<Case> {
    let mut cases = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return cases };
    let mut inputs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(suffix_in))
        .collect();
    inputs.sort();
    for input in inputs {
        let expected = PathBuf::from(input.to_string_lossy().replace(suffix_in, suffix_out));
        cases.push(Case {
            name: input.file_name().unwrap().to_string_lossy().to_string(),
            input: fs::read_to_string(&input).unwrap(),
            expected: fs::read_to_string(&expected)
                .unwrap_or_else(|_| panic!("missing {}", expected.display())),
        });
    }
    cases
}

fn run(cases: &[Case], options: &FormatOptions, idempotent: bool) {
    assert!(!cases.is_empty(), "no fixtures found");
    let mut failures = Vec::new();
    for case in cases {
        match format(&case.input, options) {
            Ok(actual) => {
                if actual != case.expected {
                    failures.push(format!("--- {} ---\n{}", case.name, diff(&case.expected, &actual)));
                } else if idempotent {
                    let again = format(&actual, options).unwrap();
                    if again != actual {
                        failures.push(format!(
                            "--- {} (not idempotent) ---\n{}",
                            case.name,
                            diff(&actual, &again)
                        ));
                    }
                }
            }
            // The official formatter leaves documents with syntax errors untouched.
            Err(_) if case.expected == case.input => {}
            Err(err) => failures.push(format!("--- {} ---\nformat error: {err}\n{:#?}", case.name, err)),
        }
    }
    if !failures.is_empty() {
        panic!("{} of {} fixture(s) failed:\n\n{}", failures.len(), cases.len(), failures.join("\n"));
    }
}

#[test]
fn spec_fixtures_single_quotes() {
    let dir = repo_root().join("tests/fixtures/formatter");
    let cases = pairs(&dir, ".input.c4", ".expected.c4");
    let options = FormatOptions { quote_style: QuoteStyle::Single, ..FormatOptions::default() };
    run(&cases, &options, true);
}

#[test]
fn cli_fixtures_auto_quotes() {
    let dir = repo_root().join("tests/fixtures/formatter-cli");
    let cases = pairs(&dir, ".input.c4", ".expected.c4");
    if cases.is_empty() {
        eprintln!("no CLI fixtures yet; skipping");
        return;
    }
    run(&cases, &FormatOptions::default(), true);
}

#[test]
fn quirk_fixtures_auto_quotes() {
    let dir = repo_root().join("tests/fixtures/formatter-quirks");
    let cases = pairs(&dir, ".input.c4", ".expected.c4");
    run(&cases, &FormatOptions::default(), true);
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "c4") {
            out.push(path);
        }
    }
}

#[test]
fn official_examples_match_official_output() {
    let root = repo_root();
    let original = root.join("tests/corpus/examples");
    let formatted = root.join("tests/corpus/examples-formatted");
    let mut files = Vec::new();
    walk(&original, &mut files);
    files.sort();
    let cases: Vec<Case> = files
        .into_iter()
        .map(|input| {
            let rel = input.strip_prefix(&original).unwrap();
            Case {
                name: rel.to_string_lossy().to_string(),
                input: fs::read_to_string(&input).unwrap(),
                expected: fs::read_to_string(formatted.join(rel)).unwrap(),
            }
        })
        .collect();
    run(&cases, &FormatOptions::default(), true);
}
