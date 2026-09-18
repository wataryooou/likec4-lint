//! Lint rules and project model for the LikeC4 DSL. See `docs/DESIGN.md`.
//!
//! ```
//! use likec4_rules::{lint, LintConfig, SourceFile};
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
mod suppress;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
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

/// Level names accepted in the configuration, in the order they are listed in error messages.
const LEVEL_NAMES: [&str; 4] = ["off", "info", "warning", "error"];

impl Level {
    fn from_name(name: &str) -> Option<Level> {
        match name {
            "off" => Some(Level::Off),
            "info" => Some(Level::Info),
            "warning" => Some(Level::Warning),
            "error" => Some(Level::Error),
            _ => None,
        }
    }
}

/// A secondary location that explains a diagnostic (for example the first of two duplicate declarations).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelatedLocation {
    pub file: PathBuf,
    pub range: TextRange,
    pub message: String,
}

/// A lint or syntax diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Rule id, e.g. `unknown-element-kind` or `syntax-error`.
    pub rule: String,
    pub severity: Severity,
    pub message: String,
    /// Empty for diagnostics about the configuration itself (`unknown-rule`,
    /// `unknown-rule-option` and option type problems reported under a rule's own id).
    pub file: PathBuf,
    /// Byte range in the file.
    pub range: TextRange,
    /// Optional hint on how to fix the problem.
    pub help: Option<String>,
    /// Optional secondary location, e.g. the first declaration for the `duplicate-*` rules.
    pub related: Option<RelatedLocation>,
}

/// Rule option value (subset of TOML/JSON scalars and arrays).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OptionValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<OptionValue>),
}

/// Per-rule configuration: either a bare level (`"off"`) or a table with a mandatory `level`
/// key and rule-specific options (`{ level = "warning", pattern = "^x" }`).
#[derive(Clone, Debug, PartialEq)]
pub enum RuleConfig {
    Level(Level),
    Detailed { level: Level, options: BTreeMap<String, OptionValue> },
}

impl<'de> Deserialize<'de> for RuleConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(RuleConfigVisitor)
    }
}

struct RuleConfigVisitor;

fn level_from_name<E: de::Error>(name: &str) -> Result<Level, E> {
    Level::from_name(name).ok_or_else(|| {
        E::custom(format!("unknown level '{name}' (expected one of: {})", LEVEL_NAMES.join(", ")))
    })
}

impl<'de> Visitor<'de> for RuleConfigVisitor {
    type Value = RuleConfig;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a level name or a table with a `level` key")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<RuleConfig, E> {
        level_from_name(value).map(RuleConfig::Level)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<RuleConfig, A::Error> {
        let mut level = None;
        let mut options = BTreeMap::new();
        while let Some((key, value)) = map.next_entry::<String, OptionValue>()? {
            if key == "level" {
                level = Some(match value {
                    OptionValue::String(name) => level_from_name(&name)?,
                    _ => return Err(de::Error::custom("`level` must be a string")),
                });
            } else {
                options.insert(key, value);
            }
        }
        let level = level.ok_or_else(|| de::Error::missing_field("level"))?;
        Ok(RuleConfig::Detailed { level, options })
    }
}

/// `[lint]` section of `likec4-lint.toml`. Keys other than `exclude` and `rules` are rejected.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
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
    /// Option names the rule accepts in `{ level = "...", <option> = ... }`; any other option
    /// name in the configuration yields an `unknown-rule-option` warning.
    pub options: &'static [&'static str],
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
/// yields one `unknown-rule` warning and an option a rule does not declare yields one
/// `unknown-rule-option` warning, both with an empty `file`. Source comments
/// (`likec4-lint-disable-next-line`, `-disable-line` and `-disable-file`, see `suppress`)
/// then filter the result; an unknown rule id named in one of those comments also yields an
/// `unknown-rule` warning, at the comment's location. The result is sorted by
/// `(file, range.start, rule)`.
pub fn lint(files: &[SourceFile], project_roots: &[PathBuf], config: &LintConfig) -> Vec<Diagnostic> {
    let resolved = config::resolve(config);
    let workspace = model::Workspace::build(files, project_roots);
    let mut diagnostics = resolved.diagnostics;
    for rule in &resolved.enabled {
        let info = &rules::INFOS[rule.index];
        let mut cx = rules::Cx::new(&workspace, info.id, &rule.options);
        rules::RUNNERS[rule.index](&mut cx);
        diagnostics.extend(cx.into_diagnostics(info.id, rule.severity));
    }
    let mut diagnostics = suppress::apply(&workspace, diagnostics);
    diagnostics.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then_with(|| a.range.start().cmp(&b.range.start()))
            .then_with(|| a.rule.cmp(&b.rule))
    });
    diagnostics
}

