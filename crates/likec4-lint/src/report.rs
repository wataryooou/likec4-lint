//! Rendering of diagnostics (pretty via `annotate-snippets`, JSON, and GitHub Actions
//! workflow commands) and small shared helpers used by every subcommand.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::IsTerminal;
use std::ops::Range;
use std::path::{Path, PathBuf};

use annotate_snippets::{AnnotationKind, Element, Group, Level, Origin, Renderer, Snippet};
use clap::ValueEnum;
use likec4_rules::{Diagnostic, Level as RuleLevel, RuleInfo, Severity, SourceFile};
use serde::Serialize;
use text_size::{TextRange, TextSize};

/// Rule id of the diagnostic emitted for a file that `format --check` / `check` would change.
pub const NEEDS_FORMATTING: &str = "needs-formatting";
/// Rule id of the diagnostic emitted when the formatter refuses its own output.
pub const FORMAT_ERROR: &str = "format-error";
/// Rule id of the diagnostic emitted when a file cannot be read or written.
pub const IO_ERROR: &str = "io-error";

/// Version of the JSON report schema (`"version"` in the output).
const JSON_VERSION: u32 = 1;

/// Source text of every file that was read, keyed by path, borrowed from the `SourceFile`s.
pub type SourceMap<'a> = HashMap<&'a Path, &'a str>;

/// Build the [`SourceMap`] of `files` without copying any text.
pub fn source_map(files: &[SourceFile]) -> SourceMap<'_> {
    files.iter().map(|f| (f.path.as_path(), f.text.as_str())).collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Pretty,
    Json,
    /// GitHub Actions workflow command annotations (`::error file=...,line=...::message`).
    Github,
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

/// Byte offsets of the line starts of one file. Built once per file, so that locating N
/// diagnostics costs O(N log lines) instead of one scan from the start of the file each.
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        LineIndex { line_starts }
    }

    /// 0-based line containing byte `offset`.
    fn line_of(&self, offset: usize) -> usize {
        // `line_starts[0] == 0`, so at least one start is `<= offset`.
        self.line_starts.partition_point(|&start| start <= offset) - 1
    }

    /// Byte offset where 0-based `line` starts.
    fn line_start(&self, line: usize) -> usize {
        self.line_starts[line]
    }

    /// Byte offset just past the end of 0-based `line` (after its newline, if any).
    fn line_end(&self, line: usize, text_len: usize) -> usize {
        self.line_starts.get(line + 1).copied().unwrap_or(text_len)
    }

    fn last_line(&self) -> usize {
        self.line_starts.len() - 1
    }

    /// 1-based `(line, column)` of byte `offset` in `text` (the text this index was built
    /// from). The column counts Unicode scalar values (chars), not bytes.
    pub fn line_col(&self, text: &str, offset: usize) -> (usize, usize) {
        let offset = offset.min(text.len());
        let line = self.line_of(offset);
        let column = text.get(self.line_start(line)..offset).map_or(0, |s| s.chars().count()) + 1;
        (line + 1, column)
    }
}

/// Number of unannotated lines shown around a diagnostic's snippet.
const SNIPPET_CONTEXT_LINES: usize = 2;
/// Lines longer than this are cut down to a window around the span before rendering:
/// `annotate-snippets` processes every character of a line it is given, which made each
/// diagnostic on a 1 MB single-line document cost milliseconds.
const MAX_LINE_BYTES: usize = 1024;
/// Bytes kept on each side of the span when a long line is cut.
const WINDOW_MARGIN_BYTES: usize = 80;
/// Marks a cut edge of a windowed line.
const CUT_MARKER: &str = "...";

/// The part of a file around one range, ready to be handed to `annotate-snippets`.
struct Excerpt<'a> {
    text: Cow<'a, str>,
    /// 1-based number of the first line of `text`.
    first_line: usize,
    /// The range, relative to `text`.
    span: Range<usize>,
    /// Set when `text` is a horizontal window of one long line. Such a snippet cannot carry
    /// the file path (the column it would print is relative to the window), so the
    /// `(line, column)` of the range start is rendered separately.
    location: Option<(usize, usize)>,
}

