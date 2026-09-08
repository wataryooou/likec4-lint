//! Loading and applying `likec4-lint.toml`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use likec4_lint::LintConfig;
use serde::Deserialize;

pub const CONFIG_FILENAME: &str = "likec4-lint.toml";

/// `[format]` section of `likec4-lint.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FormatSection {
    pub quote_style: Option<String>,
    pub indent_width: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct ConfigFile {
    format: FormatSection,
    lint: LintConfig,
}

/// Fully resolved configuration: the parsed file (if any) plus where it was found.
#[derive(Debug, Clone, Default)]
pub struct ResolvedConfig {
    pub path: Option<PathBuf>,
    pub format: FormatSection,
    pub lint: LintConfig,
}

/// Search upwards from `start` (a file or directory) for `likec4-lint.toml`.
pub fn find_config_upwards(start: &Path) -> Option<PathBuf> {
    let mut dir =
        if start.is_dir() { Some(start.to_path_buf()) } else { start.parent().map(Path::to_path_buf) };

    while let Some(d) = dir {
        let candidate = d.join(CONFIG_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        dir = d.parent().map(Path::to_path_buf);
    }
    None
}

/// Load the config file, either the explicit `--config FILE` or the nearest
/// `likec4-lint.toml` found by searching upwards from the current directory.
/// Returns default values when no config file is found.
pub fn load_config(explicit: Option<&Path>) -> Result<ResolvedConfig> {
    let path = match explicit {
        Some(p) => Some(p.to_path_buf()),
        None => {
            let cwd = std::env::current_dir().context("failed to get current directory")?;
            find_config_upwards(&cwd)
        }
    };

    let Some(path) = path else {
        return Ok(ResolvedConfig::default());
    };

    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    let file: ConfigFile =
        toml::from_str(&text).with_context(|| format!("failed to parse config file: {}", path.display()))?;

    Ok(ResolvedConfig { path: Some(path), format: file.format, lint: file.lint })
}

/// Directory that relative config values (such as `[lint].exclude` globs) are resolved against.
pub fn base_dir(resolved: &ResolvedConfig) -> PathBuf {
    match &resolved.path {
        Some(p) => p.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(".")),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Build a matcher for `[lint].exclude` glob patterns.
pub fn build_exclude_matcher(patterns: &[String]) -> Result<globset::GlobSet> {
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            globset::Glob::new(pattern).with_context(|| format!("invalid exclude pattern: {pattern}"))?;
        builder.add(glob);
    }
    builder.build().context("failed to build exclude matcher")
}

fn relative_to(path: &Path, base_dir: &Path) -> PathBuf {
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let abs_base = std::fs::canonicalize(base_dir).unwrap_or_else(|_| base_dir.to_path_buf());
    abs_path.strip_prefix(&abs_base).map(Path::to_path_buf).unwrap_or(abs_path)
}

/// `true` when `path` matches one of the `[lint].exclude` glob patterns.
pub fn is_excluded(matcher: &globset::GlobSet, path: &Path, base_dir: &Path) -> bool {
    if matcher.is_empty() {
        return false;
    }
    matcher.is_match(relative_to(path, base_dir))
}
