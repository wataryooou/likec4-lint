//! Lint rules. Every rule is a module exposing a `RuleInfo` constant (`INFO`) and a
//! `check(&mut Cx)` function; the [`registry!`] invocation at the bottom is the single
//! source of truth for the rule list (`likec4_lint::rules()` and the runner).

use std::collections::BTreeMap;
use std::path::PathBuf;

use text_size::TextRange;

use crate::model::{DocId, Project, Site, Workspace};
use crate::{Diagnostic, OptionValue, RuleInfo, Severity};

pub mod deprecated_element_predicate;
pub mod duplicate_element;
pub mod duplicate_spec;
pub mod duplicate_view;
pub mod empty_body;
pub mod invalid_color;
pub mod invalid_opacity;
pub mod multiple_specifications;
pub mod naming_convention;
pub mod require_tags;
pub mod require_title;
pub mod reserved_name;
pub mod self_relation;
pub mod syntax_error;
pub mod unknown_custom_color;
pub mod unknown_deployment_node_kind;
pub mod unknown_element_kind;
pub mod unknown_relationship_kind;
pub mod unknown_tag;
pub mod unresolved_reference;
pub mod unused_element_kind;
pub mod unused_relationship_kind;
pub mod unused_tag;
pub mod view_without_rules;

/// Rule-specific options from the configuration (`{ level = "...", key = value }`).
pub(crate) type RuleOptions = BTreeMap<String, OptionValue>;

/// A string option.
pub(crate) fn option_str<'o>(options: &'o RuleOptions, key: &str) -> Option<&'o str> {
    match options.get(key)? {
        OptionValue::String(s) => Some(s.as_str()),
        _ => None,
    }
}

/// A list-of-strings option; a bare string is accepted as a one-element list.
pub(crate) fn option_strings(options: &RuleOptions, key: &str) -> Option<Vec<String>> {
    match options.get(key)? {
        OptionValue::String(s) => Some(vec![s.clone()]),
        OptionValue::Array(items) => Some(
            items
                .iter()
                .filter_map(|v| match v {
                    OptionValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
        ),
        _ => None,
    }
}

/// A finding of a rule, before the configured severity is applied.
struct Finding {
    file: PathBuf,
    range: TextRange,
    message: String,
    help: Option<String>,
}

/// Everything a rule needs: the workspace model, its options and a sink for findings.
pub(crate) struct Cx<'w, 'a> {
    pub ws: &'w Workspace<'a>,
    pub options: &'w RuleOptions,
    findings: Vec<Finding>,
}

impl<'w, 'a> Cx<'w, 'a> {
    pub(crate) fn new(ws: &'w Workspace<'a>, options: &'w RuleOptions) -> Self {
        Cx { ws, options, findings: Vec::new() }
    }

    /// Report a finding at a model site.
    pub(crate) fn report(&mut self, site: Site, message: impl Into<String>, help: Option<String>) {
        self.report_range(site.doc, site.range, message, help);
    }

    /// Report a finding at an arbitrary range of a document.
    pub(crate) fn report_range(
        &mut self,
        doc: DocId,
        range: TextRange,
        message: impl Into<String>,
        help: Option<String>,
    ) {
        let file = self.ws.path(doc).to_path_buf();
        self.findings.push(Finding { file, range, message: message.into(), help });
    }

    /// Report a problem with the rule configuration itself (no file).
    pub(crate) fn report_config(&mut self, message: impl Into<String>, help: Option<String>) {
        self.findings.push(Finding {
            file: PathBuf::new(),
            range: TextRange::empty(0.into()),
            message: message.into(),
            help,
        });
    }

    pub(crate) fn into_diagnostics(self, rule: &'static str, severity: Severity) -> Vec<Diagnostic> {
        self.findings
            .into_iter()
            .map(|f| Diagnostic {
                rule: rule.to_string(),
                severity,
                message: f.message,
                file: f.file,
                range: f.range,
                help: f.help,
            })
            .collect()
    }
}

/// True when `pick` matches the project of `doc`, widened to every project of the
/// workspace when the document imports from another project (conservative).
pub(crate) fn declared(ws: &Workspace<'_>, doc: DocId, pick: impl Fn(&Project) -> bool) -> bool {
    if pick(ws.project_of(doc)) {
        return true;
    }
    ws.documents[doc].has_imports && ws.projects.iter().any(pick)
}

/// Signature shared by all rules.
pub(crate) type RuleFn = fn(&mut Cx<'_, '_>);

macro_rules! registry {
    ($($module:ident),* $(,)?) => {
        /// Metadata of every rule, in registration order.
        pub(crate) static INFOS: &[RuleInfo] = &[$($module::INFO),*];
        /// Check function of every rule, aligned with [`INFOS`].
        pub(crate) static RUNNERS: &[RuleFn] = &[$($module::check),*];
    };
}

registry!(
    syntax_error,
    unknown_element_kind,
    unknown_deployment_node_kind,
    unknown_relationship_kind,
    unknown_tag,
    unknown_custom_color,
    duplicate_element,
    duplicate_view,
    duplicate_spec,
    unresolved_reference,
    self_relation,
    unused_element_kind,
    unused_tag,
    unused_relationship_kind,
    empty_body,
    view_without_rules,
    deprecated_element_predicate,
    reserved_name,
    multiple_specifications,
    invalid_color,
    invalid_opacity,
    naming_convention,
    require_title,
    require_tags,
);

#[cfg(test)]
pub(crate) mod testing {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use crate::{Diagnostic, Level, LintConfig, OptionValue, RuleConfig, SourceFile};

    /// Lint in-memory files with the given project roots and configuration.
    pub fn lint_files(files: &[(&str, &str)], roots: &[&str], config: &LintConfig) -> Vec<Diagnostic> {
        let files: Vec<SourceFile> =
            files.iter().map(|(p, t)| SourceFile { path: PathBuf::from(p), text: t.to_string() }).collect();
        let roots: Vec<PathBuf> = roots.iter().map(PathBuf::from).collect();
        crate::lint(&files, &roots, config)
    }

    /// Lint a single document with the default configuration.
    pub fn lint_one(text: &str) -> Vec<Diagnostic> {
        lint_files(&[("test.c4", text)], &[], &LintConfig::default())
    }

    /// Diagnostics of one rule.
    pub fn of_rule(diags: &[Diagnostic], rule: &str) -> Vec<Diagnostic> {
        diags.iter().filter(|d| d.rule == rule).cloned().collect()
    }

    /// Messages of one rule.
    pub fn messages(diags: &[Diagnostic], rule: &str) -> Vec<String> {
        of_rule(diags, rule).into_iter().map(|d| d.message).collect()
    }

    /// A configuration enabling `rule` at `level` with options.
    pub fn enable(rule: &str, level: Level, options: &[(&str, OptionValue)]) -> LintConfig {
        let options: BTreeMap<String, OptionValue> =
            options.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        let mut config = LintConfig::default();
        config.rules.insert(rule.to_string(), RuleConfig::Detailed { level, options });
        config
    }

    pub fn strings(items: &[&str]) -> OptionValue {
        OptionValue::Array(items.iter().map(|s| OptionValue::String(s.to_string())).collect())
    }
}
