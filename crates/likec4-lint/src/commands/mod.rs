pub mod check;
pub mod format;
pub mod lint;

use std::path::PathBuf;

use anyhow::Result;
use likec4_rules::{Diagnostic, SourceFile};
use rayon::prelude::*;

use crate::cli::CommonArgs;
use crate::{config, discover, report};

/// Result of reading a batch of files in parallel.
pub struct ReadFiles {
    /// Successfully read files: context files first, then the requested files, each
    /// group in input order.
    pub source_files: Vec<SourceFile>,
    /// Index in `source_files` of the first requested file.
    requested_start: usize,
    /// One `io-error` diagnostic per requested file that could not be read.
    pub diagnostics: Vec<Diagnostic>,
}

impl ReadFiles {
    /// The requested files that could be read, in input order.
    pub fn requested(&self) -> &[SourceFile] {
        &self.source_files[self.requested_start..]
    }
}

/// Read `files` and `context_files` in parallel. Unreadable requested files are turned
/// into diagnostics rather than aborting the whole run; unreadable context files are
/// silently skipped.
///
/// Context files come first in `source_files`: `duplicate-*` rules report the second
/// declaration they see, so the requested file must be the later one or the report
/// would land on a context file and be dropped by `report::retain_reported`.
pub fn read_files(files: &[PathBuf], context_files: &[PathBuf]) -> ReadFiles {
    let read = |path: &PathBuf| (path.clone(), std::fs::read_to_string(path));
    let context: Vec<_> = context_files.par_iter().map(read).collect();
    let requested: Vec<_> = files.par_iter().map(read).collect();

    let mut source_files = Vec::with_capacity(context.len() + requested.len());
    source_files.extend(
        context.into_iter().filter_map(|(path, result)| result.ok().map(|text| SourceFile { path, text })),
    );
    let requested_start = source_files.len();

    let mut diagnostics = Vec::new();
    for (path, result) in requested {
        match result {
            Ok(text) => source_files.push(SourceFile { path, text }),
            Err(err) => {
                diagnostics.push(report::io_error_diagnostic(path, format!("failed to read file: {err}")))
            }
        }
    }

    ReadFiles { source_files, requested_start, diagnostics }
}

/// Output of [`lint_paths`], the part shared by `lint` and `check`.
pub struct Linted {
    pub resolved: config::ResolvedConfig,
    /// Files that diagnostics are reported for: the requested ones minus `[lint].exclude`.
    pub files: Vec<PathBuf>,
    pub read: ReadFiles,
    /// Lint (and read) diagnostics for `files`, plus configuration diagnostics. Unsorted.
    pub diagnostics: Vec<Diagnostic>,
}

/// Load the config, discover the requested documents and their project context, read
/// everything and run the lint rules.
pub fn lint_paths(common: &CommonArgs) -> Result<Linted> {
    let resolved = config::load_config(common.config.as_deref())?;
    let base_dir = config::base_dir(&resolved);
    let excludes = config::build_exclude_matcher(&resolved.lint.exclude)?;

    let mut discovered = discover::discover(&common.paths)?;
    discovered.collect_context(resolved.dir())?;

    // `[lint].exclude` silences a file; it stays part of the model so that everything it
    // declares still resolves from the files that are reported.
    let (files, excluded) = discovered.partition_excluded(&excludes, &base_dir);
    let mut context = discovered.context_files;
    context.extend(excluded);

    let mut read = read_files(&files, &context);
    let mut diagnostics = std::mem::take(&mut read.diagnostics);
    diagnostics.extend(likec4_rules::lint(&read.source_files, &discovered.project_roots, &resolved.lint));
    report::retain_reported(&mut diagnostics, &files);

    Ok(Linted { resolved, files, read, diagnostics })
}
