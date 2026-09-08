//! Lossless lexer, parser and syntax tree for the LikeC4 DSL.
//!
//! ```
//! let parse = likec4_syntax::parse("model {\n  sys = system 'System'\n}\n");
//! assert!(parse.ok());
//! assert_eq!(parse.syntax().text().to_string(), "model {\n  sys = system 'System'\n}\n");
//! ```

pub mod ast;
pub mod kind;
pub mod lexer;
mod parser;

use text_size::TextRange;

pub use kind::{
    is_valid_id, is_valid_id_terminal, LikeC4Language, SyntaxElement, SyntaxElementChildren, SyntaxKind,
    SyntaxNode, SyntaxNodeChildren, SyntaxToken, ALL_KEYWORDS, HARD_RESERVED_WORDS, SOFT_RESERVED_WORDS,
};
pub use rowan::{Direction, GreenNode, NodeOrToken, TextSize, WalkEvent};
pub use text_size::TextRange as Range;

/// A syntax (lexer or parser) diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError {
    pub message: String,
    pub range: TextRange,
}

/// Result of parsing a document. Parsing never fails; errors are collected.
#[derive(Clone, Debug)]
pub struct Parse {
    green: GreenNode,
    errors: Vec<SyntaxError>,
}

impl Parse {
    /// Root node of the tree (`SyntaxKind::ROOT`).
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// Green tree.
    pub fn green(&self) -> &GreenNode {
        &self.green
    }

    /// All syntax errors, sorted by position.
    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    /// True when the document parsed without any error.
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Parse a LikeC4 document.
pub fn parse(text: &str) -> Parse {
    let (green, errors) = parser::parse_text(text);
    Parse { green, errors }
}

/// Render the tree in a compact, indented debug format (used by tests).
pub fn debug_tree(node: &SyntaxNode) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for event in node.preorder_with_tokens() {
        match event {
            WalkEvent::Enter(el) => {
                let indent = "  ".repeat(depth);
                match &el {
                    NodeOrToken::Node(n) => {
                        out.push_str(&format!("{indent}{:?}@{:?}\n", n.kind(), n.text_range()));
                        depth += 1;
                    }
                    NodeOrToken::Token(t) => {
                        out.push_str(&format!(
                            "{indent}{:?}@{:?} {:?}\n",
                            t.kind(),
                            t.text_range(),
                            t.text()
                        ));
                    }
                }
            }
            WalkEvent::Leave(NodeOrToken::Node(_)) => depth -= 1,
            WalkEvent::Leave(_) => {}
        }
    }
    out
}
