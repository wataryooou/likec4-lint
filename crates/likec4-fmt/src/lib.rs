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
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::SyntaxErrors(errors) => {
                write!(f, "document has {} syntax error(s) and was not formatted", errors.len())
            }
        }
    }
}

impl std::error::Error for FormatError {}

/// Format a LikeC4 document. Returns the formatted text (which may be identical to the input).
pub fn format(text: &str, options: &FormatOptions) -> Result<String, FormatError> {
    let parse = likec4_syntax::parse(text);
    if !parse.ok() {
        return Err(FormatError::SyntaxErrors(parse.errors().to_vec()));
    }
    Ok(format_parsed(&parse.syntax(), text, options))
}

/// Format an already parsed, error-free document.
pub fn format_parsed(root: &likec4_syntax::SyntaxNode, text: &str, options: &FormatOptions) -> String {
    let lines = engine::LineIndex::new(text);
    let mut collector = engine::Collector::default();
    rules::collect(root, &lines, &mut collector);
    let mut edits = engine::compute_edits(root, text, options, collector);
    edits.extend(quotes::normalize(root, text, options.quote_style));
    engine::apply_edits(text, edits)
}
