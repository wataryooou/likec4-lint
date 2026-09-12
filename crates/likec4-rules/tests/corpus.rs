//! Lint the official examples (`tests/corpus/examples`) with the default rules: no errors.
//! Warnings are printed for manual review.

use std::path::{Path, PathBuf};
use std::time::Instant;

use likec4_rules::{lint, LintConfig, Severity, SourceFile};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace root")
}

/// Collect documents and project roots below `dir` (hidden files included, unlike the CLI).
fn collect(dir: &Path, files: &mut Vec<PathBuf>, roots: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, files, roots);
        } else if likec4_rules::is_likec4_document(&path) {
            files.push(path);
        } else if likec4_rules::is_project_config(&path) {
            roots.push(dir.to_path_buf());
        }
    }
}

fn lint_corpus(subdir: &str) -> Vec<likec4_rules::Diagnostic> {
    let dir = repo_root().join("tests/corpus").join(subdir);
    let mut paths = Vec::new();
    let mut roots = Vec::new();
    collect(&dir, &mut paths, &mut roots);
    assert!(paths.len() >= 39, "expected the full corpus in {subdir}, found {} files", paths.len());
    assert!(roots.len() >= 10, "expected the project markers in {subdir}, found {}", roots.len());

    let files: Vec<SourceFile> = paths
        .iter()
        .map(|p| SourceFile { path: p.clone(), text: std::fs::read_to_string(p).expect("read corpus file") })
        .collect();

    let started = Instant::now();
    let diagnostics = lint(&files, &roots, &LintConfig::default());
    let elapsed = started.elapsed();

    let errors: Vec<String> = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{} {:?} [{}] {}", d.file.display(), d.range, d.rule, d.message))
        .collect();
    eprintln!(
        "{subdir}: {} files, {} projects, {} diagnostics, lint time {elapsed:?}",
        files.len(),
        roots.len() + 1,
        diagnostics.len()
    );
    for d in diagnostics.iter().filter(|d| d.severity != Severity::Error) {
        let rel = d.file.strip_prefix(&dir).unwrap_or(&d.file);
        eprintln!("  {:?} {} {:?} [{}] {}", d.severity, rel.display(), d.range, d.rule, d.message);
    }
    assert!(errors.is_empty(), "errors on the official examples:\n{}", errors.join("\n"));
    diagnostics
}

#[test]
fn official_examples_have_no_errors() {
    let diagnostics = lint_corpus("examples");
    // Warnings that are known to be genuine on the official examples.
    let rules: std::collections::BTreeSet<&str> = diagnostics.iter().map(|d| d.rule.as_str()).collect();
    for rule in rules {
        assert!(likec4_rules::rules().iter().any(|r| r.id == rule), "unregistered rule id {rule}");
    }
}

#[test]
fn formatted_examples_have_no_errors() {
    lint_corpus("examples-formatted");
}
