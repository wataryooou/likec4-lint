//! `likec4-lint check` implementation: `lint` + `format --check` combined.

use anyhow::Result;

use crate::cli::CheckArgs;
use crate::commands::format::{check_file, format_error_diagnostic, resolve_format_options, CheckOutcome};
use crate::commands::{lint_paths, Linted};
use crate::report;

pub fn run(args: &CheckArgs) -> Result<u8> {
    let common = &args.common;
    let Linted { resolved, files, read, mut diagnostics } = lint_paths(common)?;
    let options = resolve_format_options(None, None, false, &resolved)?;
    let total = files.len();

    // Format-check pass. Syntax errors were already reported by the lint pass, so a
    // skipped file only gets a status line. Status lines are human-oriented text: not in
    // --format json (stdout stays a single JSON value) and not with --quiet.
    let pretty = common.output_format() == report::OutputFormat::Pretty;
    let status = pretty && !common.quiet;
    let mut needs_formatting_count = 0usize;
    for file in read.requested() {
        match check_file(&file.text, &options) {
            CheckOutcome::Unchanged => {}
            CheckOutcome::NeedsFormatting(_) => {
                if status {
                    println!("needs formatting {}", report::display_path(&file.path));
                }
                diagnostics.push(report::needs_formatting_diagnostic(file.path.clone()));
                needs_formatting_count += 1;
            }
            CheckOutcome::Skipped(_) => {
                if status {
                    println!("skipped (syntax errors) {}", report::display_path(&file.path));
                }
            }
            CheckOutcome::Unstable(err) => {
                diagnostics.push(format_error_diagnostic(file.path.clone(), &err));
                if status {
                    println!(
                        "skipped (formatter output would be invalid) {}",
                        report::display_path(&file.path)
                    );
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
    report::emit(&rep, common.output_format(), common.color, common.quiet);

    if status {
        println!("{needs_formatting_count} of {total} file(s) need formatting");
    }

    Ok(if report::has_errors(&diagnostics) { 1 } else { 0 })
}
