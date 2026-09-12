//! Hand-written lexer reproducing the token order of the Langium/Chevrotain lexer
//! generated from `like-c4.langium`.
//!
//! Terminal priority (first match wins, mirroring Chevrotain):
//! whitespace, keywords (handled as identifiers here), block/line comments, BOOLEAN,
//! LIB_ICON, URI_WITH_SCHEMA, URI_RELATIVE, URI_ALIAS, DotUnderscore, DotWildcard, Hash,
//! StickyDot, Dot, NotEqual, Eq, Percent, MarkdownString, String, Float, Number,
//! IdTerminal, Hex.

use text_size::{TextRange, TextSize};

use crate::kind::SyntaxKind;
use crate::kind::SyntaxKind::*;

/// A token produced by [`tokenize`]: kind and byte length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: SyntaxKind,
    pub len: TextSize,
}

/// A lexing problem (unterminated string/comment, unexpected character).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub range: TextRange,
}

/// Result of lexing a whole document.
#[derive(Clone, Debug, Default)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexError>,
}

/// Tokenize the complete `text`. Every byte ends up in exactly one token.
pub fn tokenize(text: &str) -> Lexed {
    let mut lexer = Lexer { text, bytes: text.as_bytes(), pos: 0, out: Lexed::default() };
    while lexer.pos < lexer.bytes.len() {
        lexer.next_token();
    }
    lexer.out
}

struct Lexer<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
    out: Lexed,
}

#[inline]
fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[inline]
fn is_ident_continue(b: u8) -> bool {
    is_word(b) || b == b'-'
}

