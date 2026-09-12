//! Loading and applying `likec4-lint.toml`.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use likec4_fmt::QuoteStyle;
use likec4_rules::LintConfig;
use serde::Deserialize;

use crate::{discover, report};

pub const CONFIG_FILENAME: &str = "likec4-lint.toml";

/// Accepted values of `--indent-width` / `[format] indent_width`.
pub const INDENT_WIDTH_RANGE: RangeInclusive<usize> = 1..=16;

/// Error message for an `indent_width` outside [`INDENT_WIDTH_RANGE`].
pub fn check_indent_width(width: usize) -> std::result::Result<(), String> {
    if INDENT_WIDTH_RANGE.contains(&width) {
        Ok(())
    } else {
        Err(format!(
            "indent_width must be between {} and {} (got {width})",
            INDENT_WIDTH_RANGE.start(),
            INDENT_WIDTH_RANGE.end()
        ))
    }
}

/// `[format]` section of `likec4-lint.toml`. Unknown keys are rejected.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FormatSection {
    pub quote_style: Option<String>,
    pub indent_width: Option<usize>,
    pub use_tabs: Option<bool>,
}

/// The whole config file. Unknown sections are rejected.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ConfigFile {
    format: FormatSection,
    lint: LintConfig,
}

/// Fully resolved configuration: the parsed file (if any) plus where it was found.
#[derive(Debug, Clone, Default)]
pub struct ResolvedConfig {
    /// Absolute path of the config file in use, if any.
    pub path: Option<PathBuf>,
    pub format: FormatSection,
    pub lint: LintConfig,
}

impl ResolvedConfig {
    /// Directory of the config file in use, if any.
    pub fn dir(&self) -> Option<&Path> {
        self.path.as_deref().and_then(Path::parent)
    }
}

/// Search upwards from `start` (a file or directory) for `likec4-lint.toml`. The search
/// stops after the first directory that contains `.git` and after the home directory
/// (see [`discover::is_search_boundary`]), so a config file above a repository is never
/// picked up from inside it.
pub fn find_config_upwards(start: &Path) -> Option<PathBuf> {
    let home = discover::home_dir();
    let mut dir =
        if start.is_dir() { Some(start.to_path_buf()) } else { start.parent().map(Path::to_path_buf) };

    while let Some(d) = dir {
        let candidate = d.join(CONFIG_FILENAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        if discover::is_search_boundary(&d, home.as_deref()) {
            return None;
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
        Some(p) => Some(discover::absolute(p)),
        None => {
            let cwd = std::env::current_dir().context("failed to get current directory")?;
            find_config_upwards(&cwd)
        }
    };

    let Some(path) = path else {
        return Ok(ResolvedConfig::default());
    };

    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file: {}", report::display_path(&path)))?;
    let file: ConfigFile = toml::from_str(&text).map_err(|err| {
        anyhow!(
            "failed to parse config file: {}\n{}",
            report::display_path(&path),
            err.to_string().trim_end()
        )
    })?;
    // Every subcommand validates the whole file, so a bad `[format]` value fails `lint` too
    // instead of surfacing only once `format` or `check` runs.
    validate_format(&file.format)
        .map_err(|err| anyhow!("invalid config file: {}\n{err}", report::display_path(&path)))?;

    Ok(ResolvedConfig { path: Some(path), format: file.format, lint: file.lint })
}

/// Check the values of the `[format]` section.
fn validate_format(section: &FormatSection) -> std::result::Result<(), String> {
    if let Some(style) = &section.quote_style {
        style.parse::<QuoteStyle>().map_err(|err| format!("[format] quote_style: {err}"))?;
    }
    if let Some(width) = section.indent_width {
        check_indent_width(width).map_err(|err| format!("[format] {err}"))?;
    }
    Ok(())
}

/// Directory that relative config values (such as `[lint].exclude` globs) are resolved against.
pub fn base_dir(resolved: &ResolvedConfig) -> PathBuf {
    match resolved.dir() {
        Some(dir) => dir.to_path_buf(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_keys_are_rejected_with_the_key_name() {
        let err = toml::from_str::<ConfigFile>("[format]\nindent = 4\n").unwrap_err().to_string();
        assert!(err.contains("indent"), "{err}");
        let err = toml::from_str::<ConfigFile>("[formatting]\n").unwrap_err().to_string();
        assert!(err.contains("formatting"), "{err}");
    }

    #[test]
    fn use_tabs_is_read_from_the_format_section() {
        let file: ConfigFile = toml::from_str("[format]\nuse_tabs = true\nindent_width = 4\n").unwrap();
        assert_eq!(file.format.use_tabs, Some(true));
        assert_eq!(file.format.indent_width, Some(4));
        let file: ConfigFile = toml::from_str("").unwrap();
        assert_eq!(file.format.use_tabs, None);
    }
}
