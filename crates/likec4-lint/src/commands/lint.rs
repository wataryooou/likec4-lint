//! `likec4-lint lint` implementation.

use anyhow::Result;

use crate::cli::LintArgs;
use crate::commands::{lint_paths, Linted};
use crate::report;

pub fn run(args: &LintArgs) -> Result<u8> {
    if args.list_rules {
        report::print_rules_table(likec4_rules::rules());
        return Ok(0);
    }

    let common = &args.common;
    let Linted { resolved, files, read, mut diagnostics } = lint_paths(common)?;
    report::sort_diagnostics(&mut diagnostics);

    let sources = report::source_map(&read.source_files);
    let rep = report::Report {
        diagnostics: &diagnostics,
        sources: &sources,
        files: files.len(),
        config_path: resolved.path.as_deref(),
    };
    report::emit(&rep, common.output_format(), common.color, common.quiet);

    Ok(if report::has_errors(&diagnostics) { 1 } else { 0 })
}
