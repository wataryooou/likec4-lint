//! `likec4-lint check` implementation: `lint` + `format --check` combined.

use anyhow::Result;

use crate::cli::CheckArgs;
use crate::commands::format::{check_file, resolve_format_options, CheckOutcome};
use crate::commands::read_files;
use crate::{config, discover, report};

pub fn run(args: &CheckArgs) -> Result<u8> {
    let common = &args.common;
    let discovered = discover::discover(&common.paths)?;
    let resolved = config::load_config(common.config.as_deref())?;
    let base_dir = config::base_dir(&resolved);
    let excludes = config::build_exclude_matcher(&resolved.lint.exclude)?;

    let files = discovered.filtered_files(&excludes, &base_dir);

    let read = read_files(&files, &discovered.context_files);
    let total = files.len();

    // Lint pass: this already reports syntax errors per file, so the format-check
    // pass below must not report them a second time.
    let mut diagnostics = read.diagnostics;
    diagnostics.extend(likec4_lint::lint(&read.source_files, &discovered.project_roots, &resolved.lint));
    report::retain_reported(&mut diagnostics, &files);

    // Format-check pass: only reports which files need formatting / were skipped.
    // Per-file status lines are human-oriented text; suppress them in --format json
    // so stdout stays a single parseable JSON value.
    let pretty = common.output_format() == report::OutputFormat::Pretty;
    let options = resolve_format_options(None, None, false, &resolved)?;
    let mut needs_formatting_count = 0usize;
    for file in read.source_files.iter().filter(|f| files.contains(&f.path)) {
        match check_file(&file.text, &options) {
            CheckOutcome::Unchanged => {}
            CheckOutcome::NeedsFormatting(_) => {
                if pretty {
                    println!("needs formatting {}", report::display_path(&file.path));
                }
                needs_formatting_count += 1;
            }
            CheckOutcome::Skipped(_) => {
                if pretty {
                    println!("skipped (syntax errors) {}", report::display_path(&file.path));
                }
            }
        }
    }

    report::sort_diagnostics(&mut diagnostics);
    report::emit_diagnostics(
        &diagnostics,
        &read.sources,
        total,
        common.output_format(),
        common.color,
        common.quiet,
    );

    if pretty && !common.quiet {
        println!("{needs_formatting_count} of {total} file(s) need formatting");
    }

    let exit = if report::has_errors(&diagnostics) || needs_formatting_count > 0 { 1 } else { 0 };
    Ok(exit)
}
