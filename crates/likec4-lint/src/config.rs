//! Resolution of `[lint.rules]` overrides against the rule registry.

use std::path::PathBuf;

use text_size::TextRange;

use crate::rules::{self, RuleOptions};
use crate::{Diagnostic, Level, LintConfig, RuleConfig, Severity};

/// A rule that is switched on, with its effective severity and options.
pub(crate) struct EnabledRule {
    /// Index into `rules::INFOS` / `rules::RUNNERS`.
    pub index: usize,
    pub severity: Severity,
    pub options: RuleOptions,
}

/// Result of resolving a configuration.
pub(crate) struct ResolvedConfig {
    pub enabled: Vec<EnabledRule>,
    /// `unknown-rule` diagnostics for ids that do not exist.
    pub diagnostics: Vec<Diagnostic>,
}

/// Severity of a level; `None` switches the rule off.
pub(crate) fn severity_of(level: Level) -> Option<Severity> {
    match level {
        Level::Off => None,
        Level::Info => Some(Severity::Info),
        Level::Warning => Some(Severity::Warning),
        Level::Error => Some(Severity::Error),
    }
}

/// Apply `config` to the registry: defaults, level overrides and options.
pub(crate) fn resolve(config: &LintConfig) -> ResolvedConfig {
    let mut enabled = Vec::with_capacity(rules::INFOS.len());
    for (index, info) in rules::INFOS.iter().enumerate() {
        let (level, options) = match config.rules.get(info.id) {
            None => (info.default_level, RuleOptions::new()),
            Some(RuleConfig::Level(level)) => (*level, RuleOptions::new()),
            Some(RuleConfig::Detailed { level, options }) => (*level, options.clone()),
        };
        if let Some(severity) = severity_of(level) {
            enabled.push(EnabledRule { index, severity, options });
        }
    }
    let diagnostics = config
        .rules
        .keys()
        .filter(|id| !rules::INFOS.iter().any(|info| info.id == id.as_str()))
        .map(|id| Diagnostic {
            rule: "unknown-rule".to_string(),
            severity: Severity::Warning,
            message: format!("Unknown rule '{id}' in configuration"),
            file: PathBuf::new(),
            range: TextRange::empty(0.into()),
            help: Some("run `likec4-lint lint --list-rules` to see the known rule ids".to_string()),
        })
        .collect();
    ResolvedConfig { enabled, diagnostics }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::OptionValue;

    fn index_of(id: &str) -> usize {
        rules::INFOS.iter().position(|i| i.id == id).expect("known rule")
    }

    #[test]
    fn defaults_enable_on_rules_only() {
        let resolved = resolve(&LintConfig::default());
        let enabled: Vec<&str> = resolved.enabled.iter().map(|r| rules::INFOS[r.index].id).collect();
        assert!(enabled.contains(&"syntax-error"));
        assert!(!enabled.contains(&"naming-convention"));
        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn level_overrides_and_off() {
        let mut config = LintConfig::default();
        config.rules.insert("unused-tag".into(), RuleConfig::Level(Level::Off));
        config.rules.insert("self-relation".into(), RuleConfig::Level(Level::Error));
        let resolved = resolve(&config);
        assert!(!resolved.enabled.iter().any(|r| r.index == index_of("unused-tag")));
        let self_relation =
            resolved.enabled.iter().find(|r| r.index == index_of("self-relation")).expect("on");
        assert_eq!(self_relation.severity, Severity::Error);
    }

    #[test]
    fn detailed_config_carries_options() {
        let mut config = LintConfig::default();
        let mut options = BTreeMap::new();
        options.insert("pattern".to_string(), OptionValue::String("^x".into()));
        config
            .rules
            .insert("naming-convention".into(), RuleConfig::Detailed { level: Level::Warning, options });
        let resolved = resolve(&config);
        let rule = resolved.enabled.iter().find(|r| r.index == index_of("naming-convention")).expect("on");
        assert_eq!(rule.severity, Severity::Warning);
        assert_eq!(rule.options.get("pattern"), Some(&OptionValue::String("^x".into())));
    }

    #[test]
    fn unknown_rule_produces_diagnostic() {
        let mut config = LintConfig::default();
        config.rules.insert("no-such-rule".into(), RuleConfig::Level(Level::Error));
        let resolved = resolve(&config);
        assert_eq!(resolved.diagnostics.len(), 1);
        let diag = &resolved.diagnostics[0];
        assert_eq!(diag.rule, "unknown-rule");
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.file, PathBuf::new());
        assert!(diag.message.contains("no-such-rule"));
    }
}
