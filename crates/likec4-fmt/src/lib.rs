//! Formatter for the LikeC4 DSL.
//!
//! The output is byte-compatible with the official `likec4 format` command
//! (Langium `AbstractFormatter` engine + `LikeC4Formatter` rules). See `docs/DESIGN.md`.

pub mod engine;
pub mod quotes;
pub mod rules;

use likec4_syntax::SyntaxError;

/// Quote style for string literals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QuoteStyle {
    /// Pick the style used by the majority of strings in the document (ties → double).
    #[default]
    Auto,
    Single,
    Double,
    /// Leave quotes untouched.
    Ignore,
}

impl std::str::FromStr for QuoteStyle {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "auto" => Ok(QuoteStyle::Auto),
            "single" => Ok(QuoteStyle::Single),
            "double" => Ok(QuoteStyle::Double),
            "ignore" => Ok(QuoteStyle::Ignore),
            other => Err(format!("unknown quote style '{other}' (expected auto, single, double or ignore)")),
        }
    }
}

/// Formatting options (defaults match the official CLI).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatOptions {
    /// Width of one indentation level when `insert_spaces` is true (default 2).
    pub indent_width: usize,
    /// Indent with spaces (default) or tabs.
    pub insert_spaces: bool,
    pub quote_style: QuoteStyle,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions { indent_width: 2, insert_spaces: true, quote_style: QuoteStyle::Auto }
    }
}

/// Why a document could not be formatted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    /// The document has syntax errors; the official formatter refuses to format it.
    SyntaxErrors(Vec<SyntaxError>),
    /// The input parsed, but the formatted text does not: the formatter would have produced
    /// a document with syntax errors. `formatted` is the rejected output, `errors` are the
    /// errors found in it (ranges refer to `formatted`).
    Unstable { errors: Vec<SyntaxError>, formatted: String },
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::SyntaxErrors(errors) => {
                write!(f, "document has {} syntax error(s) and was not formatted", errors.len())
            }
            FormatError::Unstable { errors, .. } => write!(
                f,
                "formatting would produce a document with {} syntax error(s); the output was discarded",
                errors.len()
            ),
        }
    }
}

impl std::error::Error for FormatError {}

/// Format a LikeC4 document. Returns the formatted text (which may be identical to the input).
///
/// The result is re-parsed before it is returned: a formatting pass that would turn a valid
/// document into an invalid one yields [`FormatError::Unstable`] instead of the broken text.
pub fn format(text: &str, options: &FormatOptions) -> Result<String, FormatError> {
    let parse = likec4_syntax::parse(text);
    if !parse.ok() {
        return Err(FormatError::SyntaxErrors(parse.errors().to_vec()));
    }
    let formatted = format_parsed(&parse.syntax(), text, options);
    let check = likec4_syntax::parse(&formatted);
    if !check.ok() {
        return Err(FormatError::Unstable { errors: check.errors().to_vec(), formatted });
    }
    Ok(formatted)
}

/// Format an already parsed, error-free document.
pub fn format_parsed(root: &likec4_syntax::SyntaxNode, text: &str, options: &FormatOptions) -> String {
    let lines = engine::LineIndex::new(text);
    let mut collector = engine::Collector::default();
    rules::collect(root, &lines, &mut collector);
    let mut edits = engine::compute_edits(root, text, options, collector);
    edits.extend(quotes::normalize(root, options.quote_style));
    engine::apply_edits(text, edits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use likec4_syntax::Range;

    #[test]
    fn format_returns_the_formatted_text_for_valid_input() {
        let input = "model {\na = system\n}\n";
        assert_eq!(format(input, &FormatOptions::default()).unwrap(), "model {\n  a = system\n}\n");
    }

    #[test]
    fn format_reports_syntax_errors_without_formatting() {
        let err = format("model {", &FormatOptions::default()).unwrap_err();
        assert!(matches!(err, FormatError::SyntaxErrors(_)), "{err:?}");
    }

    #[test]
    fn unstable_display_counts_errors_and_hides_the_output() {
        let err = FormatError::Unstable {
            errors: vec![SyntaxError { message: "unterminated".into(), range: Range::empty(0.into()) }],
            formatted: "REJECTED OUTPUT".into(),
        };
        let text = err.to_string();
        assert!(text.contains("1 syntax error(s)"), "{text}");
        assert!(!text.contains("REJECTED OUTPUT"), "{text}");
    }
}
