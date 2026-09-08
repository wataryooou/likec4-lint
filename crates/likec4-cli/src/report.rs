//! Rendering of diagnostics (pretty via `annotate-snippets`, and JSON) and small
//! shared helpers used by every subcommand.

use std::collections::HashMap;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use annotate_snippets::{AnnotationKind, Element, Level, Renderer, Snippet};
use clap::ValueEnum;
use likec4_lint::{Diagnostic, Level as RuleLevel, RuleInfo, Severity};
use serde::Serialize;
use text_size::{TextRange, TextSize};

/// Source text of every file that was read, keyed by path. Used to render snippets.
pub type SourceMap = HashMap<PathBuf, String>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Pretty,
    Json,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

/// Resolve whether ANSI colors should be used, honoring `--color` and `NO_COLOR`.
pub fn use_color(mode: ColorMode) -> bool {
    match mode {
        ColorMode::Always => true,
        ColorMode::Never => false,
        ColorMode::Auto => std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal(),
    }
}

/// Convert a byte offset into a 1-based `(line, column)` pair. The column counts
/// Unicode scalar values (chars), not bytes.
pub fn offset_to_line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let mut line = 1usize;
    let mut column = 1usize;
    for ch in text[..offset].chars() {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

/// Build a diagnostic for a file that could not be read from disk.
pub fn io_error_diagnostic(path: PathBuf, err: &std::io::Error) -> Diagnostic {
    Diagnostic {
        rule: "io-error".to_string(),
        severity: Severity::Error,
        message: format!("failed to read file: {err}"),
        file: path,
        range: TextRange::new(TextSize::from(0), TextSize::from(0)),
        help: None,
    }
}

/// Sort diagnostics by file, then by position, then by rule id (deterministic output).
pub fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| a.range.start().cmp(&b.range.start()))
            .then_with(|| a.rule.cmp(&b.rule))
    });
}

/// `true` when any diagnostic has `Severity::Error`.
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

fn count_by_severity(diagnostics: &[Diagnostic], severity: Severity) -> usize {
    diagnostics.iter().filter(|d| d.severity == severity).count()
}

/// `N error(s), M warning(s) in K file(s)`.
pub fn summary_line(diagnostics: &[Diagnostic], files_count: usize) -> String {
    let errors = count_by_severity(diagnostics, Severity::Error);
    let warnings = count_by_severity(diagnostics, Severity::Warning);
    format!("{errors} error(s), {warnings} warning(s) in {files_count} file(s)")
}

fn level_for(severity: Severity) -> Level<'static> {
    match severity {
        Severity::Error => Level::ERROR,
        Severity::Warning => Level::WARNING,
        Severity::Info => Level::INFO,
    }
}

/// Render diagnostics as pretty, human readable text using `annotate-snippets`.
/// Each diagnostic becomes its own report so unrelated files/positions stay separate.
pub fn render_pretty(diagnostics: &[Diagnostic], sources: &SourceMap, color: bool) -> String {
    let renderer = if color { Renderer::styled() } else { Renderer::plain() };
    let empty = String::new();
    let mut out = String::new();

    for diag in diagnostics {
        let source = sources.get(&diag.file).unwrap_or(&empty);
        let path = display_path(&diag.file);
        let start = u32::from(diag.range.start()) as usize;
        let end = u32::from(diag.range.end()) as usize;
        let (start, end) = if end <= source.len() { (start, end) } else { (0, 0) };

        let snippet = Snippet::source(source.as_str())
            .path(path.as_str())
            .annotation(AnnotationKind::Primary.span(start..end));

        let mut elements: Vec<Element<'_>> = vec![snippet.into()];
        if let Some(help) = &diag.help {
            elements.push(Level::HELP.message(help.as_str()).into());
        }

        let group = level_for(diag.severity)
            .primary_title(diag.message.as_str())
            .id(diag.rule.as_str())
            .elements(elements);

        out.push_str(&renderer.render(&[group]));
        out.push('\n');
    }

    out
}

#[derive(Serialize)]
struct JsonRange {
    start: u32,
    end: u32,
}

