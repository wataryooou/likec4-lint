pub mod check;
pub mod format;
pub mod lint;

use std::collections::HashMap;
use std::path::PathBuf;

use likec4_lint::{Diagnostic, SourceFile};
use rayon::prelude::*;

use crate::report;

/// Result of reading a batch of files in parallel.
pub struct ReadFiles {
    /// Successfully read files, in the same order as the input paths.
    pub source_files: Vec<SourceFile>,
    /// Source text of every successfully read file, keyed by path.
    pub sources: HashMap<PathBuf, String>,
    /// One `io-error` diagnostic per file that could not be read.
    pub diagnostics: Vec<Diagnostic>,
}

/// Read `files` and `context_files` in parallel. Unreadable requested files are turned
/// into diagnostics rather than aborting the whole run; unreadable context files are
/// silently skipped.
pub fn read_files(files: &[PathBuf], context_files: &[PathBuf]) -> ReadFiles {
    let results: Vec<(PathBuf, std::io::Result<String>)> = files
        .par_iter()
        .map(|p| (p.clone(), std::fs::read_to_string(p)))
        .chain(context_files.par_iter().map(|p| (p.clone(), std::fs::read_to_string(p))))
        .collect();
    let requested: std::collections::HashSet<&PathBuf> = files.iter().collect();

    let mut source_files = Vec::with_capacity(results.len());
    let mut sources = HashMap::with_capacity(results.len());
    let mut diagnostics = Vec::new();

    for (path, result) in results {
        match result {
            Ok(text) => {
                sources.insert(path.clone(), text.clone());
                source_files.push(SourceFile { path, text });
            }
            Err(err) if requested.contains(&path) => {
                diagnostics.push(report::io_error_diagnostic(path, &err))
            }
            Err(_) => {}
        }
    }

    ReadFiles { source_files, sources, diagnostics }
}