/// Cut the lines around `range` (plus [`SNIPPET_CONTEXT_LINES`] on each side) out of `text`.
/// Returns `None` for a file-level range (empty, at the start of the file): those
/// diagnostics are rendered without a snippet.
fn excerpt<'a>(text: &'a str, index: &LineIndex, range: TextRange) -> Option<Excerpt<'a>> {
    let start = usize::from(range.start()).min(text.len());
    let end = usize::from(range.end()).clamp(start, text.len());
    if start == 0 && end == 0 {
        return None;
    }
    let line = index.line_of(start);
    if index.line_end(line, text.len()) - index.line_start(line) > MAX_LINE_BYTES {
        return Some(window(text, index, line, start, end));
    }
    let first = line.saturating_sub(SNIPPET_CONTEXT_LINES);
    let last = (index.line_of(end) + SNIPPET_CONTEXT_LINES).min(index.last_line());
    let slice_start = index.line_start(first);
    let slice_end = index.line_end(last, text.len());
    let text = text.get(slice_start..slice_end)?;
    Some(Excerpt {
        text: Cow::Borrowed(text),
        first_line: first + 1,
        span: start - slice_start..end - slice_start,
        location: None,
    })
}

/// Cut 0-based `line` of `text` down to [`WINDOW_MARGIN_BYTES`] on each side of
/// `start..end` (a span running past the line is cut at the line end), marking cut edges.
fn window<'a>(text: &'a str, index: &LineIndex, line: usize, start: usize, end: usize) -> Excerpt<'a> {
    let line_start = index.line_start(line);
    let line_end = index.line_end(line, text.len());
    let end = end.min(line_end);
    let mut from = start.saturating_sub(WINDOW_MARGIN_BYTES).max(line_start);
    let mut to = end.saturating_add(WINDOW_MARGIN_BYTES).min(line_end);
    while !text.is_char_boundary(from) {
        from -= 1;
    }
    while !text.is_char_boundary(to) {
        to += 1;
    }

    let mut windowed = String::with_capacity(to - from + 2 * CUT_MARKER.len());
    if from > line_start {
        windowed.push_str(CUT_MARKER);
    }
    let offset = windowed.len();
    windowed.push_str(&text[from..to]);
    if to < line_end {
        windowed.push_str(CUT_MARKER);
    }

    let (_, column) = index.line_col(text, start);
    Excerpt {
        text: Cow::Owned(windowed),
        first_line: line + 1,
        span: start - from + offset..end - from + offset,
        location: Some((line + 1, column)),
    }
}

/// A snippet showing `ex`, annotated with `kind` (and `label`). With a `path` the snippet
/// renders its own ` --> path:line:col` header.
fn snippet<'a>(
    ex: Excerpt<'a>,
    path: Option<String>,
    kind: AnnotationKind,
    label: Option<&'a str>,
) -> Element<'a> {
    Snippet::source(ex.text)
        .line_start(ex.first_line)
        .path(path)
        .annotation(kind.span(ex.span).label(label))
        .into()
}

/// Insert a ` --> path:line:col` header after the title line of a rendered diagnostic whose
/// primary snippet is a windowed long line (see [`Excerpt::location`]). Such a snippet is
/// rendered without a path, because the column it would print is relative to the window.
/// `annotate-snippets` renders a lone header with a one-column gutter, so it is padded to
/// the width of the diagnostic's line number.
fn splice_header(
    renderer: &Renderer,
    rendered: String,
    level: Level<'static>,
    path: String,
    line: usize,
    column: usize,
) -> String {
    let origin = Origin::path(path).line(line).char_column(column);
    let header = renderer.render(&[Group::with_level(level).element(origin)]);
    let gutter = line.to_string().len();
    match rendered.split_once('\n') {
        Some((title, rest)) => format!("{title}\n{}{header}\n{rest}", " ".repeat(gutter - 1)),
        None => rendered,
    }
}

/// Build a diagnostic that concerns a whole file rather than a range in it (`io-error`,
/// `format-error`, `needs-formatting`): severity error, empty range at the start of the file.
pub fn file_error(rule: &str, path: PathBuf, message: String, help: Option<String>) -> Diagnostic {
    Diagnostic {
        rule: rule.to_string(),
        severity: Severity::Error,
        message,
        file: path,
        range: TextRange::new(TextSize::from(0), TextSize::from(0)),
        help,
        related: None,
    }
}

/// Build a diagnostic for a file that could not be read from disk.
pub fn io_error_diagnostic(path: PathBuf, message: String) -> Diagnostic {
    file_error(IO_ERROR, path, message, None)
}