/// True when `path` looks like a LikeC4 document (`.c4`, `.likec4` or `.like-c4`, in any
/// ASCII case).
pub fn is_likec4_document(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    match name.rsplit_once('.') {
        Some((_, extension)) => {
            ["c4", "likec4", "like-c4"].iter().any(|known| extension.eq_ignore_ascii_case(known))
        }
        None => false,
    }
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
    fn registry_ids_are_unique_and_start_with_syntax_error() {
        let ids: Vec<&str> = rules().iter().map(|r| r.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "duplicate rule ids");
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

    /// Deep nesting is handled by the model walk without per-element recursion (FQNs are
    /// computed in linear time). Nesting deeper than `likec4_syntax::MAX_NODE_DEPTH` is cut
    /// off by the parser and surfaces as a `syntax-error`; the linter must cope with the
    /// flat error node that follows without panicking.
    #[test]
    fn deep_element_nesting_is_linted_without_recursion() {
        fn nested(depth: usize) -> String {
            let mut text = String::from("specification {\n  element system\n}\nmodel {\n");
            text.push_str(&"a = system {\n".repeat(depth));
            text.push_str(&"}\n".repeat(depth + 1));
            text
        }
        // Under the parser's limit: the whole chain is modelled, only the innermost body is empty.
        let files = [SourceFile { path: "deep.c4".into(), text: nested(200) }];
        let rules: Vec<String> =
            lint(&files, &[], &LintConfig::default()).iter().map(|d| d.rule.clone()).collect();
        assert_eq!(rules, ["empty-body"]);
        // Beyond the limit: reported once by the parser, no rule trips over the rest.
        let files = [SourceFile { path: "deeper.c4".into(), text: nested(100_000) }];
        let diagnostics = lint(&files, &[], &LintConfig::default());
        assert!(
            diagnostics.iter().any(|d| d.rule == "syntax-error" && d.message.contains("nesting too deep")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn document_and_config_detection() {
        assert!(is_likec4_document(Path::new("x/model.c4")));
        assert!(is_likec4_document(Path::new("model.likec4")));
        assert!(is_likec4_document(Path::new("model.like-c4")));
        assert!(is_likec4_document(Path::new("MODEL.C4")));
        assert!(is_likec4_document(Path::new("model.LikeC4")));
        assert!(!is_likec4_document(Path::new("model.txt")));
        assert!(!is_likec4_document(Path::new("model.c4.bak")));
        assert!(!is_likec4_document(Path::new("c4")));
        assert!(is_project_config(Path::new("p/.likec4rc")));
        assert!(is_project_config(Path::new("p/likec4.config.mjs")));
        assert!(!is_project_config(Path::new("p/likec4.json")));
    }

    #[test]
    fn rule_config_accepts_bare_levels_and_tables() {
        let config: LintConfig = toml::from_str(
            "exclude = [\"gen/**\"]\n[rules]\nunused-tag = \"off\"\nnaming-convention = { level = \"warning\", pattern = \"^x\" }\n",
        )
        .expect("valid config");
        assert_eq!(config.exclude, ["gen/**"]);
        assert_eq!(config.rules["unused-tag"], RuleConfig::Level(Level::Off));
        let mut options = BTreeMap::new();
        options.insert("pattern".to_string(), OptionValue::String("^x".into()));
        assert_eq!(
            config.rules["naming-convention"],
            RuleConfig::Detailed { level: Level::Warning, options }
        );
    }

    #[test]
    fn rule_config_rejects_unknown_levels_with_a_readable_message() {
        let expected = "unknown level 'warn' (expected one of: off, info, warning, error)";
        let err = toml::from_str::<LintConfig>("[rules]\nunused-tag = \"warn\"\n").unwrap_err().to_string();
        assert!(err.contains(expected), "{err}");
        let err = toml::from_str::<LintConfig>("[rules]\nunused-tag = { level = \"warn\" }\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains(expected), "{err}");
        let err =
            toml::from_str::<LintConfig>("[rules]\nunused-tag = { level = 1 }\n").unwrap_err().to_string();
        assert!(err.contains("`level` must be a string"), "{err}");
        let err = toml::from_str::<LintConfig>("[rules]\nnaming-convention = { pattern = \"x\" }\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing field `level`"), "{err}");
    }

    #[test]
    fn unknown_config_keys_are_rejected() {
        let err = toml::from_str::<LintConfig>("exclud = [\"x\"]\n").unwrap_err().to_string();
        assert!(err.contains("unknown field `exclud`"), "{err}");
    }
}
