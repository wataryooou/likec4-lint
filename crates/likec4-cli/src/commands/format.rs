//! `likec4-lint format` implementation.

use std::path::Path;

use anyhow::{bail, Context, Result};
use likec4_fmt::{FormatError, FormatOptions, QuoteStyle};
use likec4_lint::{Diagnostic, Severity};
use likec4_syntax::SyntaxError;

use crate::cli::FormatArgs;
use crate::commands::read_files;
use crate::report::OutputFormat;
use crate::{config, discover, report};

/// Outcome of formatting a single file without writing anything to disk.
pub enum CheckOutcome {
    Unchanged,
    NeedsFormatting(String),
    Skipped(Vec<SyntaxError>),
}

/// Resolve effective `FormatOptions`, applying CLI overrides > config file > defaults.
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
    Ok(FormatOptions { indent_width, insert_spaces: !use_tabs, quote_style })
}

/// Format `text` and classify the result without touching the filesystem.
pub fn check_file(text: &str, options: &FormatOptions) -> CheckOutcome {
    match likec4_fmt::format(text, options) {
        Ok(formatted) if formatted == text => CheckOutcome::Unchanged,
        Ok(formatted) => CheckOutcome::NeedsFormatting(formatted),
        Err(FormatError::SyntaxErrors(errors)) => CheckOutcome::Skipped(errors),
    }
}

fn print_diff(path: &Path, old: &str, new: &str) {
    let old_label = format!("{} (original)", report::display_path(path));
    let new_label = format!("{} (formatted)", report::display_path(path));
    let diff = similar::TextDiff::from_lines(old, new);
    let mut unified = diff.unified_diff();
    unified.header(&old_label, &new_label);
    print!("{unified}");
}

fn print_diagnostics(
    diagnostics: &[Diagnostic],
    sources: &report::SourceMap,
    format: OutputFormat,
    color: crate::report::ColorMode,
    files_count: usize,
) {
    if diagnostics.is_empty() {
        return;
    }
    match format {
        OutputFormat::Pretty => {
            print!("{}", report::render_pretty(diagnostics, sources, report::use_color(color)))
        }
        OutputFormat::Json => println!("{}", report::render_json(diagnostics, sources, files_count)),
    }
}

fn run_stdin(options: &FormatOptions) -> Result<u8> {
    use std::io::Read;

    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).context("failed to read stdin")?;

    match likec4_fmt::format(&input, options) {
        Ok(formatted) => {
            print!("{formatted}");
            Ok(0)
        }
        Err(FormatError::SyntaxErrors(errors)) => {
            for err in &errors {
                let (line, column) =
                    report::offset_to_line_col(&input, u32::from(err.range.start()) as usize);
                eprintln!("error: {} (<stdin>:{line}:{column})", err.message);
            }
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
        return run_stdin(&options);
    }

    let discovered = discover::discover(&common.paths)?;
    let base_dir = config::base_dir(&resolved);
    let excludes = config::build_exclude_matcher(&resolved.lint.exclude)?;
    let files = discovered.filtered_files(&excludes, &base_dir);

    if !args.check && !args.write && files.len() != 1 {
        bail!(
            "format: {} file(s) matched; pass --write or --check to process more than one file",
            files.len()
        );
    }

    let read = read_files(&files, &[]);
    let mut diagnostics = read.diagnostics;
    let total = files.len();
    // Per-file status lines ("formatted X", "needs formatting X", ...) are human-oriented
    // text; suppress them in --format json so stdout stays a single parseable JSON value.
    let pretty = common.output_format() == OutputFormat::Pretty;

    let mut formatted_count = 0usize;
    let mut needs_formatting_count = 0usize;

    for file in &read.source_files {
        match check_file(&file.text, &options) {
            CheckOutcome::Unchanged => {
                if !args.check && !args.write {
                    print!("{}", file.text);
                }
            }
            CheckOutcome::NeedsFormatting(formatted) => {
                if args.write {
                    std::fs::write(&file.path, &formatted)
                        .with_context(|| format!("failed to write {}", file.path.display()))?;
                    if pretty {
                        println!("formatted {}", report::display_path(&file.path));
                    }
                    formatted_count += 1;
                } else if args.check {
                    if pretty {
                        println!("needs formatting {}", report::display_path(&file.path));
                        if args.diff {
                            print_diff(&file.path, &file.text, &formatted);
                        }
                    }
                    needs_formatting_count += 1;
                } else {
                    print!("{formatted}");
                }
            }
            CheckOutcome::Skipped(errors) => {
                for err in &errors {
                    diagnostics.push(Diagnostic {
                        rule: "syntax-error".to_string(),
                        severity: Severity::Error,
                        message: err.message.clone(),
                        file: file.path.clone(),
                        range: err.range,
                        help: None,
                    });
                }
                if pretty {
                    println!("skipped (syntax errors) {}", report::display_path(&file.path));
                }
            }
        }
    }

    report::sort_diagnostics(&mut diagnostics);
    print_diagnostics(&diagnostics, &read.sources, common.output_format(), common.color, total);

    if pretty && !common.quiet {
        if args.write {
            println!("{formatted_count} of {total} file(s) formatted");
        }
        if args.check {
            println!("{needs_formatting_count} of {total} file(s) need formatting");
        }
    }

    let exit = if report::has_errors(&diagnostics) || needs_formatting_count > 0 { 1 } else { 0 };
    Ok(exit)
}