/// Build the diagnostic for a file that `--check` found unformatted.
pub fn needs_formatting_diagnostic(path: PathBuf) -> Diagnostic {
    let help = format!("run `likec4-lint format --write {}`", display_path(&path));
    file_error(NEEDS_FORMATTING, path, "File is not formatted".to_string(), Some(help))
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

/// `true` when any diagnostic has `Severity::Error`. This is the only thing that decides
/// between exit codes 0 and 1.
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

fn count_by_severity(diagnostics: &[Diagnostic], severity: Severity) -> usize {
    diagnostics.iter().filter(|d| d.severity == severity).count()
}

/// Everything a subcommand hands to the reporter.
pub struct Report<'a> {
    /// Sorted diagnostics to render.
    pub diagnostics: &'a [Diagnostic],
    /// Text of every file that was read (requested and context), for snippets and positions.
    pub sources: &'a SourceMap<'a>,
    /// Number of files that diagnostics were collected for.
    pub files: usize,
    /// The config file in use, if one was found or given.
    pub config_path: Option<&'a Path>,
}

/// `N error(s), M warning(s), K info(s) in F file(s)`, plus ` (config: <path>)` when a
/// config file is in use.
pub fn summary_line(report: &Report<'_>) -> String {
    let errors = count_by_severity(report.diagnostics, Severity::Error);
    let warnings = count_by_severity(report.diagnostics, Severity::Warning);
    let infos = count_by_severity(report.diagnostics, Severity::Info);
    let mut line =
        format!("{errors} error(s), {warnings} warning(s), {infos} info(s) in {} file(s)", report.files);
    if let Some(config) = report.config_path {
        line.push_str(&format!(" (config: {})", display_path(config)));
    }
    line
}

fn level_for(severity: Severity) -> Level<'static> {
    match severity {
        Severity::Error => Level::ERROR,
        Severity::Warning => Level::WARNING,
        Severity::Info => Level::INFO,
    }
}

/// Line indexes of every file some diagnostic (or its related location) points into.
fn line_indexes<'a>(report: &Report<'a>) -> HashMap<&'a Path, LineIndex> {
    let mut indexes = HashMap::new();
    let files = report
        .diagnostics
        .iter()
        .flat_map(|d| std::iter::once(d.file.as_path()).chain(d.related.iter().map(|r| r.file.as_path())));
    for file in files {
        if let Some(text) = report.sources.get(file) {
            indexes.entry(file).or_insert_with(|| LineIndex::new(text));
        }
    }
    indexes
}

/// Render diagnostics as pretty, human readable text using `annotate-snippets`.
/// Each diagnostic becomes its own report so unrelated files/positions stay separate.
///
/// `needs-formatting` diagnostics are rendered only when `status_diagnostics` is set: the
/// `needs formatting <path>` status line printed by the formatting commands already carries
/// that information, unless those lines are suppressed (`--quiet`).
pub fn render_pretty(report: &Report<'_>, color: bool, status_diagnostics: bool) -> String {
    let renderer = if color { Renderer::styled() } else { Renderer::plain() };
    let cwd = std::env::current_dir().ok();
    let indexes = line_indexes(report);
    let mut out = String::new();

    let snippet_for = |file: &Path, range: TextRange| -> Option<Excerpt<'_>> {
        let text = report.sources.get(file)?;
        excerpt(text, indexes.get(file)?, range)
    };
    let position = |file: &Path, range: TextRange| -> Option<(usize, usize)> {
        let text = report.sources.get(file)?;
        Some(indexes.get(file)?.line_col(text, usize::from(range.start())))
    };

    for diag in report.diagnostics {
        if diag.rule == NEEDS_FORMATTING && !status_diagnostics {
            continue;
        }
        let level = level_for(diag.severity);
        let mut elements: Vec<Element<'_>> = Vec::new();
        // Header of a windowed primary snippet, spliced in after rendering.
        let mut header = None;

        if diag.file.as_os_str().is_empty() {
            // Configuration problem: point at the config file, there is no source to show.
            if let Some(config) = report.config_path {
                elements.push(Origin::path(display_path_in(config, cwd.as_deref())).into());
            }
        } else {
            let path = display_path_in(&diag.file, cwd.as_deref());
            match snippet_for(&diag.file, diag.range) {
                Some(ex) => match ex.location {
                    Some((line, column)) => {
                        elements.push(snippet(ex, None, AnnotationKind::Primary, None));
                        header = Some((path, line, column));
                    }
                    None => elements.push(snippet(ex, Some(path), AnnotationKind::Primary, None)),
                },
                None => elements.push(Origin::path(path).into()),
            }
        }

        if let Some(related) = &diag.related {
            let path = display_path_in(&related.file, cwd.as_deref());
            match snippet_for(&related.file, related.range) {
                // The usual second snippet, unless a windowed long line is involved on either
                // side: then the location is given as a note, which keeps the spliced header
                // aligned with a gutter that only the primary line dictates.
                Some(ex) if ex.location.is_none() && header.is_none() => {
                    let label = Some(related.message.as_str());
                    elements.push(snippet(ex, Some(path), AnnotationKind::Context, label));
                }
                _ => {
                    elements.push(Level::NOTE.message(related.message.as_str()).into());
                    let mut origin = Origin::path(path);
                    if let Some((line, column)) = position(&related.file, related.range) {
                        origin = origin.line(line).char_column(column);
                    }
                    elements.push(origin.into());
                }
            }
        }

        if let Some(help) = &diag.help {
            elements.push(Level::HELP.message(help.as_str()).into());
        }

        let group =
            level.clone().primary_title(diag.message.as_str()).id(diag.rule.as_str()).elements(elements);
        let mut rendered = renderer.render(&[group]);
        if let Some((path, line, column)) = header {
            rendered = splice_header(&renderer, rendered, level, path, line, column);
        }
        out.push_str(&rendered);
        out.push('\n');
    }

    out
}

