//! End-to-end tests for the `likec4-lint` binary.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_likec4-lint"))
}

/// `tests/corpus/examples` at the workspace root.
fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/examples")
}

#[test]
fn help_shows_top_level_description() {
    let output = bin().arg("--help").output().expect("failed to run likec4-lint --help");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Fast linter and formatter for the LikeC4 DSL"),
        "help output missing top-level description:\n{stdout}"
    );
    assert!(stdout.contains("lint"));
    assert!(stdout.contains("format"));
    assert!(stdout.contains("check"));
}

#[test]
fn nonexistent_path_exits_with_code_2() {
    let output = bin()
        .args(["lint", "/no/such/path/does-not-exist"])
        .output()
        .expect("failed to run likec4-lint lint");
    assert_eq!(output.status.code(), Some(2), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(!String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn stdin_passthrough() {
    // An empty document always parses successfully, regardless of how complete the
    // (stubbed) parser is, so this exercises the --stdin plumbing deterministically.
    let mut child = bin()
        .args(["format", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn likec4-lint format --stdin");

    child.stdin.take().unwrap().write_all(b"").unwrap();
    let output = child.wait_with_output().expect("failed to wait on likec4-lint format --stdin");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, b"");
}

#[test]
fn format_multiple_files_without_write_or_check_exits_with_code_2() {
    let output = bin()
        .args(["format", examples_dir().to_str().unwrap()])
        .output()
        .expect("failed to run likec4-lint format");
    assert_eq!(output.status.code(), Some(2), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--write") && stderr.contains("--check"), "unexpected stderr: {stderr}");
}

#[test]
fn lint_format_json_produces_parseable_json() {
    let output = bin()
        .args(["lint", examples_dir().to_str().unwrap(), "--format", "json"])
        .output()
        .expect("failed to run likec4-lint lint --format json");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!("stdout was not valid JSON ({e}):\n{stdout}");
    });

    assert!(value.get("diagnostics").is_some_and(|d| d.is_array()));
    let summary = value.get("summary").expect("missing summary");
    assert!(summary.get("errors").is_some());
    assert!(summary.get("warnings").is_some());
    assert!(summary.get("files").is_some());
}

#[test]
fn list_rules_succeeds() {
    let output =
        bin().args(["lint", "--list-rules"]).output().expect("failed to run likec4-lint lint --list-rules");
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(!output.stdout.is_empty());
}