#[inline]
fn is_hex(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

impl<'a> Lexer<'a> {
    fn peek(&self, offset: usize) -> u8 {
        self.bytes.get(self.pos + offset).copied().unwrap_or(0)
    }

    fn rest(&self) -> &'a [u8] {
        &self.bytes[self.pos..]
    }

    fn starts_with(&self, s: &[u8]) -> bool {
        self.rest().starts_with(s)
    }

    fn prev_is_word(&self) -> bool {
        self.pos > 0 && is_word(self.bytes[self.pos - 1])
    }

    fn emit(&mut self, kind: SyntaxKind, len: usize) {
        debug_assert!(len > 0, "zero-length token of kind {kind:?}");
        self.out.tokens.push(Token { kind, len: TextSize::new(len as u32) });
        self.pos += len;
    }

    fn error(&mut self, len: usize, message: impl Into<String>) {
        let start = TextSize::new(self.pos as u32);
        let end = TextSize::new((self.pos + len) as u32);
        self.out.errors.push(LexError { message: message.into(), range: TextRange::new(start, end) });
        self.emit(ERROR, len);
    }

    /// Length of the UTF-8 character at the current position.
    fn char_len(&self) -> usize {
        self.text[self.pos..].chars().next().map(char::len_utf8).unwrap_or(1)
    }

    fn next_token(&mut self) {
        let b = self.peek(0);

        // whitespace / newlines
        if b == b' ' || b == b'\t' {
            let len = self.rest().iter().take_while(|&&c| c == b' ' || c == b'\t').count();
            return self.emit(WHITESPACE, len);
        }
        if b == b'\n' || b == b'\r' {
            let len = self.rest().iter().take_while(|&&c| c == b'\n' || c == b'\r').count();
            return self.emit(NEWLINE, len);
        }

        // comments
        if self.starts_with(b"/*") {
            return match find(self.rest(), b"*/", 2) {
                Some(end) => self.emit(BLOCK_COMMENT, end + 2),
                None => {
                    let len = self.rest().len();
                    self.error(len, "unterminated block comment")
                }
            };
        }
        if self.starts_with(b"//") {
            let len = self.rest().iter().take_while(|&&c| c != b'\n' && c != b'\r').count();
            return self.emit(LINE_COMMENT, len);
        }

        // multi-character symbols (keywords in the grammar; longest first)
        for (sym, kind) in [
            (&b"]<->"[..], R_BRACK_BI_ARROW),
            (b"<->", BI_ARROW),
            (b"]->", R_BRACK_ARROW),
            (b"->", ARROW),
            (b"-[", ARROW_L_BRACK),
            (b"<-", BACK_ARROW),
        ] {
            if self.starts_with(sym) {
                return self.emit(kind, sym.len());
            }
        }

        match b {
            b'{' => return self.emit(L_CURLY, 1),
            b'}' => return self.emit(R_CURLY, 1),
            b'(' => return self.emit(L_PAREN, 1),
            b')' => return self.emit(R_PAREN, 1),
            b'[' => return self.emit(L_BRACK, 1),
            b']' => return self.emit(R_BRACK, 1),
            b',' => return self.emit(COMMA, 1),
            b':' => return self.emit(COLON, 1),
            b';' => return self.emit(SEMICOLON, 1),
            b'*' => return self.emit(STAR, 1),
            _ => {}
        }

        // words: BOOLEAN, LIB_ICON, URI_WITH_SCHEMA, IdTerminal (and Hex fallback)
        if b.is_ascii_alphabetic() || b == b'_' {
            if let Some(len) = self.uri_with_schema_len() {
                return self.emit(URI, len);
            }
            if let Some(len) = self.lib_icon_len() {
                return self.emit(LIB_ICON, len);
            }
            if let Some(len) = self.ident_len() {
                let word = &self.rest()[..len];
                // BOOLEAN: /\b(true|false)\b/ is tried before IdTerminal.
                if (word.starts_with(b"true") && !is_word(self.peek(4)))
                    || (word.starts_with(b"false") && !is_word(self.peek(5)))
                {
                    let blen = if word.starts_with(b"true") { 4 } else { 5 };
                    return self.emit(BOOLEAN, blen);
                }
                return self.emit(IDENT, len);
            }
            if let Some(len) = self.hex_len() {
                return self.emit(HEX, len);
            }
            let len = self.rest().iter().take_while(|&&c| is_ident_continue(c)).count().max(1);
            return self.error(len, "unexpected identifier");
        }

        // numbers: Percent, Float, Number, Hex
        if b.is_ascii_digit() {
            // `\b` before the digits: the previous character must not be a word character.
            if !self.prev_is_word() {
                let digits = self.rest().iter().take_while(|c| c.is_ascii_digit()).count();
                if self.peek(digits) == b'%' {
                    return self.emit(PERCENT, digits + 1);
                }
                if self.peek(digits) == b'.' && self.peek(digits + 1).is_ascii_digit() {
                    let frac = self.rest()[digits + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
                    let len = digits + 1 + frac;
                    if !is_word(self.peek(len)) {
                        return self.emit(FLOAT, len);
                    }
                }
                if !is_word(self.peek(digits)) {
                    return self.emit(NUMBER, digits);
                }
            }
            if let Some(len) = self.hex_len() {
                return self.emit(HEX, len);
            }
            let len = self.rest().iter().take_while(|&&c| is_word(c)).count().max(1);
            return self.error(len, "unexpected number");
        }

        match b {
            b'.' => {
                // URI_RELATIVE: /\.{0,2}\/[^\/]\S+/
                if let Some(len) = self.uri_relative_len() {
                    return self.emit(URI, len);
                }
                if self.prev_is_word() {
                    // DotUnderscore: /\b\._(?![_a-zA-Z])/
                    if self.peek(1) == b'_' {
                        let after = self.peek(2);
                        if !(after == b'_' || after.is_ascii_alphabetic()) {
                            return self.emit(DOT_UNDERSCORE, 2);
                        }
                    }
                    // DotWildcard: /\b\.\*{1,2}/
                    if self.peek(1) == b'*' {
                        let len = if self.peek(2) == b'*' { 3 } else { 2 };
                        return self.emit(DOT_WILDCARD, len);
                    }
                    return self.emit(STICKY_DOT, 1);
                }
                self.emit(DOT, 1)
            }
            b'/' => {
                if let Some(len) = self.uri_relative_len() {
                    return self.emit(URI, len);
                }
                self.error(1, "unexpected character '/'")
            }
            b'@' => {
                if let Some(len) = self.uri_alias_len() {
                    return self.emit(URI, len);
                }
                self.error(1, "unexpected character '@'")
            }
            b'#' => self.emit(HASH, 1),
            b'!' => {
                if self.peek(1) == b'=' {
                    let len = if self.peek(2) == b'=' { 3 } else { 2 };
                    return self.emit(NOT_EQUAL, len);
                }
                self.error(1, "unexpected character '!'")
            }
            b'=' => {
                let len = if self.peek(1) == b'=' { 2 } else { 1 };
                self.emit(EQ, len)
            }
            b'\'' | b'"' => self.string_or_markdown(b),
            _ => {
                let len = self.char_len();
                let ch = self.text[self.pos..].chars().next().unwrap_or('?');
                self.error(len, format!("unexpected character {ch:?}"))
            }
        }
    }

    /// `([a-zA-Z]|_+[a-zA-Z0-9])[-\w]*`
    fn ident_len(&self) -> Option<usize> {
        let rest = self.rest();
        let mut i = 0;
        if rest[0].is_ascii_alphabetic() {
            i = 1;
        } else {
            while i < rest.len() && rest[i] == b'_' {
                i += 1;
            }
            if i == 0 || i >= rest.len() || !rest[i].is_ascii_alphanumeric() {
                return None;
            }
            i += 1;
        }
        while i < rest.len() && is_ident_continue(rest[i]) {
            i += 1;
        }
        Some(i)
    }

    /// `[a-fA-F0-9]{3,}(?![-_g-zG-Z])`
    fn hex_len(&self) -> Option<usize> {
        let rest = self.rest();
        let n = rest.iter().take_while(|&&c| is_hex(c)).count();
        if n < 3 {
            return None;
        }
        let after = rest.get(n).copied().unwrap_or(0);
        if after == b'-' || after == b'_' || after.is_ascii_alphabetic() {
            return None;
        }
        Some(n)
    }

    /// `(aws|azure|bootstrap|gcp|tech):[-\w]*`
    fn lib_icon_len(&self) -> Option<usize> {
        let rest = self.rest();
        for prefix in [&b"aws:"[..], b"azure:", b"bootstrap:", b"gcp:", b"tech:"] {
            if rest.starts_with(prefix) {
                let tail = rest[prefix.len()..].iter().take_while(|&&c| is_ident_continue(c)).count();
                return Some(prefix.len() + tail);
            }
        }
        None
    }

    /// `\w+:\/{2}\S+`
    fn uri_with_schema_len(&self) -> Option<usize> {
        let rest = self.rest();
        let w = rest.iter().take_while(|&&c| is_word(c)).count();
        if w == 0 || !rest[w..].starts_with(b"://") {
            return None;
        }
        let tail = non_space_len(&rest[w + 3..]);
        if tail == 0 {
            return None;
        }
        Some(w + 3 + tail)
    }

    /// `\.{0,2}\/[^\/]\S+`
    fn uri_relative_len(&self) -> Option<usize> {
        let rest = self.rest();
        let dots = rest.iter().take_while(|&&c| c == b'.').count().min(2);
        if rest.get(dots) != Some(&b'/') {
            return None;
        }
        let first = rest.get(dots + 1).copied()?;
        if first == b'/' || first.is_ascii_whitespace() {
            return None;
        }
        let tail = non_space_len(&rest[dots + 1..]);
        if tail < 2 {
            return None;
        }
        Some(dots + 1 + tail)
    }

    /// `@[a-zA-Z0-9_-]*\/[^\s]+`
    fn uri_alias_len(&self) -> Option<usize> {
        let rest = self.rest();
        let name = rest[1..].iter().take_while(|&&c| is_ident_continue(c)).count();
        if rest.get(1 + name) != Some(&b'/') {
            return None;
        }
        let tail = non_space_len(&rest[1 + name + 1..]);
        if tail == 0 {
            return None;
        }
        Some(1 + name + 1 + tail)
    }

    fn string_or_markdown(&mut self, quote: u8) {
        let fence = [quote; 3];
        if self.starts_with(&fence) {
            match find(self.rest(), &fence, 3) {
                Some(end) => return self.emit(MARKDOWN_STRING, end + 3),
                None => {
                    // Chevrotain would fall back to a plain string; try that first.
                }
            }
        }
        // `"(?:[^"\\]|\\.)*"` — an escape is a backslash plus any character except a line
        // terminator (`.` in a JavaScript regex never matches `\n`, `\r`, U+2028 or U+2029),
        // so a backslash directly before a line break (or at EOF) ends the match: no STRING.
        let rest = self.rest();
        let mut i = 1;
        while i < rest.len() {
            match rest[i] {
                b'\\' => {
                    if i + 1 >= rest.len() || line_terminator_len(&rest[i + 1..]) > 0 {
                        break;
                    }
                    i += 2;
                }
                c if c == quote => return self.emit(STRING, i + 1),
                _ => i += 1,
            }
        }
        let len = line_len(rest).max(1);
        self.error(len, "unterminated string literal");
    }
}

/// Length of the line terminator at the start of `bytes` (`\n`, `\r`, U+2028, U+2029), else 0.
fn line_terminator_len(bytes: &[u8]) -> usize {
    match bytes {
        [b'\n', ..] | [b'\r', ..] => 1,
        [0xE2, 0x80, 0xA8, ..] | [0xE2, 0x80, 0xA9, ..] => 3,
        _ => 0,
    }
}

/// Number of bytes before the first line terminator (see [`line_terminator_len`]).
fn line_len(bytes: &[u8]) -> usize {
    let mut i = 0;
    while i < bytes.len() && line_terminator_len(&bytes[i..]) == 0 {
        i += 1;
    }
    i
}

fn non_space_len(bytes: &[u8]) -> usize {
    bytes.iter().take_while(|&&c| !c.is_ascii_whitespace()).count()
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if haystack.len() < from + needle.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(SyntaxKind, &str)> {
        let lexed = tokenize(text);
        let mut pos = 0usize;
        lexed
            .tokens
            .iter()
            .map(|t| {
                let len: usize = t.len.into();
                let s = &text[pos..pos + len];
                pos += len;
                (t.kind, s)
            })
            .collect()
    }

    #[test]
    fn lossless() {
        let text = "model {\n  a = system 'x' // c\n  a -> b.c\n}\n";
        let lexed = tokenize(text);
        let total: usize = lexed.tokens.iter().map(|t| usize::from(t.len)).sum();
        assert_eq!(total, text.len());
        assert!(lexed.errors.is_empty());
    }

    #[test]
    fn identifiers_and_keywords() {
        assert_eq!(
            kinds("model view-with-dash _x __1 a1"),
            vec![
                (IDENT, "model"),
                (WHITESPACE, " "),
                (IDENT, "view-with-dash"),
                (WHITESPACE, " "),
                (IDENT, "_x"),
                (WHITESPACE, " "),
                (IDENT, "__1"),
                (WHITESPACE, " "),
                (IDENT, "a1"),
            ]
        );
    }

    #[test]
    fn arrow_without_spaces_is_greedy_identifier() {
        // Same behaviour as Langium: `a-` is an identifier, `>` is an error.
        let k = kinds("a->b");
        assert_eq!(k[0], (IDENT, "a-"));
        assert_eq!(k[1].0, ERROR);
    }

    #[test]
    fn dots() {
        assert_eq!(
            kinds("a.b .uses c._ d.* e.** f._x"),
            vec![
                (IDENT, "a"),
                (STICKY_DOT, "."),
                (IDENT, "b"),
                (WHITESPACE, " "),
                (DOT, "."),
                (IDENT, "uses"),
                (WHITESPACE, " "),
                (IDENT, "c"),
                (DOT_UNDERSCORE, "._"),
                (WHITESPACE, " "),
                (IDENT, "d"),
                (DOT_WILDCARD, ".*"),
                (WHITESPACE, " "),
                (IDENT, "e"),
                (DOT_WILDCARD, ".**"),
                (WHITESPACE, " "),
                (IDENT, "f"),
                (STICKY_DOT, "."),
                (IDENT, "_x"),
            ]
        );
    }

    #[test]
    fn symbols() {
        assert_eq!(
            kinds("-> <- <-> -[ ]-> ]<-> = == != !== { } ( ) [ ] , : ; * #"),
            vec![
                (ARROW, "->"),
                (WHITESPACE, " "),
                (BACK_ARROW, "<-"),
                (WHITESPACE, " "),
                (BI_ARROW, "<->"),
                (WHITESPACE, " "),
                (ARROW_L_BRACK, "-["),
                (WHITESPACE, " "),
                (R_BRACK_ARROW, "]->"),
                (WHITESPACE, " "),
                (R_BRACK_BI_ARROW, "]<->"),
                (WHITESPACE, " "),
                (EQ, "="),
                (WHITESPACE, " "),
                (EQ, "=="),
                (WHITESPACE, " "),
                (NOT_EQUAL, "!="),
                (WHITESPACE, " "),
                (NOT_EQUAL, "!=="),
                (WHITESPACE, " "),
                (L_CURLY, "{"),
                (WHITESPACE, " "),
                (R_CURLY, "}"),
                (WHITESPACE, " "),
                (L_PAREN, "("),
                (WHITESPACE, " "),
                (R_PAREN, ")"),
                (WHITESPACE, " "),
                (L_BRACK, "["),
                (WHITESPACE, " "),
                (R_BRACK, "]"),
                (WHITESPACE, " "),
                (COMMA, ","),
                (WHITESPACE, " "),
                (COLON, ":"),
                (WHITESPACE, " "),
                (SEMICOLON, ";"),
                (WHITESPACE, " "),
                (STAR, "*"),
                (WHITESPACE, " "),
                (HASH, "#"),
            ]
        );
    }

    #[test]
    fn numbers_and_colors() {
        assert_eq!(
            kinds("10 1.5 30% #fff #1a2b3c #123456 #abcdef 10px"),
            vec![
                (NUMBER, "10"),
                (WHITESPACE, " "),
                (FLOAT, "1.5"),
                (WHITESPACE, " "),
                (PERCENT, "30%"),
                (WHITESPACE, " "),
                (HASH, "#"),
                (IDENT, "fff"),
                (WHITESPACE, " "),
                (HASH, "#"),
                (HEX, "1a2b3c"),
                (WHITESPACE, " "),
                (HASH, "#"),
                (NUMBER, "123456"),
                (WHITESPACE, " "),
                (HASH, "#"),
                (IDENT, "abcdef"),
                (WHITESPACE, " "),
                (ERROR, "10px"),
            ]
        );
    }

    #[test]
    fn strings() {
        assert_eq!(
            kinds(
                r#"'a' "b\"c" '''md''' """x\ny""" 'multi
line' ''"#
            ),
            vec![
                (STRING, "'a'"),
                (WHITESPACE, " "),
                (STRING, r#""b\"c""#),
                (WHITESPACE, " "),
                (MARKDOWN_STRING, "'''md'''"),
                (WHITESPACE, " "),
                (MARKDOWN_STRING, "\"\"\"x\\ny\"\"\""),
                (WHITESPACE, " "),
                (STRING, "'multi\nline'"),
                (WHITESPACE, " "),
                (STRING, "''"),
            ]
        );
        let lexed = tokenize("'unterminated\nmodel");
        assert_eq!(lexed.tokens[0].kind, ERROR);
        assert_eq!(lexed.errors.len(), 1);
    }

    #[test]
    fn backslash_before_line_break_does_not_escape() {
        // Langium: `"(?:[^"\\]|\\.)*"` — `.` never matches a line terminator, so a backslash
        // directly before a newline ends the STRING match and the token becomes a lexer error.
        for (text, first) in [
            ("\"a\\\nb\"", "\"a\\"),
            ("'a\\\r\nb'", "'a\\"),
            ("\"a\\\u{2028}b\"", "\"a\\"),
            ("\"a\\\u{2029}b\"", "\"a\\"),
        ] {
            let k = kinds(text);
            assert!(!k.contains(&(STRING, text)), "{text:?} must not lex as one STRING: {k:?}");
            assert_eq!(k[0], (ERROR, first), "{text:?}");
            let total: usize = tokenize(text).tokens.iter().map(|t| usize::from(t.len)).sum();
            assert_eq!(total, text.len());
        }
        // A backslash at the very end of the input is not an escape either.
        assert_eq!(kinds("\"a\\")[0].0, ERROR);
        // Escaped quotes and other escapes are still fine, and raw newlines are allowed.
        assert_eq!(kinds("\"a\\\"b\\\\\"")[0], (STRING, "\"a\\\"b\\\\\""));
        assert_eq!(kinds("'a\nb'")[0], (STRING, "'a\nb'"));
    }

    #[test]
    fn uris_icons_booleans() {
        assert_eq!(
            kinds("https://a.b/c ./x.md ../yy /zz @scope/pkg aws:person tech:foo true false trueish"),
            vec![
                (URI, "https://a.b/c"),
                (WHITESPACE, " "),
                (URI, "./x.md"),
                (WHITESPACE, " "),
                (URI, "../yy"),
                (WHITESPACE, " "),
                (URI, "/zz"),
                (WHITESPACE, " "),
                (URI, "@scope/pkg"),
                (WHITESPACE, " "),
                (LIB_ICON, "aws:person"),
                (WHITESPACE, " "),
                (LIB_ICON, "tech:foo"),
                (WHITESPACE, " "),
                (BOOLEAN, "true"),
                (WHITESPACE, " "),
                (BOOLEAN, "false"),
                (WHITESPACE, " "),
                (IDENT, "trueish"),
            ]
        );
    }

    #[test]
    fn comments_and_newlines() {
        assert_eq!(
            kinds("a // line\r\n/* block\n */b"),
            vec![
                (IDENT, "a"),
                (WHITESPACE, " "),
                (LINE_COMMENT, "// line"),
                (NEWLINE, "\r\n"),
                (BLOCK_COMMENT, "/* block\n */"),
                (IDENT, "b"),
            ]
        );
    }

    #[test]
    fn unicode_outside_strings_is_error_but_lossless() {
        let text = "a 日本 'ünï'";
        let lexed = tokenize(text);
        let total: usize = lexed.tokens.iter().map(|t| usize::from(t.len)).sum();
        assert_eq!(total, text.len());
        assert!(lexed.tokens.iter().any(|t| t.kind == ERROR));
        assert_eq!(lexed.tokens.last().unwrap().kind, STRING);
    }
}