#[derive(Serialize)]
struct JsonDiagnostic<'a> {
    rule: &'a str,
    severity: Severity,
    message: &'a str,
    file: &'a Path,
    range: JsonRange,
    line: usize,
    column: usize,
    help: Option<&'a str>,
}

#[derive(Serialize)]
struct JsonSummary {
    errors: usize,
    warnings: usize,
    files: usize,
}

#[derive(Serialize)]
struct JsonReport<'a> {
    diagnostics: Vec<JsonDiagnostic<'a>>,
    summary: JsonSummary,
}

/// Render diagnostics as the JSON report described in `docs/DESIGN.md`.
pub fn render_json(diagnostics: &[Diagnostic], sources: &SourceMap, files_count: usize) -> String {
    let empty = String::new();
    let json_diagnostics: Vec<JsonDiagnostic<'_>> = diagnostics
        .iter()
        .map(|d| {
            let source = sources.get(&d.file).unwrap_or(&empty);
            let (line, column) = offset_to_line_col(source, u32::from(d.range.start()) as usize);
            JsonDiagnostic {
                rule: &d.rule,
                severity: d.severity,
                message: &d.message,
                file: &d.file,
                range: JsonRange { start: d.range.start().into(), end: d.range.end().into() },
                line,
                column,
                help: d.help.as_deref(),
            }
        })
        .collect();

    let report = JsonReport {
        summary: JsonSummary {
            errors: count_by_severity(diagnostics, Severity::Error),
            warnings: count_by_severity(diagnostics, Severity::Warning),
            files: files_count,
        },
        diagnostics: json_diagnostics,
    };

    serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
}

/// Print diagnostics (if any) and, unless `quiet`, a trailing summary line.
pub fn emit_diagnostics(
    diagnostics: &[Diagnostic],
    sources: &SourceMap,
    files_count: usize,
    format: OutputFormat,
    color: ColorMode,
    quiet: bool,
) {
    match format {
        OutputFormat::Pretty => {
            if !diagnostics.is_empty() {
                print!("{}", render_pretty(diagnostics, sources, use_color(color)));
            }
            if !quiet {
                println!("{}", summary_line(diagnostics, files_count));
            }
        }
        OutputFormat::Json => {
            println!("{}", render_json(diagnostics, sources, files_count));
        }
    }
}

fn level_str(level: RuleLevel) -> &'static str {
    match level {
        RuleLevel::Off => "off",
        RuleLevel::Info => "info",
        RuleLevel::Warning => "warning",
        RuleLevel::Error => "error",
    }
}

/// Print `likec4_lint::rules()` as a table (`--list-rules`).
pub fn print_rules_table(rules: &[RuleInfo]) {
    if rules.is_empty() {
        println!("no rules registered");
        return;
    }

    let id_width = rules.iter().map(|r| r.id.len()).max().unwrap_or(2).max("ID".len());
    let level_width = "warning".len().max("LEVEL".len());

    println!("{:<id_width$}  {:<level_width$}  DESCRIPTION", "ID", "LEVEL");
    for rule in rules {
        let level = level_str(rule.default_level);
        println!("{:<id_width$}  {level:<level_width$}  {}", rule.id, rule.description);
    }
}

/// Path as shown to the user: relative to the current directory when possible.
pub fn display_path(path: &Path) -> String {
    match std::env::current_dir() {
        Ok(cwd) => match path.strip_prefix(&cwd) {
            Ok(rel) if !rel.as_os_str().is_empty() => rel.display().to_string(),
            _ => path.display().to_string(),
        },
        Err(_) => path.display().to_string(),
    }
}

/// Keep only diagnostics for the requested `files` (plus file-less diagnostics such as
/// configuration problems); context files loaded from the same project are not reported.
pub fn retain_reported(diagnostics: &mut Vec<Diagnostic>, files: &[PathBuf]) {
    let requested: std::collections::HashSet<&Path> = files.iter().map(PathBuf::as_path).collect();
    diagnostics.retain(|d| d.file.as_os_str().is_empty() || requested.contains(d.file.as_path()));
}