#[derive(Serialize)]
struct JsonRange {
    start: u32,
    end: u32,
}

impl From<TextRange> for JsonRange {
    fn from(range: TextRange) -> Self {
        JsonRange { start: range.start().into(), end: range.end().into() }
    }
}

#[derive(Serialize)]
struct JsonRelated<'a> {
    file: String,
    range: JsonRange,
    line: usize,
    column: usize,
    message: &'a str,
}

#[derive(Serialize)]
struct JsonDiagnostic<'a> {
    rule: &'a str,
    severity: Severity,
    message: &'a str,
    /// Display path (relative to the working directory); `null` for configuration diagnostics.
    file: Option<String>,
    range: JsonRange,
    line: usize,
    column: usize,
    help: Option<&'a str>,
    related: Option<JsonRelated<'a>>,
}

#[derive(Serialize)]
struct JsonSummary {
    errors: usize,
    warnings: usize,
    infos: usize,
    files: usize,
    config: Option<String>,
}

#[derive(Serialize)]
struct JsonReport<'a> {
    version: u32,
    diagnostics: Vec<JsonDiagnostic<'a>>,
    summary: JsonSummary,
}

/// Render diagnostics as the JSON report described in `docs/DESIGN.md`. Paths are the
/// same relative paths as in the pretty output, unescaped (serde escapes them).
pub fn render_json(report: &Report<'_>) -> String {
    let cwd = std::env::current_dir().ok();
    let indexes = line_indexes(report);
    let position = |file: &Path, range: TextRange| -> (usize, usize) {
        match (report.sources.get(file), indexes.get(file)) {
            (Some(text), Some(index)) => index.line_col(text, usize::from(range.start())),
            _ => (1, 1),
        }
    };
    let json_path = |path: &Path| relative_path_in(path, cwd.as_deref()).to_string_lossy().into_owned();

    let diagnostics: Vec<JsonDiagnostic<'_>> = report
        .diagnostics
        .iter()
        .map(|d| {
            let (line, column) = position(&d.file, d.range);
            let related = d.related.as_ref().map(|r| {
                let (line, column) = position(&r.file, r.range);
                JsonRelated {
                    file: json_path(&r.file),
                    range: r.range.into(),
                    line,
                    column,
                    message: &r.message,
                }
            });
            JsonDiagnostic {
                rule: &d.rule,
                severity: d.severity,
                message: &d.message,
                file: if d.file.as_os_str().is_empty() { None } else { Some(json_path(&d.file)) },
                range: d.range.into(),
                line,
                column,
                help: d.help.as_deref(),
                related,
            }
        })
        .collect();

    let json = JsonReport {
        version: JSON_VERSION,
        diagnostics,
        summary: JsonSummary {
            errors: count_by_severity(report.diagnostics, Severity::Error),
            warnings: count_by_severity(report.diagnostics, Severity::Warning),
            infos: count_by_severity(report.diagnostics, Severity::Info),
            files: report.files,
            config: report.config_path.map(json_path),
        },
    };

    serde_json::to_string_pretty(&json).expect("the JSON report only contains serializable types")
}

