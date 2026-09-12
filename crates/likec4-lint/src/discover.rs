//! File discovery: turn CLI `PATHS` into a list of LikeC4 documents plus the project
//! roots found along the way, and collect the other documents of those projects as
//! context.

use std::collections::{BTreeSet, HashSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use ignore::WalkBuilder;

pub struct Discovered {
    /// LikeC4 documents to process, sorted and deduplicated (two names for the same file,
    /// such as a symlink and its target, keep the name that appeared first).
    pub files: Vec<PathBuf>,
    /// Subset of `files` that were named directly on the command line (as opposed to
    /// found while walking a directory). `[lint].exclude` must never filter these out:
    /// an explicit file argument is always processed, regardless of extension or excludes.
    pub explicit_files: BTreeSet<PathBuf>,
    /// Directories that contain a project config marker file (e.g. `likec4.config.json`).
    pub project_roots: Vec<PathBuf>,
    /// Documents that belong to the same projects as `files` but were not requested on the
    /// command line. They are loaded so that specifications, elements and views declared in
    /// sibling files resolve, but no diagnostics are reported for them. Filled by
    /// [`Discovered::collect_context`].
    pub context_files: Vec<PathBuf>,
    /// The `PATHS` arguments (absolute) and whether each one names a file.
    roots: Vec<(PathBuf, bool)>,
    /// Canonical paths of `files`.
    canonical_files: HashSet<PathBuf>,
}

impl Discovered {
    /// Split `files` by `[lint].exclude` into `(reported, excluded)`. Explicit file
    /// arguments (see `explicit_files`) are always reported.
    pub fn partition_excluded(
        &self,
        excludes: &globset::GlobSet,
        base_dir: &Path,
    ) -> (Vec<PathBuf>, Vec<PathBuf>) {
        self.files.iter().cloned().partition(|p| {
            self.explicit_files.contains(p) || !crate::config::is_excluded(excludes, p, base_dir)
        })
    }

    /// `files` minus `[lint].exclude` (explicit file arguments are always kept).
    pub fn filtered_files(&self, excludes: &globset::GlobSet, base_dir: &Path) -> Vec<PathBuf> {
        self.partition_excluded(excludes, base_dir).0
    }

    /// Fill `context_files` with the documents that share a project with a requested file.
    ///
    /// For files under a project marker that is the rest of the marker's directory tree. For
    /// files that belong to no marker (the default project) the tree to load is rooted at the
    /// directory of the config file in use (`config_dir`) when the requested path lies below
    /// it. Otherwise a directory argument delimits its own tree, like a LikeC4 workspace, and
    /// a file argument gets, in order of preference: the nearest directory at or above it
    /// that contains `.git`, the working directory when the file lies below it, or else the
    /// file's own directory.
    ///
    /// The home directory is never an origin, whichever rule would select it: a config file,
    /// a marker or `.git` directly in `$HOME` must not turn the whole home directory into one
    /// project. Markers found while walking are added to `project_roots`, so documents of
    /// nested projects stay grouped separately by the linter.
    pub fn collect_context(&mut self, config_dir: Option<&Path>) -> Result<()> {
        let cwd = std::env::current_dir().context("failed to get current directory")?;
        let config_dir = config_dir.map(absolute);

        let mut origins: BTreeSet<PathBuf> = self.project_roots.iter().cloned().collect();
        for (root, is_file) in &self.roots {
            if self.has_default_project_file(root, *is_file) {
                origins.extend(default_project_origin(root, *is_file, config_dir.as_deref(), &cwd));
            }
        }

        let mut seen = self.canonical_files.clone();
        let mut context = BTreeSet::new();
        let mut markers = BTreeSet::new();
        for origin in &origins {
            let walked = walk(origin);
            markers.extend(walked.markers);
            for document in walked.documents {
                if seen.insert(canonical(&document)) {
                    context.insert(document);
                }
            }
        }

        self.project_roots.extend(markers);
        self.project_roots.sort();
        self.project_roots.dedup();
        self.context_files = context.into_iter().collect();
        Ok(())
    }

    /// True when the argument `root` contributes at least one file that lies under no project marker.
    fn has_default_project_file(&self, root: &Path, is_file: bool) -> bool {
        let in_marker_project = |path: &Path| self.project_roots.iter().any(|r| path.starts_with(r));
        if is_file {
            !in_marker_project(root)
        } else {
            self.files.iter().any(|f| f.starts_with(root) && !in_marker_project(f))
        }
    }
}

/// Everything a directory walk found.
#[derive(Default)]
struct Walked {
    documents: Vec<PathBuf>,
    /// Directories containing a project marker file.
    markers: BTreeSet<PathBuf>,
    /// First error met while walking, if any (the walk continues past it).
    error: Option<ignore::Error>,
}

fn is_node_modules(entry: &ignore::DirEntry) -> bool {
    entry.file_name().to_str() == Some("node_modules")
}

/// Walk `root` recursively, respecting `.gitignore` (inside and outside git repositories),
/// skipping hidden entries and `node_modules`, and collect the LikeC4 documents and project
/// markers found.
fn walk(root: &Path) -> Walked {
    let mut walked = Walked::default();
    let walker =
        WalkBuilder::new(root).require_git(false).filter_entry(|entry| !is_node_modules(entry)).build();

    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                walked.error.get_or_insert(err);
                continue;
            }
        };
        let path = entry.path();
        let Some(file_type) = entry.file_type() else { continue };
        if file_type.is_dir() {
            // Hidden markers such as `.likec4rc` are skipped by the walker, so probe
            // every visited directory for the known config file names.
            if has_project_config(path) {
                walked.markers.insert(path.to_path_buf());
            }
        } else if file_type.is_file() && likec4_rules::is_likec4_document(path) {
            walked.documents.push(path.to_path_buf());
        }
    }
    walked
}

