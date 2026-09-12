//! Resolution of `[lint.rules]` overrides against the rule registry.

use std::path::PathBuf;

use text_size::TextRange;

use crate::rules::{self, RuleOptions};
use crate::{Diagnostic, Level, LintConfig, RuleConfig, RuleInfo, Severity};

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
    /// `unknown-rule` diagnostics for ids that do not exist and `unknown-rule-option`
    /// diagnostics for options a rule does not declare.
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

/// A warning about the configuration itself: no file, empty range.
fn config_diagnostic(rule: &str, message: String, help: String) -> Diagnostic {
    Diagnostic {
        rule: rule.to_string(),
        severity: Severity::Warning,
        message,
        file: PathBuf::new(),
        range: TextRange::empty(0.into()),
        help: Some(help),
        related: None,
    }
}

fn unknown_option(info: &RuleInfo, name: &str) -> Diagnostic {
    let help = if info.options.is_empty() {
        format!("rule '{}' takes no options", info.id)
    } else {
        format!("known options of '{}': {}", info.id, info.options.join(", "))
    };
    config_diagnostic("unknown-rule-option", format!("Unknown option '{name}' for rule '{}'", info.id), help)
}

/// Apply `config` to the registry: defaults, level overrides and options.
pub(crate) fn resolve(config: &LintConfig) -> ResolvedConfig {
    let mut enabled = Vec::with_capacity(rules::INFOS.len());
    let mut diagnostics = Vec::new();
    for (index, info) in rules::INFOS.iter().enumerate() {
        let (level, options) = match config.rules.get(info.id) {
            None => (info.default_level, RuleOptions::new()),
            Some(RuleConfig::Level(level)) => (*level, RuleOptions::new()),
            Some(RuleConfig::Detailed { level, options }) => (*level, options.clone()),
        };
        for name in options.keys().filter(|name| !info.options.contains(&name.as_str())) {
            diagnostics.push(unknown_option(info, name));
        }
        if let Some(severity) = severity_of(level) {
            enabled.push(EnabledRule { index, severity, options });
        }
    }
    diagnostics.extend(
        config.rules.keys().filter(|id| !rules::INFOS.iter().any(|info| info.id == id.as_str())).map(|id| {
            config_diagnostic(
                "unknown-rule",
                format!("Unknown rule '{id}' in configuration"),
                "run `likec4-lint lint --list-rules` to see the known rule ids".to_string(),
            )
        }),
    );
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

    fn detailed(rule: &str, level: Level, options: &[(&str, OptionValue)]) -> LintConfig {
        let options: BTreeMap<String, OptionValue> =
            options.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        let mut config = LintConfig::default();
        config.rules.insert(rule.to_string(), RuleConfig::Detailed { level, options });
        config
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
    fn detailed_config_carries_declared_options() {
        let config =
            detailed("naming-convention", Level::Warning, &[("pattern", OptionValue::String("^x".into()))]);
        let resolved = resolve(&config);
        let rule = resolved.enabled.iter().find(|r| r.index == index_of("naming-convention")).expect("on");
        assert_eq!(rule.severity, Severity::Warning);
        assert_eq!(rule.options.get("pattern"), Some(&OptionValue::String("^x".into())));
        assert!(resolved.diagnostics.is_empty());
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

    #[test]
    fn unknown_rule_option_produces_diagnostic() {
        let config =
            detailed("naming-convention", Level::Warning, &[("patern", OptionValue::String("^x".into()))]);
        let resolved = resolve(&config);
        assert_eq!(resolved.diagnostics.len(), 1);
        let diag = &resolved.diagnostics[0];
        assert_eq!(diag.rule, "unknown-rule-option");
        assert_eq!(diag.severity, Severity::Warning);
        assert_eq!(diag.file, PathBuf::new());
        assert_eq!(diag.message, "Unknown option 'patern' for rule 'naming-convention'");
        assert_eq!(diag.help.as_deref(), Some("known options of 'naming-convention': pattern, targets"));

        let config = detailed("self-relation", Level::Off, &[("strict", OptionValue::Bool(true))]);
        let resolved = resolve(&config);
        assert_eq!(resolved.diagnostics[0].message, "Unknown option 'strict' for rule 'self-relation'");
        assert_eq!(resolved.diagnostics[0].help.as_deref(), Some("rule 'self-relation' takes no options"));
    }
}
