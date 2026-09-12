//! `likec4-lint format` implementation.

use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use likec4_fmt::{FormatError, FormatOptions, QuoteStyle};
use likec4_rules::{Diagnostic, Severity};
use likec4_syntax::SyntaxError;

use crate::cli::FormatArgs;
use crate::commands::read_files;
use crate::report::{self, LineIndex, OutputFormat};
use crate::{config, discover};

/// Outcome of formatting a single file without writing anything to disk.
pub enum CheckOutcome {
    Unchanged,
    NeedsFormatting(String),
    /// The input has syntax errors and was left alone.
    Skipped(Vec<SyntaxError>),
    /// The formatter's own output would not parse; nothing may be written.
    Unstable(FormatError),
}

/// Resolve effective `FormatOptions`, applying CLI overrides > config file > defaults.
/// `use_tabs` only turns tabs on; `false` leaves the config value in charge.
pub fn resolve_format_options(
    quote_style: Option<&str>,
    indent_width: Option<usize>,
    use_tabs: bool,
    resolved: &config::ResolvedConfig,
) -> Result<FormatOptions> {
    let quote_style = match quote_style.or(resolved.format.quote_style.as_deref()) {
        Some(s) => s.parse::<QuoteStyle>().map_err(|e| anyhow::anyhow!(e))?,
        None => QuoteStyle::default(),
    };
    let indent_width = indent_width.or(resolved.format.indent_width).unwrap_or(2);
    config::check_indent_width(indent_width).map_err(|e| anyhow::anyhow!(e))?;
    let use_tabs = use_tabs || resolved.format.use_tabs.unwrap_or(false);
    Ok(FormatOptions { indent_width, insert_spaces: !use_tabs, quote_style })
}

/// Classify the result of formatting `text`.
pub fn classify(result: Result<String, FormatError>, text: &str) -> CheckOutcome {
    match result {
        Ok(formatted) if formatted == text => CheckOutcome::Unchanged,
        Ok(formatted) => CheckOutcome::NeedsFormatting(formatted),
        Err(FormatError::SyntaxErrors(errors)) => CheckOutcome::Skipped(errors),
        Err(err @ FormatError::Unstable { .. }) => CheckOutcome::Unstable(err),
    }
}

/// Format `text` and classify the result without touching the filesystem.
pub fn check_file(text: &str, options: &FormatOptions) -> CheckOutcome {
    classify(likec4_fmt::format(text, options), text)
}

/// The `format-error` diagnostic for a file whose formatted output would be invalid.
pub fn format_error_diagnostic(path: PathBuf, err: &FormatError) -> Diagnostic {
    report::file_error(report::FORMAT_ERROR, path, err.to_string(), None)
}

fn syntax_error_diagnostics<'a>(
    path: &'a Path,
    errors: &'a [SyntaxError],
) -> impl Iterator<Item = Diagnostic> + 'a {
    errors.iter().map(move |err| Diagnostic {
        rule: "syntax-error".to_string(),
        severity: Severity::Error,
        message: err.message.clone(),
        file: path.to_path_buf(),
        range: err.range,
        help: None,
        related: None,
    })
}