/// Escape `text` for a GitHub Actions workflow command
/// (`::error file=...,line=...::message`). `property` additionally escapes `:` and `,`,
/// which only matter inside a `key=value` property, not inside the trailing message. See
/// <https://docs.github.com/actions/using-workflows/workflow-commands-for-github-actions>.
fn escape_github(text: &str, property: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '%' => out.push_str("%25"),
            '\r' => out.push_str("%0D"),
            '\n' => out.push_str("%0A"),
            ':' if property => out.push_str("%3A"),
            ',' if property => out.push_str("%2C"),
            _ => out.push(c),
        }
    }
    out
}

/// The workflow command for `severity` (`error`/`warning`/`notice`).
fn github_command(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "notice",
    }
}

/// Path for a `file=` property: [`relative_path_in`] with `\` normalized to `/`, so the
/// annotation lines up with the PR diff on Windows too.
fn github_path_in(path: &Path, cwd: Option<&Path>) -> String {
    relative_path_in(path, cwd).to_string_lossy().replace('\\', "/")
}

/// Render diagnostics as GitHub Actions workflow commands, one per line:
/// `::error file=...,line=...,col=...,endLine=...,endColumn=...,title=<rule>::<message>`.
/// `message` is the diagnostic message, with `help` appended as ` (help)` and `related` as
/// `; <related message>: <path>:<line>:<col>`. A diagnostic with no `file` (a configuration
/// problem) is rendered without the location properties: `::warning title=<rule>::<message>`.
pub fn render_github(report: &Report<'_>) -> String {
    let cwd = std::env::current_dir().ok();
    let indexes = line_indexes(report);
    let position = |file: &Path, range: TextRange| -> (usize, usize, usize, usize) {
        match (report.sources.get(file), indexes.get(file)) {
            (Some(text), Some(index)) => {
                let (line, col) = index.line_col(text, usize::from(range.start()));
                let (end_line, end_col) = index.line_col(text, usize::from(range.end()));
                (line, col, end_line, end_col)
            }
            _ => (1, 1, 1, 1),
        }
    };

    let mut out = String::new();
    for diag in report.diagnostics {
        let command = github_command(diag.severity);
        let title = escape_github(&diag.rule, true);

        let mut message = diag.message.clone();
        if let Some(help) = &diag.help {
            message = format!("{message} ({help})");
        }
        if let Some(related) = &diag.related {
            let (line, col, ..) = position(&related.file, related.range);
            let path = github_path_in(&related.file, cwd.as_deref());
            let related_message = &related.message;
            message = format!("{message}; {related_message}: {path}:{line}:{col}");
        }
        let message = escape_github(&message, false);

        if diag.file.as_os_str().is_empty() {
            out.push_str(&format!("::{command} title={title}::{message}\n"));
        } else {
            let path = escape_github(&github_path_in(&diag.file, cwd.as_deref()), true);
            let (line, col, end_line, end_col) = position(&diag.file, diag.range);
            out.push_str(&format!(
                "::{command} file={path},line={line},col={col},endLine={end_line},endColumn={end_col},title={title}::{message}\n"
            ));
        }
    }
    out
}

