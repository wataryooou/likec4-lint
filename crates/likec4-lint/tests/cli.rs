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

/// A scratch project: a temporary directory used as the working directory of every command
/// run through `cmd()`. `new()` also drops an empty `likec4-lint.toml` into it. The home
/// directory of the child process is pinned to a hidden directory inside the project, so
/// nothing in the tests ever touches the developer's real home directory; it is not an
/// ancestor of the working directory, so the boundary rules under test see the project.
struct Project {
    dir: tempfile::TempDir,
}

impl Project {
    fn new() -> Self {
        let project = Self::without_config();
        project.write("likec4-lint.toml", "");
        project
    }

    fn without_config() -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        Project { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(rel)).unwrap()
    }

    fn cmd(&self) -> Command {
        self.cmd_in(self.dir.path())
    }

    /// Like `cmd()`, with the working directory set to `cwd` (inside the project).
    fn cmd_in(&self, cwd: &Path) -> Command {
        let mut cmd = bin();
        // The working directory the child sees is canonical (macOS resolves the temp dir
        // symlink), so the pinned home directory must be canonical too.
        let home = self.dir.path().canonicalize().unwrap().join(".home");
        std::fs::create_dir_all(&home).unwrap();
        cmd.current_dir(cwd).env("HOME", &home).env("USERPROFILE", &home);
        cmd
    }

    /// Run `args` and parse stdout as the JSON report.
    fn json(&self, args: &[&str]) -> (serde_json::Value, Option<i32>) {
        let output = self.cmd().args(args).output().unwrap();
        (json_report(&output), output.status.code())
    }
}

fn json_report(output: &std::process::Output) -> serde_json::Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "stdout was not valid JSON ({e}):\n{stdout}\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn rule_ids(report: &serde_json::Value) -> Vec<&str> {
    report["diagnostics"].as_array().unwrap().iter().map(|d| d["rule"].as_str().unwrap()).collect()
}

const SPEC: &str = "specification {\n  element system\n  tag t\n}\n";
const MODEL_USING_SPEC: &str = "model {\n  a = system {\n    #t\n  }\n  b = system\n  a -> b\n}\n";

fn run_with_stdin(mut cmd: Command, input: &str) -> std::process::Output {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn likec4-lint");
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    child.wait_with_output().expect("failed to wait on likec4-lint")
}

const UNFORMATTED: &str = "model{a=system}\n";
const FORMATTED: &str = "model {\n  a = system\n}\n";
/// A document that is both formatted and free of lint findings.
const CLEAN: &str = "specification {\n  element system\n}\nmodel {\n  a = system\n}\n";
/// `CLEAN` with one misindented line: lint-clean, but not formatted.
const CLEAN_UNFORMATTED: &str = "specification {\n  element system\n}\nmodel {\na = system\n}\n";
/// `CLEAN` indented with tabs.
const CLEAN_TABS: &str = "specification {\n\telement system\n}\nmodel {\n\ta = system\n}\n";

#[test]
fn format_check_and_write_are_mutually_exclusive() {
    let project = Project::new();
    project.write("model.c4", UNFORMATTED);
    let output = project.cmd().args(["format", "--check", "--write", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--check") && stderr.contains("--write"), "unexpected stderr: {stderr}");
    assert_eq!(std::fs::read_to_string(project.dir.path().join("model.c4")).unwrap(), UNFORMATTED);
}

#[test]
fn format_stdin_check_exits_1_for_unformatted_input_and_keeps_stdout_empty() {
    let project = Project::new();
    let mut cmd = project.cmd();
    cmd.args(["format", "--stdin", "--check"]);
    let output = run_with_stdin(cmd, UNFORMATTED);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty(), "stdout: {}", String::from_utf8_lossy(&output.stdout));
}

#[test]
fn format_stdin_check_exits_0_for_formatted_input() {
    let project = Project::new();
    let mut cmd = project.cmd();
    cmd.args(["format", "--stdin", "--check"]);
    let output = run_with_stdin(cmd, FORMATTED);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stdout.is_empty());
}