/// Replace the contents of `path` without ever leaving it truncated: the new text goes to a
/// temporary file in the same directory, which is then renamed over the target. A symlink is
/// resolved first, so the link stays in place and the file it points to is rewritten. The
/// target's permissions are copied onto the new file.
fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    let target = fs::canonicalize(path)?;
    // The rename below only needs directory permissions; open the target for writing first
    // so that a read-only file is refused (permission denied) instead of silently replaced.
    OpenOptions::new().write(true).open(&target)?;
    let dir = target.parent().ok_or_else(|| io::Error::other("path has no parent directory"))?;
    let name = target.file_name().ok_or_else(|| io::Error::other("path has no file name"))?;
    let permissions = fs::metadata(&target)?.permissions();
    let (temp_path, temp) = create_temp_file(dir, name)?;
    let result = write_and_replace(temp, &temp_path, &target, contents, permissions);
    if result.is_err() {
        // Best effort: do not leave the temporary file behind.
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn write_and_replace(
    mut temp: fs::File,
    temp_path: &Path,
    target: &Path,
    contents: &str,
    permissions: fs::Permissions,
) -> io::Result<()> {
    temp.write_all(contents.as_bytes())?;
    // The handle must be closed before the rename on Windows.
    drop(temp);
    fs::set_permissions(temp_path, permissions)?;
    fs::rename(temp_path, target)
}

/// Create a new, hidden temporary file next to `name` in `dir` (dot-prefixed so that a
/// leftover from a crash is never picked up as a document).
fn create_temp_file(dir: &Path, name: &OsStr) -> io::Result<(PathBuf, fs::File)> {
    let pid = std::process::id();
    for attempt in 0..64u32 {
        let mut temp_name = OsString::from(".");
        temp_name.push(name);
        temp_name.push(format!(".likec4-lint.{pid}.{attempt}.tmp"));
        let candidate = dir.join(temp_name);
        match OpenOptions::new().write(true).create_new(true).open(&candidate) {
            Ok(file) => return Ok((candidate, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "could not create a temporary file"))
}

/// Print a unified diff between `old` and `new`, labelled with the (already escaped)
/// display `name`. Control characters in the body are escaped too; newlines and tabs stay.
fn print_diff(name: &str, old: &str, new: &str) {
    let old_label = format!("{name} (original)");
    let new_label = format!("{name} (formatted)");
    let diff = similar::TextDiff::from_lines(old, new);
    let mut unified = diff.unified_diff();
    unified.header(&old_label, &new_label);
    print!("{}", report::escape_control_except(&unified.to_string(), &['\n', '\t']));
}

/// `format --stdin`: format one document from stdin. Without `--check` the formatted
/// text goes to stdout; with `--check` nothing is printed to stdout except an optional
/// `--diff`, and the exit code says whether the input was formatted (0) or not (1).
fn run_stdin(args: &FormatArgs, options: &FormatOptions) -> Result<u8> {
    use std::io::Read;

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).context("failed to read stdin")?;
    let name = match &args.stdin_filepath {
        Some(path) => report::display_path(path),
        None => "<stdin>".to_string(),
    };

    match check_file(&input, options) {
        CheckOutcome::Unchanged => {
            if !args.check {
                print!("{input}");
            }
            Ok(0)
        }
        CheckOutcome::NeedsFormatting(formatted) => {
            if args.check {
                eprintln!("needs formatting {name}");
                if args.diff {
                    print_diff(&name, &input, &formatted);
                }
                Ok(1)
            } else {
                print!("{formatted}");
                Ok(0)
            }
        }
        CheckOutcome::Skipped(errors) => {
            let index = LineIndex::new(&input);
            for err in &errors {
                let (line, column) = index.line_col(&input, usize::from(err.range.start()));
                eprintln!("error: {} ({name}:{line}:{column})", err.message);
            }
            Ok(1)
        }
        CheckOutcome::Unstable(err) => {
            eprintln!("error: {err} ({name})");
            Ok(1)
        }
    }
}

pub fn run(args: &FormatArgs) -> Result<u8> {
    let common = &args.common;
    let resolved = config::load_config(common.config.as_deref())?;
    let options =
        resolve_format_options(args.quote_style.as_deref(), args.indent_width, args.use_tabs, &resolved)?;

    if args.stdin {
        return run_stdin(args, &options);
    }

    let discovered = discover::discover(&common.paths)?;
    let base_dir = config::base_dir(&resolved);
    let excludes = config::build_exclude_matcher(&resolved.lint.exclude)?;
    let files = discovered.filtered_files(&excludes, &base_dir);

    // Without --check/--write the formatted text of exactly one file goes to stdout.
    let bare = !args.check && !args.write;
    if bare && files.len() != 1 {
        if files.is_empty() {
            let paths: Vec<String> = common.paths.iter().map(|p| report::display_path(p)).collect();
            let paths = if paths.is_empty() { ".".to_string() } else { paths.join(", ") };
            bail!("no LikeC4 documents found under {paths}");
        }
        bail!(
            "format: {} file(s) matched; pass --write or --check to process more than one file",
            files.len()
        );
    }

    let mut read = read_files(&files, &[]);
    let mut diagnostics = std::mem::take(&mut read.diagnostics);
    let total = files.len();
    let pretty = common.output_format() == OutputFormat::Pretty;
    // Per-file status lines ("formatted X", "needs formatting X", ...) are human-oriented
    // text: not in --format json (stdout stays a single JSON value) and not with --quiet.
    let status = pretty && !common.quiet;

    let mut formatted_count = 0usize;
    let mut needs_formatting_count = 0usize;

    for file in read.requested() {
        let name = || report::display_path(&file.path);
        match check_file(&file.text, &options) {
            CheckOutcome::Unchanged => {
                if bare {
                    print!("{}", file.text);
                }
            }
            CheckOutcome::NeedsFormatting(formatted) => {
                if args.write {
                    match write_atomically(&file.path, &formatted) {
                        Ok(()) => {
                            if status {
                                println!("formatted {}", name());
                            }
                            formatted_count += 1;
                        }
                        Err(err) => diagnostics.push(report::io_error_diagnostic(
                            file.path.clone(),
                            format!("failed to write file: {err}"),
                        )),
                    }
                } else if args.check {
                    if status {
                        println!("needs formatting {}", name());
                    }
                    if pretty && args.diff {
                        print_diff(&name(), &file.text, &formatted);
                    }
                    diagnostics.push(report::needs_formatting_diagnostic(file.path.clone()));
                    needs_formatting_count += 1;
                } else {
                    print!("{formatted}");
                }
            }
            CheckOutcome::Skipped(errors) => {
                diagnostics.extend(syntax_error_diagnostics(&file.path, &errors));
                if status {
                    println!("skipped (syntax errors) {}", name());
                }
            }
            CheckOutcome::Unstable(err) => {
                diagnostics.push(format_error_diagnostic(file.path.clone(), &err));
                if status {
                    println!("skipped (formatter output would be invalid) {}", name());
                }
            }
        }
    }

    report::sort_diagnostics(&mut diagnostics);
    let sources = report::source_map(&read.source_files);
    let rep = report::Report {
        diagnostics: &diagnostics,
        sources: &sources,
        files: total,
        config_path: resolved.path.as_deref(),
    };
    if bare {
        // The document itself is the output here; only problems are reported.
        if !diagnostics.is_empty() {
            match common.output_format() {
                OutputFormat::Pretty => {
                    print!("{}", report::render_pretty(&rep, report::use_color(common.color), true))
                }
                OutputFormat::Json => println!("{}", report::render_json(&rep)),
            }
        }
    } else {
        report::emit(&rep, common.output_format(), common.color, common.quiet);
    }

    if status {
        if args.write {
            println!("{formatted_count} of {total} file(s) formatted");
        }
        if args.check {
            println!("{needs_formatting_count} of {total} file(s) need formatting");
        }
    }

    Ok(if report::has_errors(&diagnostics) { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use likec4_syntax::Range;

    fn unstable() -> FormatError {
        FormatError::Unstable {
            errors: vec![SyntaxError { message: "unexpected token".into(), range: Range::empty(3.into()) }],
            formatted: "broken output".into(),
        }
    }

    #[test]
    fn classify_maps_every_formatter_result() {
        assert!(matches!(classify(Ok("x\n".into()), "x\n"), CheckOutcome::Unchanged));
        assert!(matches!(classify(Ok("y\n".into()), "x\n"), CheckOutcome::NeedsFormatting(f) if f == "y\n"));
        let errors = vec![SyntaxError { message: "eof".into(), range: Range::empty(0.into()) }];
        assert!(
            matches!(classify(Err(FormatError::SyntaxErrors(errors)), "x"), CheckOutcome::Skipped(e) if e.len() == 1)
        );
        assert!(matches!(
            classify(Err(unstable()), "x"),
            CheckOutcome::Unstable(FormatError::Unstable { .. })
        ));
    }

    #[test]
    fn unstable_output_becomes_a_format_error_diagnostic_at_the_start_of_the_file() {
        let err = unstable();
        let diagnostic = format_error_diagnostic(PathBuf::from("model.c4"), &err);
        assert_eq!(diagnostic.rule, "format-error");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.message, err.to_string());
        assert_eq!(diagnostic.file, PathBuf::from("model.c4"));
        assert_eq!(diagnostic.range, Range::empty(0.into()));
        assert!(!diagnostic.message.contains("broken output"));
    }

    #[test]
    fn indent_width_must_be_in_range() {
        let resolved = config::ResolvedConfig::default();
        for width in [1, 2, 16] {
            let options = resolve_format_options(None, Some(width), false, &resolved).unwrap();
            assert_eq!(options.indent_width, width);
        }
        for width in [0, 17] {
            let err = resolve_format_options(None, Some(width), false, &resolved).unwrap_err().to_string();
            assert!(err.contains("indent_width") && err.contains(&width.to_string()), "{err}");
        }
    }

    #[test]
    fn use_tabs_comes_from_the_config_unless_the_flag_forces_it() {
        let mut resolved = config::ResolvedConfig::default();
        assert!(resolve_format_options(None, None, false, &resolved).unwrap().insert_spaces);
        assert!(!resolve_format_options(None, None, true, &resolved).unwrap().insert_spaces);
        resolved.format.use_tabs = Some(true);
        assert!(!resolve_format_options(None, None, false, &resolved).unwrap().insert_spaces);
    }

    #[test]
    fn write_atomically_replaces_the_contents_and_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.c4");
        fs::write(&path, "old").unwrap();
        write_atomically(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(entries, [OsString::from("model.c4")]);
    }

    #[cfg(unix)]
    #[test]
    fn write_atomically_writes_through_a_symlink_and_keeps_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target.c4");
        let link = dir.path().join("link.c4");
        fs::write(&target, "old").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        write_atomically(&link, "new").unwrap();

        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(fs::metadata(&target).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