/// Print the report: pretty or GitHub Actions diagnostics (if any) followed by a summary
/// line unless `quiet`, or the JSON report (always, even with no diagnostics).
pub fn emit(report: &Report<'_>, format: OutputFormat, color: ColorMode, quiet: bool) {
    match format {
        OutputFormat::Pretty => {
            if !report.diagnostics.is_empty() {
                print!("{}", render_pretty(report, use_color(color), quiet));
            }
            if !quiet {
                println!("{}", summary_line(report));
            }
        }
        OutputFormat::Json => {
            println!("{}", render_json(report));
        }
        OutputFormat::Github => {
            if !report.diagnostics.is_empty() {
                print!("{}", render_github(report));
            }
            if !quiet {
                println!("{}", summary_line(report));
            }
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

/// Print `likec4_rules::rules()` as a table (`--list-rules`).
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

/// Path as reported to the user: relative to the working directory `cwd` when it lies
/// below it, unchanged otherwise. Not escaped; this is the form used in the JSON report.
fn relative_path_in(path: &Path, cwd: Option<&Path>) -> PathBuf {
    match cwd.and_then(|cwd| path.strip_prefix(cwd).ok()) {
        Some(rel) if !rel.as_os_str().is_empty() => rel.to_path_buf(),
        _ => path.to_path_buf(),
    }
}

/// Path as shown in terminal output: [`relative_path`] with control characters escaped, so
/// that a crafted file name cannot smuggle escape sequences into the terminal.
pub fn display_path(path: &Path) -> String {
    display_path_in(path, std::env::current_dir().ok().as_deref())
}

fn display_path_in(path: &Path, cwd: Option<&Path>) -> String {
    escape_control(&relative_path_in(path, cwd).to_string_lossy())
}

/// Replace every control character (C0, DEL and C1, including tab and newline) with its
/// `\u{XX}` escape.
pub fn escape_control(text: &str) -> String {
    escape_control_except(text, &[])
}

/// Like [`escape_control`], but keeps `keep` (for `--diff` bodies: newlines and tabs).
pub fn escape_control_except(text: &str, keep: &[char]) -> String {
    let needs_escape = |c: char| c.is_control() && !keep.contains(&c);
    if !text.chars().any(needs_escape) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if needs_escape(c) {
            out.push_str(&format!("\\u{{{:02x}}}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// Keep only diagnostics for the requested `files` (plus file-less diagnostics such as
/// configuration problems); context files loaded from the same project are not reported.
pub fn retain_reported(diagnostics: &mut Vec<Diagnostic>, files: &[PathBuf]) {
    let requested: std::collections::HashSet<&Path> = files.iter().map(PathBuf::as_path).collect();
    diagnostics.retain(|d| d.file.as_os_str().is_empty() || requested.contains(d.file.as_path()));
}

#[cfg(test)]
mod tests {
    use likec4_rules::RelatedLocation;

    use super::*;

    #[test]
    fn github_escapes_percent_cr_lf_in_the_message_and_colon_comma_in_properties() {
        // Two files, each with the interesting span on line 2 so the positions aren't (1, 1)
        // by accident.
        let main_text = "line1\nline2 target\nline3\n";
        let related_text = "rel1\nrel2 here\n";
        let sources: SourceMap<'_> =
            [(Path::new("/x/a:b,c.c4"), main_text), (Path::new("/x/other.c4"), related_text)]
                .into_iter()
                .collect();

        let diagnostics = vec![Diagnostic {
            rule: "test-rule".to_string(),
            severity: Severity::Warning,
            message: "have: 5%, more\nok".to_string(),
            file: PathBuf::from("/x/a:b,c.c4"),
            range: TextRange::new(TextSize::from(12), TextSize::from(18)), // "target"
            help: Some("retry: use --fix, ok".to_string()),
            related: Some(RelatedLocation {
                file: PathBuf::from("/x/other.c4"),
                range: TextRange::new(TextSize::from(10), TextSize::from(14)), // "here"
                message: "first here".to_string(),
            }),
        }];

        let report = Report { diagnostics: &diagnostics, sources: &sources, files: 1, config_path: None };

        assert_eq!(
            render_github(&report),
            "::warning file=/x/a%3Ab%2Cc.c4,line=2,col=7,endLine=2,endColumn=13,title=test-rule\
             ::have: 5%25, more%0Aok (retry: use --fix, ok); first here: /x/other.c4:2:6\n"
        );
    }

    #[test]
    fn github_renders_a_fileless_diagnostic_without_location_properties() {
        let diagnostics = vec![Diagnostic {
            rule: "unknown-rule".to_string(),
            severity: Severity::Warning,
            message: "Unknown rule 'nope'".to_string(),
            file: PathBuf::new(),
            range: TextRange::new(TextSize::from(0), TextSize::from(0)),
            help: None,
            related: None,
        }];
        let sources: SourceMap<'_> = HashMap::new();
        let report = Report { diagnostics: &diagnostics, sources: &sources, files: 1, config_path: None };

        assert_eq!(render_github(&report), "::warning title=unknown-rule::Unknown rule 'nope'\n");
    }

    #[test]
    fn github_path_normalizes_backslashes_to_forward_slashes() {
        assert_eq!(github_path_in(Path::new("/work/a\\b.c4"), Some(Path::new("/work"))), "a/b.c4");
    }

    #[test]
    fn line_index_locates_offsets_with_char_columns() {
        let text = "ab\nc\u{e9}d\n\nx";
        let index = LineIndex::new(text);
        assert_eq!(index.line_col(text, 0), (1, 1));
        assert_eq!(index.line_col(text, 2), (1, 3));
        assert_eq!(index.line_col(text, 3), (2, 1));
        // `é` is two bytes but one column.
        assert_eq!(index.line_col(text, 6), (2, 3));
        // The newline itself still belongs to its line.
        assert_eq!(index.line_col(text, 7), (2, 4));
        assert_eq!(index.line_col(text, 8), (3, 1));
        assert_eq!(index.line_col(text, 9), (4, 1));
        // Past the end: clamped to the end of the text.
        assert_eq!(index.line_col(text, 999), (4, 2));
    }

    #[test]
    fn excerpt_keeps_two_lines_of_context_and_rebases_the_span() {
        let text = "l1\nl2\nl3\nl4\nl5\nl6\nl7\n";
        let index = LineIndex::new(text);
        let range = TextRange::new(TextSize::from(12), TextSize::from(14)); // "l5"
        let ex = excerpt(text, &index, range).unwrap();
        assert_eq!(ex.text, "l3\nl4\nl5\nl6\nl7\n");
        assert_eq!(ex.first_line, 3);
        assert_eq!(&ex.text[ex.span.clone()], "l5");
        let ex = excerpt(text, &index, TextRange::new(TextSize::from(0), TextSize::from(2))).unwrap();
        assert_eq!(ex.text, "l1\nl2\nl3\n");
        assert_eq!(ex.first_line, 1);
        assert!(excerpt(text, &index, TextRange::new(TextSize::from(0), TextSize::from(0))).is_none());
    }

    #[test]
    fn excerpt_windows_over_long_lines() {
        let text = format!("l1\n{}X{}\nl3\n", "a".repeat(2000), "b".repeat(2000));
        let index = LineIndex::new(&text);
        let x = 3 + 2000;
        let range = TextRange::new(TextSize::from(x as u32), TextSize::from(x as u32 + 1));
        let ex = excerpt(&text, &index, range).unwrap();
        assert_eq!(ex.first_line, 2);
        assert_eq!(ex.location, Some((2, 2001)));
        assert!(ex.text.starts_with("...") && ex.text.ends_with("..."), "{}", ex.text);
        assert_eq!(ex.text.len(), 3 + 80 + 1 + 80 + 3);
        assert_eq!(&ex.text[ex.span.clone()], "X");
        // A span near the start of the line is not cut on the left.
        let ex = excerpt(&text, &index, TextRange::new(TextSize::from(3), TextSize::from(4))).unwrap();
        assert!(ex.text.starts_with("aaa") && ex.text.ends_with("..."), "{}", ex.text);
        assert_eq!(ex.location, Some((2, 1)));
        // Short lines keep the ordinary excerpt.
        let ex = excerpt(&text, &index, TextRange::new(TextSize::from(0), TextSize::from(2))).unwrap();
        assert_eq!(ex.location, None);
    }

    #[test]
    fn control_characters_in_paths_are_escaped() {
        let path = Path::new("a\u{1b}[31mb\tc\nd\u{7f}e\u{85}f.c4");
        assert_eq!(display_path_in(path, None), "a\\u{1b}[31mb\\u{09}c\\u{0a}d\\u{7f}e\\u{85}f.c4");
        assert_eq!(display_path_in(Path::new("plain.c4"), None), "plain.c4");
        assert_eq!(escape_control_except("a\tb\n\u{1b}c\r\n", &['\n', '\t']), "a\tb\n\\u{1b}c\\u{0d}\n");
    }

    #[test]
    fn relative_path_is_stripped_of_the_working_directory_only() {
        let cwd = Path::new("/work");
        assert_eq!(relative_path_in(Path::new("/work/a/b.c4"), Some(cwd)), PathBuf::from("a/b.c4"));
        assert_eq!(relative_path_in(Path::new("/other/b.c4"), Some(cwd)), PathBuf::from("/other/b.c4"));
        assert_eq!(relative_path_in(Path::new("/work"), Some(cwd)), PathBuf::from("/work"));
    }
}