#[test]
fn format_stdin_check_diff_names_the_stdin_filepath() {
    let project = Project::new();
    let mut cmd = project.cmd();
    cmd.args(["format", "--stdin", "--check", "--diff", "--stdin-filepath", "src/model.c4"]);
    let output = run_with_stdin(cmd, UNFORMATTED);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--- src/model.c4 (original)"), "stdout: {stdout}");
    assert!(stdout.contains("+model {"), "stdout: {stdout}");
}

#[test]
fn format_stdin_rejects_write_and_paths() {
    let project = Project::new();
    project.write("model.c4", UNFORMATTED);
    for extra in [vec!["--write"], vec!["model.c4"]] {
        let mut cmd = project.cmd();
        cmd.args(["format", "--stdin"]).args(&extra);
        let output = run_with_stdin(cmd, UNFORMATTED);
        assert_eq!(
            output.status.code(),
            Some(2),
            "args {extra:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn lint_single_file_reports_duplicates_declared_in_project_context() {
    let project = Project::new();
    project.write("dup/likec4.config.json", "{}");
    project.write("dup/spec.c4", "specification {\n  element system\n}\n");
    project.write("dup/a.c4", "model {\n  a = system\n}\n");
    project.write("dup/zzz.c4", "model {\n  a = system\n}\n");
    for file in ["dup/zzz.c4", "dup/a.c4"] {
        let output = project.cmd().args(["lint", file, "--format", "json"]).output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let value: serde_json::Value = serde_json::from_str(&stdout).expect("json");
        let rules: Vec<&str> =
            value["diagnostics"].as_array().unwrap().iter().map(|d| d["rule"].as_str().unwrap()).collect();
        assert_eq!(rules, ["duplicate-element"], "linting {file}: {stdout}");
        assert_eq!(output.status.code(), Some(1));
    }
}

// E4: a file that belongs to no marker project is still linted together with its siblings.

#[test]
fn lint_single_file_loads_siblings_from_the_config_directory_as_context() {
    let project = Project::new();
    project.write("spec.c4", SPEC);
    project.write("model.c4", MODEL_USING_SPEC);
    let (report, code) = project.json(&["lint", "model.c4", "--json"]);
    assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{report:#}");
    assert_eq!(report["summary"]["files"], 1);
    assert_eq!(code, Some(0));
}

#[test]
fn lint_single_file_loads_siblings_from_the_git_root_as_context() {
    let project = Project::without_config();
    std::fs::create_dir(project.path().join(".git")).unwrap();
    project.write("spec/spec.c4", SPEC);
    project.write("src/model.c4", MODEL_USING_SPEC);
    let (report, code) = project.json(&["lint", "src/model.c4", "--json"]);
    assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{report:#}");
    assert_eq!(code, Some(0));
}

#[test]
fn lint_single_file_falls_back_to_the_working_directory_as_context_origin() {
    // No config, no `.git` (and the home directory is the project, so nothing above counts).
    let project = Project::without_config();
    project.write("spec/spec.c4", SPEC);
    project.write("src/model.c4", MODEL_USING_SPEC);
    let (report, code) = project.json(&["lint", "src/model.c4", "--json"]);
    assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{report:#}");
    assert_eq!(code, Some(0));
}

#[test]
fn lint_file_outside_the_working_directory_uses_its_parent_as_context_origin() {
    let project = Project::without_config();
    project.write("out/spec.c4", SPEC);
    project.write("out/model.c4", MODEL_USING_SPEC);
    project.write("in/.keep", "");
    let output = project
        .cmd_in(&project.path().join("in"))
        .args(["lint", "../out/model.c4", "--json"])
        .output()
        .unwrap();
    let report = json_report(&output);
    assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{report:#}");
    assert_eq!(output.status.code(), Some(0));
}

// E5: excluded files are silenced, not removed from the model.

#[test]
fn excluded_files_are_loaded_as_context_but_not_reported() {
    let project = Project::new();
    project.write("likec4-lint.toml", "[lint]\nexclude = [\"**/generated/**\"]\n");
    // The generated file declares the kinds and also carries a warning of its own.
    project.write("generated/spec.c4", "specification {\n  element system\n  tag t\n  tag unused\n}\n");
    project.write("model.c4", MODEL_USING_SPEC);
    for command in ["lint", "check"] {
        let (report, code) = project.json(&[command, ".", "--json"]);
        assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{command}: {report:#}");
        assert_eq!(report["summary"]["files"], 1, "{command}: {report:#}");
        assert_eq!(code, Some(0), "{command}");
    }
}

#[test]
fn format_check_skips_excluded_files_entirely() {
    let project = Project::new();
    project.write("likec4-lint.toml", "[lint]\nexclude = [\"**/generated/**\"]\n");
    project.write("generated/spec.c4", UNFORMATTED);
    project.write("model.c4", FORMATTED);
    let (report, code) = project.json(&["format", "--check", ".", "--json"]);
    assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{report:#}");
    assert_eq!(report["summary"]["files"], 1);
    assert_eq!(code, Some(0));
}

// E7: "needs formatting" is a diagnostic, so it shows up in JSON and drives the exit code.

#[test]
fn format_check_json_reports_needs_formatting_diagnostics() {
    let project = Project::new();
    project.write("model.c4", UNFORMATTED);
    let (report, code) = project.json(&["format", "--check", "model.c4", "--json"]);
    assert_eq!(code, Some(1));
    let diagnostics = report["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 1, "{report:#}");
    let d = &diagnostics[0];
    assert_eq!(d["rule"], "needs-formatting");
    assert_eq!(d["severity"], "error");
    assert_eq!(d["message"], "File is not formatted");
    assert_eq!(d["file"], "model.c4");
    assert_eq!(d["range"], serde_json::json!({ "start": 0, "end": 0 }));
    assert_eq!(d["help"], "run `likec4-lint format --write model.c4`");
    assert_eq!(report["summary"]["errors"], 1);
}

#[test]
fn check_json_reports_needs_formatting_diagnostics() {
    let project = Project::new();
    project.write("model.c4", "specification{element system}\n");
    let (report, code) = project.json(&["check", "model.c4", "--json"]);
    assert_eq!(code, Some(1));
    assert_eq!(rule_ids(&report), ["needs-formatting", "unused-element-kind"], "{report:#}");
    assert_eq!(report["summary"]["errors"], 1);
    assert_eq!(report["summary"]["warnings"], 1);
}

#[test]
fn format_check_json_is_printed_even_when_everything_is_formatted() {
    let project = Project::new();
    project.write("model.c4", CLEAN);
    for args in [vec!["format", "--check"], vec!["check"], vec!["lint"]] {
        let mut args = args;
        args.extend(["model.c4", "--json"]);
        let (report, code) = project.json(&args);
        assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{args:?}: {report:#}");
        assert_eq!(code, Some(0), "{args:?}");
    }
}

// W2: `--write` replaces files atomically and keeps going after a failure.

#[cfg(unix)]
#[test]
fn format_write_continues_past_a_file_it_cannot_replace() {
    use std::os::unix::fs::PermissionsExt;

    let project = Project::new();
    // Sorted first, so the failure happens before the file that must still be formatted.
    project.write("a-readonly/x.c4", CLEAN_UNFORMATTED);
    project.write("b.c4", CLEAN_UNFORMATTED);
    let readonly = project.path().join("a-readonly");
    std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o555)).unwrap();
    let (report, code) = project.json(&["format", "--write", "a-readonly/x.c4", "b.c4", "--json"]);
    // Restore before the temp dir is dropped, or its cleanup fails silently.
    std::fs::set_permissions(&readonly, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(code, Some(1));
    assert_eq!(rule_ids(&report), ["io-error"], "{report:#}");
    assert_eq!(report["diagnostics"][0]["file"], "a-readonly/x.c4");
    assert!(report["diagnostics"][0]["message"].as_str().unwrap().starts_with("failed to write file"));
    assert_eq!(project.read("a-readonly/x.c4"), CLEAN_UNFORMATTED);
    assert_eq!(project.read("b.c4"), CLEAN);
}

#[cfg(unix)]
#[test]
fn format_write_rewrites_the_target_of_a_symlink_and_keeps_the_link() {
    let project = Project::new();
    let target = project.write("real/model.c4", CLEAN_UNFORMATTED);
    let link = project.path().join("link.c4");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let output = project.cmd().args(["format", "--write", "link.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(project.read("real/model.c4"), CLEAN);
}

// W3: configuration mistakes are errors, with the offending key in the message.

#[test]
fn unknown_config_keys_are_rejected() {
    let project = Project::new();
    project.write("likec4-lint.toml", "[format]\nindent = 4\n");
    project.write("model.c4", CLEAN);
    let output = project.cmd().args(["lint", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error: failed to parse config file: likec4-lint.toml"), "{stderr}");
    assert!(stderr.contains("indent") && stderr.contains("line 2"), "{stderr}");
}

#[test]
fn unknown_rule_levels_are_rejected_with_a_readable_message() {
    let project = Project::new();
    project.write("likec4-lint.toml", "[lint.rules]\nunused-tag = \"warn\"\n");
    project.write("model.c4", CLEAN);
    let output = project.cmd().args(["lint", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown level 'warn'"), "{stderr}");
}

// W4: the upward search stops at a repository root, and the config in use is named.

#[test]
fn config_search_stops_at_a_git_repository_root() {
    let project = Project::without_config();
    // Invalid on purpose: adopting it is an exit 2.
    project.write("likec4-lint.toml", "[lint.rules]\nunused-tag = \"warn\"\n");
    project.write("repo/model.c4", CLEAN);
    let repo = project.path().join("repo");

    // Control: without `.git` the search reaches the parent directory.
    let output = project.cmd_in(&repo).args(["lint", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    std::fs::create_dir(repo.join(".git")).unwrap();
    let output = project.cmd_in(&repo).args(["lint", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("(config:"), "{stdout}");
}

#[test]
fn summary_names_the_config_file_in_use() {
    let project = Project::new();
    project.write("model.c4", CLEAN);
    let output = project.cmd().args(["lint", "model.c4"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "0 error(s), 0 warning(s), 0 info(s) in 1 file(s) (config: likec4-lint.toml)\n");
    let (report, _) = project.json(&["lint", "model.c4", "--json"]);
    assert_eq!(report["summary"]["config"], "likec4-lint.toml");

    let project = Project::without_config();
    project.write("model.c4", CLEAN);
    let output = project.cmd().args(["lint", "model.c4"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, "0 error(s), 0 warning(s), 0 info(s) in 1 file(s)\n");
    let (report, _) = project.json(&["lint", "model.c4", "--json"]);
    assert!(report["summary"]["config"].is_null(), "{report:#}");
}

// W5: `use_tabs` in the config, `indent_width` validated everywhere.

#[test]
fn use_tabs_in_the_config_applies_to_check_and_format() {
    let project = Project::new();
    project.write("likec4-lint.toml", "[format]\nuse_tabs = true\n");
    project.write("model.c4", CLEAN_TABS);
    for command in [vec!["check"], vec!["format", "--check"]] {
        let mut args = command.clone();
        args.extend(["model.c4", "--json"]);
        let (report, code) = project.json(&args);
        assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{command:?}: {report:#}");
        assert_eq!(code, Some(0), "{command:?}");
    }
    // The flag only turns tabs on; the config cannot be overridden back to spaces, and a
    // space-indented file is reported.
    project.write("spaces.c4", CLEAN);
    let (report, code) = project.json(&["format", "--check", "spaces.c4", "--json"]);
    assert_eq!(rule_ids(&report), ["needs-formatting"], "{report:#}");
    assert_eq!(code, Some(1));
}

#[test]
fn indent_width_out_of_range_is_a_usage_error() {
    let project = Project::new();
    project.write("model.c4", CLEAN);
    let output =
        project.cmd().args(["format", "--check", "--indent-width", "17", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("indent_width"));

    project.write("likec4-lint.toml", "[format]\nindent_width = 0\n");
    for command in [vec!["check"], vec!["format", "--check"]] {
        let mut args = command.clone();
        args.push("model.c4");
        let output = project.cmd().args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{command:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("indent_width"), "{command:?}");
    }
}

// W6: the JSON contract.

#[test]
fn json_report_follows_the_documented_contract() {
    let project = Project::new();
    project.write("likec4-lint.toml", "[lint.rules]\nnope = \"off\"\n");
    project.write("spec.c4", "specification {\n  element system\n}\n");
    project.write("a.c4", "model {\n  x = system\n}\n");
    project.write("b.c4", "model {\n  x = system\n}\n");
    let (report, code) = project.json(&["lint", "b.c4", "--json"]);
    assert_eq!(code, Some(1));
    assert_eq!(report["version"], 1);
    assert_eq!(rule_ids(&report), ["unknown-rule", "duplicate-element"], "{report:#}");

    let diagnostics = report["diagnostics"].as_array().unwrap();
    let mut keys = vec!["rule", "severity", "message", "file", "range", "line", "column", "help", "related"];
    keys.sort_unstable();
    for diagnostic in diagnostics {
        let object = diagnostic.as_object().unwrap();
        assert_eq!(object.keys().collect::<Vec<_>>(), keys, "{diagnostic:#}");
    }

    let unknown = &diagnostics[0];
    assert_eq!(unknown["severity"], "warning");
    assert!(unknown["file"].is_null(), "{unknown:#}");
    assert_eq!(unknown["range"], serde_json::json!({ "start": 0, "end": 0 }));
    assert_eq!(unknown["line"], 1);
    assert_eq!(unknown["column"], 1);
    assert!(unknown["related"].is_null());

    let duplicate = &diagnostics[1];
    assert_eq!(duplicate["severity"], "error");
    assert_eq!(duplicate["file"], "b.c4");
    assert_eq!(duplicate["line"], 2);
    assert_eq!(duplicate["column"], 3);
    assert!(duplicate["range"]["start"].as_u64().unwrap() < duplicate["range"]["end"].as_u64().unwrap());
    let related = &duplicate["related"];
    assert_eq!(related["file"], "a.c4");
    assert_eq!(related["message"], "first declared here");
    assert_eq!(related["line"], 2);
    assert_eq!(related["column"], 3);
    assert_eq!(related["range"], duplicate["range"]);

    let summary = report["summary"].as_object().unwrap();
    assert_eq!(summary.keys().collect::<Vec<_>>(), ["config", "errors", "files", "infos", "warnings"]);
    assert_eq!(summary["errors"], 1);
    assert_eq!(summary["warnings"], 1);
    assert_eq!(summary["infos"], 0);
    assert_eq!(summary["files"], 1);
    assert_eq!(summary["config"], "likec4-lint.toml");
}

// W7: `.gitignore` applies outside git repositories too.

#[test]
fn gitignore_is_respected_outside_git_repositories() {
    let project = Project::new();
    project.write(".gitignore", "ignored/\n");
    project.write("ignored/x.c4", "model {\n  a = ghost\n}\n");
    project.write("model.c4", CLEAN);
    let (report, code) = project.json(&["lint", ".", "--json"]);
    assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{report:#}");
    assert_eq!(report["summary"]["files"], 1);
    assert_eq!(code, Some(0));
}

// W8: a symlink and its target are the same document.

#[cfg(unix)]
#[test]
fn a_symlink_and_its_target_are_linted_once() {
    let project = Project::new();
    let target = project.write("model.c4", CLEAN);
    std::os::unix::fs::symlink(&target, project.path().join("link.c4")).unwrap();
    let (report, code) = project.json(&["lint", "link.c4", "model.c4", "--json"]);
    assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{report:#}");
    assert_eq!(report["summary"]["files"], 1);
    assert_eq!(code, Some(0));
}

// W9: control characters in file names never reach the terminal.

#[cfg(unix)]
#[test]
fn control_characters_in_file_names_are_escaped_in_pretty_output() {
    let project = Project::new();
    project.write("a\u{1b}[31m.c4", "model {\n  a = ghost\n}\n");
    let output = project.cmd().args(["lint", "a\u{1b}[31m.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a\\u{1b}[31m.c4"), "{stdout}");
    assert!(!stdout.contains('\u{1b}'), "{stdout}");
    // JSON carries the raw name (escaped by the JSON encoder, decoded by the consumer).
    let (report, _) = project.json(&["lint", "a\u{1b}[31m.c4", "--json"]);
    assert_eq!(report["diagnostics"][0]["file"], "a\u{1b}[31m.c4");
}

// N3, N5, N6, N10, N11.

#[test]
fn format_without_write_or_check_reports_when_no_documents_match() {
    let project = Project::new();
    project.write("empty/.keep", "");
    let output = project.cmd().args(["format", "empty"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no LikeC4 documents found under empty"), "{stderr}");
}

#[test]
fn json_flag_conflicts_with_an_explicit_format() {
    let project = Project::new();
    project.write("model.c4", CLEAN);
    let output = project.cmd().args(["lint", "model.c4", "--json", "--format", "pretty"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let (report, code) = project.json(&["lint", "model.c4", "--json"]);
    assert_eq!(code, Some(0));
    assert_eq!(report["version"], 1);
}

#[test]
fn quiet_suppresses_status_and_summary_lines_but_not_diagnostics() {
    let project = Project::new();
    project.write("model.c4", CLEAN_UNFORMATTED);
    for args in [vec!["format", "--check"], vec!["check"]] {
        let mut args = args;
        args.extend(["--quiet", "model.c4"]);
        let output = project.cmd().args(&args).output().unwrap();
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        // The status line and the summary are gone, but the finding itself is still reported.
        assert!(stdout.contains("error[needs-formatting]"), "{args:?}: {stdout}");
        assert!(!stdout.contains("needs formatting model.c4"), "{args:?}: {stdout}");
        assert!(!stdout.contains("error(s)"), "{args:?}: {stdout}");
    }
    let output = project.cmd().args(["format", "--write", "--quiet", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty(), "{}", String::from_utf8_lossy(&output.stdout));
    assert_eq!(project.read("model.c4"), CLEAN);

    project.write("bad.c4", "model {\n  a = ghost\n}\n");
    let output = project.cmd().args(["lint", "--quiet", "bad.c4"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("error[unknown-element-kind]"), "{stdout}");
    assert!(!stdout.contains("error(s)"), "{stdout}");
}

#[test]
fn info_diagnostics_are_counted_in_the_summary() {
    let project = Project::new();
    project.write("likec4-lint.toml", "[lint.rules]\nunused-tag = \"info\"\n");
    project
        .write("model.c4", "specification {\n  element system\n  tag unused\n}\nmodel {\n  a = system\n}\n");
    let output = project.cmd().args(["lint", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("info[unused-tag]"), "{stdout}");
    assert!(
        stdout.ends_with("0 error(s), 0 warning(s), 1 info(s) in 1 file(s) (config: likec4-lint.toml)\n"),
        "{stdout}"
    );
    let (report, _) = project.json(&["lint", "model.c4", "--json"]);
    assert_eq!(report["summary"]["infos"], 1);
}

#[test]
fn config_diagnostics_point_at_the_config_file_without_a_snippet() {
    let project = Project::new();
    project.write("likec4-lint.toml", "[lint.rules]\nnope = \"off\"\n");
    project.write("model.c4", CLEAN);
    let output = project.cmd().args(["lint", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("warning[unknown-rule]"), "{stdout}");
    assert!(stdout.contains("--> likec4-lint.toml\n"), "{stdout}");
    assert!(!stdout.contains(":1:1"), "{stdout}");
}

#[test]
fn a_directory_argument_delimits_its_own_context_unless_a_config_file_sits_above_it() {
    // A directory argument is a workspace of its own: siblings above it are not loaded even
    // inside a git repository (the CI smoke test relies on this for `tests/corpus`).
    let project = Project::without_config();
    std::fs::create_dir(project.path().join(".git")).unwrap();
    project.write("spec/spec.c4", SPEC);
    project.write("src/model.c4", MODEL_USING_SPEC);
    let (report, code) = project.json(&["lint", "src", "--json"]);
    assert_eq!(
        rule_ids(&report),
        ["unknown-element-kind", "unknown-tag", "unknown-element-kind"],
        "{report:#}"
    );
    assert_eq!(code, Some(1));
    // A config file above the directory marks the project root and widens the context.
    project.write("likec4-lint.toml", "");
    let (report, code) = project.json(&["lint", "src", "--json"]);
    assert_eq!(rule_ids(&report), Vec::<&str>::new(), "{report:#}");
    assert_eq!(code, Some(0));
}

#[test]
fn diagnostics_on_very_long_lines_show_a_window_around_the_range() {
    let steps: Vec<String> = (0..200).map(|i| format!("step{i}")).collect();
    let line = format!("    {}", steps.join(" -> "));
    assert!(line.len() > 1024);
    let project = Project::new();
    project.write("long.c4", &format!("views {{\n  dynamic view v {{\n{line}\n  }}\n}}\n"));
    let output = project.cmd().args(["lint", "long.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(" --> long.c4:3:5\n"), "{stdout}");
    assert!(stdout.contains("3 |     step0 -> step1 -> "), "{stdout}");
    let last_column = line.chars().count() - "step199".len() + 1;
    assert!(stdout.contains(&format!(" --> long.c4:3:{last_column}\n")), "{stdout}");
    assert!(stdout.contains("3 | ...") && stdout.contains("step199\n"), "{stdout}");
    let longest = stdout.lines().map(|l| l.len()).max().unwrap();
    assert!(longest < 400, "a rendered line is {longest} bytes long:\n{stdout}");
}

#[test]
fn format_write_never_produces_a_file_that_fails_format_check() {
    // The official formatter turns `'''say "hi"'''` into `"""say \"hi\""""`, which does not
    // parse; `likec4-lint` leaves such markdown strings alone (README, deliberate deviations).
    let project = Project::new();
    project.write(
        "model.c4",
        "specification {\n  element el\n}\nmodel {\n  a = el \"A\" {\n    description '''say \"hi\"'''\n  }\n}\n",
    );
    let write = project.cmd().args(["format", "--write", "model.c4"]).output().unwrap();
    assert_eq!(write.status.code(), Some(0), "stderr: {}", String::from_utf8_lossy(&write.stderr));
    let check = project.cmd().args(["format", "--check", "model.c4"]).output().unwrap();
    assert_eq!(check.status.code(), Some(0), "stdout: {}", String::from_utf8_lossy(&check.stdout));
    let text = std::fs::read_to_string(project.dir.path().join("model.c4")).unwrap();
    assert!(text.contains("'''say \"hi\"'''"), "markdown string was rewritten: {text}");
}

#[cfg(unix)]
#[test]
fn format_write_refuses_a_read_only_file() {
    use std::os::unix::fs::PermissionsExt;

    let project = Project::new();
    let path = project.write("model.c4", CLEAN_UNFORMATTED);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    if std::fs::OpenOptions::new().write(true).open(&path).is_ok() {
        return; // running as root: file permissions are not enforced
    }
    let output = project.cmd().args(["format", "--write", "model.c4"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("error[io-error]"), "{stdout}");
    assert!(!stdout.contains("formatted model.c4"), "{stdout}");
    assert_eq!(project.read("model.c4"), CLEAN_UNFORMATTED);
}

#[test]
fn the_home_directory_is_never_a_context_origin() {
    let project = Project::without_config();
    project.write("sub/a/spec.c4", "specification {\n  element system\n}\n");
    project.write("sub/b/model.c4", "model {\n  x = system\n}\n");
    let home = project.path().canonicalize().unwrap();
    // A config file, a project marker or a repository directly in `$HOME` must not make the
    // whole home directory the context of a file linted somewhere below it.
    for marker in ["likec4-lint.toml", ".likec4rc", ".git/HEAD"] {
        let path = project.write(marker, "");
        let output = project
            .cmd()
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .args(["lint", "sub/b/model.c4"])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("error[unknown-element-kind]"), "{marker}: {stdout}");
        std::fs::remove_file(&path).unwrap();
    }
}

#[test]
fn lint_rejects_invalid_format_settings_too() {
    let project = Project::new();
    project.write("model.c4", CLEAN);
    let cases = [
        ("[format]\nindent_width = 0\n", "indent_width"),
        ("[format]\nquote_style = \"bogus\"\n", "quote_style"),
    ];
    for (body, needle) in cases {
        project.write("likec4-lint.toml", body);
        let output = project.cmd().args(["lint", "model.c4"]).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{body}: {stderr}");
        assert!(stderr.contains(needle), "{body}: {stderr}");
    }
}