/// Discover LikeC4 documents under `paths`. An explicit file argument is always
/// included, regardless of its extension. An explicit directory argument is walked
/// recursively (respecting `.gitignore`, skipping hidden entries and `node_modules`)
/// and only files matching `likec4_rules::is_likec4_document` are kept.
pub fn discover(paths: &[PathBuf]) -> Result<Discovered> {
    let args: Vec<PathBuf> = if paths.is_empty() { vec![PathBuf::from(".")] } else { paths.to_vec() };

    let mut files = Vec::new();
    let mut explicit_files = BTreeSet::new();
    let mut project_roots = BTreeSet::new();
    let mut roots = Vec::with_capacity(args.len());

    for arg in &args {
        let metadata =
            std::fs::metadata(arg).with_context(|| format!("path not found: {}", arg.display()))?;
        let root = absolute(arg);

        // A project may be defined by a marker above the path being processed.
        if let Some(ancestor) = nearest_project_root_above(&root) {
            project_roots.insert(ancestor);
        }

        if metadata.is_file() {
            files.push(root.clone());
            explicit_files.insert(root.clone());
            roots.push((root, true));
            continue;
        }

        let walked = walk(&root);
        if let Some(err) = walked.error {
            return Err(err).with_context(|| format!("failed to walk {}", root.display()));
        }
        files.extend(walked.documents);
        project_roots.extend(walked.markers);
        roots.push((root, false));
    }

    // Two names for the same file (symlink and target, or a path given twice) would be
    // linted twice and trip the duplicate-* rules; keep the first name only.
    let mut canonical_files = HashSet::new();
    files.retain(|path| canonical_files.insert(canonical(path)));
    files.sort();

    Ok(Discovered {
        files,
        explicit_files,
        project_roots: project_roots.into_iter().collect(),
        context_files: Vec::new(),
        roots,
        canonical_files,
    })
}

/// Context origin of a default-project argument; see [`Discovered::collect_context`].
/// `None` when the only candidate is the home directory.
fn default_project_origin(
    requested: &Path,
    is_file: bool,
    config_dir: Option<&Path>,
    cwd: &Path,
) -> Option<PathBuf> {
    let home = home_dir();
    let usable = |dir: &Path| !is_home(dir, home.as_deref());
    let start = if is_file { requested.parent().unwrap_or(requested) } else { requested };
    if let Some(dir) = config_dir.filter(|dir| start.starts_with(dir) && usable(dir)) {
        return Some(dir.to_path_buf());
    }
    if !is_file {
        return Some(start.to_path_buf());
    }
    if let Some(dir) = git_root_of(start) {
        return Some(dir);
    }
    if start.starts_with(cwd) && usable(cwd) {
        return Some(cwd.to_path_buf());
    }
    usable(start).then(|| start.to_path_buf())
}

/// Nearest directory at or above `start` that contains `.git`; the home directory and
/// everything above it are never candidates.
fn git_root_of(start: &Path) -> Option<PathBuf> {
    let home = home_dir();
    for dir in start.ancestors() {
        if is_home(dir, home.as_deref()) {
            return None;
        }
        if has_git(dir) {
            return Some(dir.to_path_buf());
        }
    }
    None
}

/// Absolute, lexically normalised form of `path`: `.` and `..` components are folded
/// away, but symlinks are not resolved, so paths stay comparable with each other, with
/// the working directory and with the project roots.
pub fn absolute(path: &Path) -> PathBuf {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    normalized
}

/// `path` with symlinks resolved, or `path` itself when that fails.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The user's home directory, canonicalised.
pub fn home_dir() -> Option<PathBuf> {
    std::env::home_dir().and_then(|home| std::fs::canonicalize(home).ok())
}

/// True when `dir` contains `.git` (a directory, or the file of a worktree or submodule).
fn has_git(dir: &Path) -> bool {
    dir.join(".git").exists()
}

fn is_home(dir: &Path, home: Option<&Path>) -> bool {
    match home {
        Some(home) => std::fs::canonicalize(dir).is_ok_and(|dir| dir == home),
        None => false,
    }
}

/// True when an upward search that has just examined `dir` must stop: `dir` is a
/// repository root (contains `.git`) or the home directory.
pub fn is_search_boundary(dir: &Path, home: Option<&Path>) -> bool {
    has_git(dir) || is_home(dir, home)
}

/// True when `dir` directly contains one of the LikeC4 project config files.
fn has_project_config(dir: &Path) -> bool {
    likec4_rules::PROJECT_CONFIG_FILENAMES.iter().any(|name| dir.join(name).is_file())
}

/// Nearest ancestor of `path` (excluding `path` itself) that contains a project config
/// file. The search stops at a repository root (after examining it) and at the home
/// directory, which is never a project root even when it holds a marker.
fn nearest_project_root_above(path: &Path) -> Option<PathBuf> {
    let home = home_dir();
    let path = absolute(path);
    let mut current = path.parent();
    while let Some(candidate) = current {
        if is_home(candidate, home.as_deref()) {
            return None;
        }
        if has_project_config(candidate) {
            return Some(candidate.to_path_buf());
        }
        if is_search_boundary(candidate, home.as_deref()) {
            return None;
        }
        current = candidate.parent();
    }
    None
}
