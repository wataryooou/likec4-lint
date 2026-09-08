//! Lint rules and project model for the LikeC4 DSL. See `docs/DESIGN.md`.
//!
//! ```
//! use likec4_lint::{lint, LintConfig, SourceFile};
//!
//! let files = [SourceFile {
//!     path: "model.c4".into(),
//!     text: "specification {\n  element system\n}\nmodel {\n  a = system\n  a -> b\n}\n".into(),
//! }];
//! let diagnostics = lint(&files, &[], &LintConfig::default());
//! assert!(diagnostics.iter().any(|d| d.rule == "unresolved-reference"));
//! ```

mod config;
mod model;
mod rules;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use text_size::TextRange;

/// Severity of a diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Configured level of a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Off,
    Info,
    Warning,
    Error,
}

/// A lint or syntax diagnostic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    /// Rule id, e.g. `unknown-element-kind` or `syntax-error`.
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    /// Empty for diagnostics about the configuration itself (`unknown-rule`).
    pub file: PathBuf,
    /// Byte range in the file.
    #[serde(with = "range_serde")]
    pub range: TextRange,
    /// Optional hint on how to fix the problem.
    pub help: Option<String>,
}

mod range_serde {
    use serde::Serializer;
    use text_size::TextRange;

    pub fn serialize<S: Serializer>(range: &TextRange, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Range", 2)?;
        st.serialize_field("start", &u32::from(range.start()))?;
        st.serialize_field("end", &u32::from(range.end()))?;
        st.end()
    }
}

/// Rule option value (subset of TOML/JSON scalars and arrays).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OptionValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<OptionValue>),
}

/// Per-rule configuration: either a bare level or a level with rule-specific options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleConfig {
    Level(Level),
    Detailed {
        level: Level,
        #[serde(flatten)]
        options: BTreeMap<String, OptionValue>,
    },
}

/// `[lint]` section of `likec4-lint.toml`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LintConfig {
    /// Glob patterns (relative to the config file) of files to skip. Applied by the
    /// caller before [`lint`] is invoked.
    pub exclude: Vec<String>,
    /// Rule overrides keyed by rule id.
    pub rules: BTreeMap<String, RuleConfig>,
}

/// A source document handed to the linter.
#[derive(Clone, Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub text: String,
}

/// Metadata about a rule, for `--list-rules` and documentation.
#[derive(Clone, Copy, Debug)]
pub struct RuleInfo {
    pub id: &'static str,
    pub default_level: Level,
    pub description: &'static str,
}

/// All known rules, in registration order.
pub fn rules() -> &'static [RuleInfo] {
    rules::INFOS
}

/// Lint a set of documents. Documents are grouped into projects by `project_roots`
/// (directories containing a `likec4.config.*` or `.likec4rc` file); a document belongs
/// to the deepest project root that contains it, or to the implicit default project.
///
/// Rules are enabled, silenced or re-levelled through `config.rules`; an unknown rule id
/// yields one `unknown-rule` warning with an empty `file`. The result is sorted by
/// `(file, range.start, rule)`.
pub fn lint(files: &[SourceFile], project_roots: &[PathBuf], config: &LintConfig) -> Vec<Diagnostic> {
    let resolved = config::resolve(config);
    let workspace = model::Workspace::build(files, project_roots);
    let mut diagnostics = resolved.diagnostics;
    for rule in &resolved.enabled {
        let mut cx = rules::Cx::new(&workspace, &rule.options);
        rules::RUNNERS[rule.index](&mut cx);
        diagnostics.extend(cx.into_diagnostics(rules::INFOS[rule.index].id, rule.severity));
    }
    diagnostics.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| a.range.start().cmp(&b.range.start()))
            .then_with(|| a.rule.cmp(&b.rule))
    });
    diagnostics
}

/// True when `path` looks like a LikeC4 document.
pub fn is_likec4_document(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.ends_with(".c4") || name.ends_with(".likec4") || name.ends_with(".like-c4")
}

/// File names that mark a LikeC4 project root (from `@likec4/config` `filenames.ts`).
pub const PROJECT_CONFIG_FILENAMES: &[&str] = &[
    ".likec4rc",
    ".likec4.config.json",
    "likec4.config.json",
    "likec4.config.js",
    "likec4.config.cjs",
    "likec4.config.mjs",
    "likec4.config.ts",
    "likec4.config.cts",
    "likec4.config.mts",
];

/// True when `path` is a project marker file.
pub fn is_project_config(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    PROJECT_CONFIG_FILENAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_ids_are_unique_and_match_design() {
        let ids: Vec<&str> = rules().iter().map(|r| r.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate rule ids");
        assert_eq!(ids.len(), 24);
        assert_eq!(ids[0], "syntax-error");
    }

    #[test]
    fn diagnostics_are_sorted_and_config_errors_come_first() {
        let files = [
            SourceFile { path: "b.c4".into(), text: "model {\n  a = ghost\n  a -> zz\n}\n".into() },
            SourceFile { path: "a.c4".into(), text: "model {\n  x = ghost\n}\n".into() },
        ];
        let mut config = LintConfig::default();
        config.rules.insert("nope".into(), RuleConfig::Level(Level::Off));
        let diagnostics = lint(&files, &[], &config);
        assert_eq!(diagnostics[0].rule, "unknown-rule");
        let files_in_order: Vec<&Path> = diagnostics.iter().skip(1).map(|d| d.file.as_path()).collect();
        let mut sorted = files_in_order.clone();
        sorted.sort();
        assert_eq!(files_in_order, sorted);
    }

    #[test]
    fn document_and_config_detection() {
        assert!(is_likec4_document(Path::new("x/model.c4")));
        assert!(is_likec4_document(Path::new("model.likec4")));
        assert!(!is_likec4_document(Path::new("model.txt")));
        assert!(is_project_config(Path::new("p/.likec4rc")));
        assert!(is_project_config(Path::new("p/likec4.config.mjs")));
        assert!(!is_project_config(Path::new("p/likec4.json")));
    }
}
