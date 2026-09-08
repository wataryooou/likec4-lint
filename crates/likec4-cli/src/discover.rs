//! File discovery: turn CLI `PATHS` into a list of LikeC4 documents plus the
//! project roots found along the way.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ignore::WalkBuilder;

pub struct Discovered {
    /// LikeC4 documents to process, sorted and deduplicated.
    pub files: Vec<PathBuf>,
    /// Subset of `files` that were named directly on the command line (as opposed to
    /// found while walking a directory). `[lint].exclude` must never filter these out:
    /// an explicit file argument is always processed, regardless of extension or excludes.
    pub explicit_files: BTreeSet<PathBuf>,
    /// Directories that contain a project config marker file (e.g. `likec4.config.json`).
    pub project_roots: Vec<PathBuf>,
    /// Documents that belong to the same projects as `files` but were not requested on the
    /// command line. They are loaded so that specifications, elements and views declared in
    /// sibling files resolve, but no diagnostics are reported for them.
    pub context_files: Vec<PathBuf>,
}

impl Discovered {
    /// `files` filtered by `[lint].exclude`, except explicit file arguments (see
    /// `explicit_files`), which are always kept.
    pub fn filtered_files(&self, excludes: &globset::GlobSet, base_dir: &std::path::Path) -> Vec<PathBuf> {
        self.files
            .iter()
            .filter(|p| {
                self.explicit_files.contains(p.as_path())
                    || !crate::config::is_excluded(excludes, p, base_dir)
            })
            .cloned()
            .collect()
    }
}

fn is_node_modules(entry: &ignore::DirEntry) -> bool {
    entry.file_name().to_str() == Some("node_modules")
}

/// Discover LikeC4 documents under `paths`. An explicit file argument is always
/// included, regardless of its extension. An explicit directory argument is walked
/// recursively (respecting `.gitignore`, skipping hidden entries and `node_modules`)
/// and only files matching `likec4_lint::is_likec4_document` are kept.
pub fn discover(paths: &[PathBuf]) -> Result<Discovered> {
    let roots: Vec<PathBuf> = if paths.is_empty() { vec![PathBuf::from(".")] } else { paths.to_vec() };

    let mut files = Vec::new();
    let mut explicit_files = BTreeSet::new();
    let mut project_roots = BTreeSet::new();

    for root in &roots {
        let metadata =
            std::fs::metadata(root).with_context(|| format!("path not found: {}", root.display()))?;
        let root = absolute(root);

        if metadata.is_file() {
            if let Some(ancestor) = nearest_project_root_above(&root) {
                project_roots.insert(ancestor);
            }
            files.push(root.clone());
            explicit_files.insert(root.clone());
            continue;
        }

        // A project may be defined by a config file above the directory being linted.
        if let Some(ancestor) = nearest_project_root_above(&root) {
            project_roots.insert(ancestor);
        }

        let walker = WalkBuilder::new(&root).filter_entry(|entry| !is_node_modules(entry)).build();

        for entry in walker {
            let entry = entry.with_context(|| format!("failed to walk {}", root.display()))?;
            let path = entry.path();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                // Hidden markers such as `.likec4rc` are skipped by the walker, so probe
                // every visited directory for the known config file names.
                if has_project_config(path) {
                    project_roots.insert(path.to_path_buf());
                }
                continue;
            }
            let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
            if !is_file {
                continue;
            }
            if likec4_lint::is_likec4_document(path) {
                files.push(path.to_path_buf());
            }
            if likec4_lint::is_project_config(path) {
                if let Some(parent) = path.parent() {
                    project_roots.insert(parent.to_path_buf());
                }
            }
        }
    }

    files.sort();
    files.dedup();

    // Load the rest of every project that contains a requested file, as context.
    let file_set: BTreeSet<&Path> = files.iter().map(PathBuf::as_path).collect();
    let mut context_files = BTreeSet::new();
    for project_root in &project_roots {
        let walker = WalkBuilder::new(project_root).filter_entry(|entry| !is_node_modules(entry)).build();
        for entry in walker.flatten() {
            let is_file = entry.file_type().map(|t| t.is_file()).unwrap_or(false);
            let path = entry.path();
            if is_file && likec4_lint::is_likec4_document(path) && !file_set.contains(path) {
                context_files.insert(path.to_path_buf());
            }
        }
    }

    Ok(Discovered {
        files,
        explicit_files,
        project_roots: project_roots.into_iter().collect(),
        context_files: context_files.into_iter().collect(),
    })
}

/// Absolute, lexically normalised form of `path` (no symlink resolution, so paths stay
/// comparable with each other and with the project roots).
pub fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// True when `dir` directly contains one of the LikeC4 project config files.
fn has_project_config(dir: &Path) -> bool {
    likec4_lint::PROJECT_CONFIG_FILENAMES.iter().any(|name| dir.join(name).is_file())
}

/// Nearest ancestor of `dir` (excluding `dir` itself) that contains a project config file.
fn nearest_project_root_above(dir: &Path) -> Option<PathBuf> {
    let dir = absolute(dir);
    let mut current = dir.parent();
    while let Some(candidate) = current {
        if has_project_config(candidate) {
            return Some(candidate.to_path_buf());
        }
        current = candidate.parent();
    }
    None
}
