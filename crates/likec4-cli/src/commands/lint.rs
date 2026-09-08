//! `likec4-lint lint` implementation.

use anyhow::Result;

use crate::cli::LintArgs;
use crate::commands::read_files;
use crate::{config, discover, report};

pub fn run(args: &LintArgs) -> Result<u8> {
    if args.list_rules {
        report::print_rules_table(likec4_lint::rules());
        return Ok(0);
    }

    let common = &args.common;
    let discovered = discover::discover(&common.paths)?;
    let resolved = config::load_config(common.config.as_deref())?;
    let base_dir = config::base_dir(&resolved);
    let excludes = config::build_exclude_matcher(&resolved.lint.exclude)?;

    let files = discovered.filtered_files(&excludes, &base_dir);

    let read = read_files(&files, &discovered.context_files);
    let mut diagnostics = read.diagnostics;
    diagnostics.extend(likec4_lint::lint(&read.source_files, &discovered.project_roots, &resolved.lint));
    report::retain_reported(&mut diagnostics, &files);
    report::sort_diagnostics(&mut diagnostics);

    report::emit_diagnostics(
        &diagnostics,
        &read.sources,
        files.len(),
        common.output_format(),
        common.color,
        common.quiet,
    );

    Ok(if report::has_errors(&diagnostics) { 1 } else { 0 })
}
