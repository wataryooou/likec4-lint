//! Parser infrastructure: token cursor, markers, events and tree construction.
//!
//! Grammar rules live in [`grammar`]. The design follows rust-analyzer: the parser
//! produces a flat list of events which are replayed into a `rowan::GreenNodeBuilder`.
//! Trivia is attached the way Langium attaches hidden tokens (see `docs/DESIGN.md`).

pub(crate) mod grammar;
#[cfg(test)]
mod tests;

use rowan::{GreenNode, GreenNodeBuilder};
use text_size::{TextRange, TextSize};

use crate::kind::SyntaxKind::{self, *};
use crate::lexer::{Lexed, Token};
use crate::SyntaxError;

#[derive(Clone, Copy, Debug)]
struct RawToken {
    kind: SyntaxKind,
    range: TextRange,
}

#[derive(Debug)]
enum Event {
    /// `kind == None` is a tombstone (abandoned marker).
    Start {
        kind: Option<SyntaxKind>,
        forward_parent: Option<u32>,
    },
    Finish,
    Token {
        kind: SyntaxKind,
    },
}

/// Parse `text` into a green tree plus diagnostics.
pub(crate) fn parse_text(text: &str) -> (GreenNode, Vec<SyntaxError>) {
    let lexed = crate::lexer::tokenize(text);
    let mut parser = Parser::new(text, &lexed);
    grammar::root(&mut parser);
    let Parser { tokens, events, mut errors, .. } = parser;
    errors.extend(lexed.errors.iter().map(|e| SyntaxError { message: e.message.clone(), range: e.range }));
    errors.sort_by_key(|e| (e.range.start(), e.range.end()));
    let green = build_tree(text, &tokens, events);
    (green, errors)
}

/// Maximum number of lookahead / bump operations without progress before we bail out
/// of a loop. Protects against accidental infinite loops in grammar code.
const STEP_LIMIT: u32 = 10_000_000;

pub(crate) struct Parser<'t> {
    text: &'t str,
    tokens: Vec<RawToken>,
    /// Indices into `tokens` of the non-trivia tokens.
    non_trivia: Vec<u32>,
    /// Index into `non_trivia` of the current token.
    pos: usize,
    events: Vec<Event>,
    errors: Vec<SyntaxError>,
    steps: u32,
}

#[must_use = "a marker must be completed or abandoned"]
pub(crate) struct Marker {
    pos: u32,
    completed: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CompletedMarker {
    start_pos: u32,
    #[allow(dead_code)] // read through `kind()`, kept for grammar helpers
    kind: SyntaxKind,
}

impl<'t> Parser<'t> {
    fn new(text: &'t str, lexed: &Lexed) -> Self {
        let mut tokens = Vec::with_capacity(lexed.tokens.len());
        let mut non_trivia = Vec::with_capacity(lexed.tokens.len());
        let mut offset = TextSize::new(0);
        for (i, Token { kind, len }) in lexed.tokens.iter().copied().enumerate() {
            let range = TextRange::at(offset, len);
            offset += len;
            tokens.push(RawToken { kind, range });
            if !kind.is_trivia() {
                non_trivia.push(i as u32);
            }
        }
        Parser { text, tokens, non_trivia, pos: 0, events: Vec::new(), errors: Vec::new(), steps: 0 }
    }

    // ----- lookahead -----

    fn raw(&self, n: usize) -> Option<RawToken> {
        self.non_trivia.get(self.pos + n).map(|&i| self.tokens[i as usize])
    }

    /// Kind of the `n`-th non-trivia token ahead (`ERROR`-free `EOF` is reported as `__LAST`).
    pub(crate) fn nth(&self, n: usize) -> SyntaxKind {
        self.raw(n).map(|t| t.kind).unwrap_or(__LAST)
    }

    /// Source text of the `n`-th token ahead (empty at EOF).
    pub(crate) fn nth_text(&self, n: usize) -> &'t str {
        self.raw(n).map(|t| &self.text[t.range]).unwrap_or("")
    }

    pub(crate) fn current(&self) -> SyntaxKind {
        self.nth(0)
    }

    pub(crate) fn current_text(&self) -> &'t str {
        self.nth_text(0)
    }

    pub(crate) fn at(&self, kind: SyntaxKind) -> bool {
        self.nth(0) == kind
    }

    pub(crate) fn nth_at(&self, n: usize, kind: SyntaxKind) -> bool {
        self.nth(n) == kind
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.pos >= self.non_trivia.len()
    }

    /// True when the current token is an identifier with exactly this text.
    pub(crate) fn at_kw(&self, kw: &str) -> bool {
        self.nth_at_kw(0, kw)
    }

    pub(crate) fn nth_at_kw(&self, n: usize, kw: &str) -> bool {
        self.nth(n) == IDENT && self.nth_text(n) == kw
    }

    /// True when the current token is an identifier usable as `Id`.
    pub(crate) fn at_id(&self) -> bool {
        self.nth_at_id(0)
    }

    pub(crate) fn nth_at_id(&self, n: usize) -> bool {
        self.nth(n) == IDENT && crate::kind::is_valid_id(self.nth_text(n))
    }

    /// True when the current token is an identifier usable as `IdTerminal`.
    pub(crate) fn at_id_terminal(&self) -> bool {
        self.at(IDENT) && crate::kind::is_valid_id_terminal(self.current_text())
    }

    /// True when a newline (or the start of the file) precedes the current token.
    pub(crate) fn at_line_start(&self) -> bool {
        let Some(&idx) = self.non_trivia.get(self.pos) else { return true };
        let prev = self.non_trivia.get(self.pos.wrapping_sub(1)).copied();
        let from = match prev {
            Some(p) if self.pos > 0 => p as usize + 1,
            _ => 0,
        };
        self.tokens[from..idx as usize].iter().any(|t| t.kind == NEWLINE)
    }

    /// Range of the current token (empty range at EOF).
    pub(crate) fn current_range(&self) -> TextRange {
        match self.raw(0) {
            Some(t) => t.range,
            None => {
                let end = TextSize::new(self.text.len() as u32);
                TextRange::empty(end)
            }
        }
    }

    /// Range of the previously consumed token (empty range at start).
    #[allow(dead_code)] // part of the parser toolkit, not needed by the current grammar
    pub(crate) fn prev_range(&self) -> TextRange {
        if self.pos == 0 {
            return TextRange::empty(TextSize::new(0));
        }
        let idx = self.non_trivia[self.pos - 1];
        self.tokens[idx as usize].range
    }

    // ----- consuming -----

    fn do_bump(&mut self, kind: SyntaxKind) {
        assert!(!self.at_eof(), "bump at EOF");
        self.events.push(Event::Token { kind });
        self.pos += 1;
        self.steps = 0;
    }

    /// Consume the current token with its lexed kind.
    pub(crate) fn bump_any(&mut self) {
        let kind = self.current();
        self.do_bump(kind);
    }

    /// Consume the current token, asserting its kind.
    pub(crate) fn bump(&mut self, kind: SyntaxKind) {
        assert!(self.at(kind), "expected {kind:?}, found {:?} ({:?})", self.current(), self.current_text());
        self.do_bump(kind);
    }

    /// Consume the current token, remapping its kind (used for keywords).
    #[allow(dead_code)] // part of the parser toolkit, not needed by the current grammar
    pub(crate) fn bump_as(&mut self, kind: SyntaxKind) {
        self.do_bump(kind);
    }

    /// Consume the current identifier as the keyword `kw`, asserting it matches.
    pub(crate) fn bump_kw(&mut self, kw: &str) {
        assert!(self.at_kw(kw), "expected keyword {kw:?}, found {:?}", self.current_text());
        let kind = SyntaxKind::from_keyword(kw).unwrap_or_else(|| panic!("{kw} is not a keyword kind"));
        self.do_bump(kind);
    }

    /// Consume `kind` if present.
    pub(crate) fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.do_bump(kind);
            true
        } else {
            false
        }
    }

    /// Consume keyword `kw` if present.
    pub(crate) fn eat_kw(&mut self, kw: &str) -> bool {
        if self.at_kw(kw) {
            self.bump_kw(kw);
            true
        } else {
            false
        }
    }

    /// Consume `kind` or report an error (without consuming anything).
    pub(crate) fn expect(&mut self, kind: SyntaxKind) -> bool {
        if self.eat(kind) {
            return true;
        }
        self.error(format!("expected {}", describe(kind)));
        false
    }

    /// Consume keyword `kw` or report an error.
    pub(crate) fn expect_kw(&mut self, kw: &str) -> bool {
        if self.eat_kw(kw) {
            return true;
        }
        self.error(format!("expected '{kw}'"));
        false
    }

    /// Consume an `Id` or report an error.
    pub(crate) fn expect_id(&mut self) -> bool {
        if self.at_id() {
            self.bump(IDENT);
            return true;
        }
        if self.at(IDENT) {
            self.error(format!(
                "'{}' is a reserved word and cannot be used as an identifier",
                self.current_text()
            ));
        } else {
            self.error("expected identifier");
        }
        false
    }

    /// Consume an `IdTerminal` (identifier that is not a keyword) or report an error.
    pub(crate) fn expect_id_terminal(&mut self) -> bool {
        if self.at_id_terminal() {
            self.bump(IDENT);
            return true;
        }
        if self.at(IDENT) {
            self.error(format!("'{}' is a keyword and cannot be used as a name here", self.current_text()));
        } else {
            self.error("expected identifier");
        }
        false
    }

    // ----- errors -----

    /// Report an error at the current token.
    pub(crate) fn error(&mut self, message: impl Into<String>) {
        let range = self.current_range();
        self.error_at(range, message);
    }

    pub(crate) fn error_at(&mut self, range: TextRange, message: impl Into<String>) {
        self.errors.push(SyntaxError { message: message.into(), range });
    }

    /// Report an error and consume the current token into an `ERROR_NODE`.
    #[allow(dead_code)] // part of the parser toolkit, not needed by the current grammar
    pub(crate) fn err_and_bump(&mut self, message: impl Into<String>) {
        self.error(message);
        if !self.at_eof() {
            let m = self.start();
            self.bump_any();
            m.complete(self, ERROR_NODE);
        }
    }

    /// Guard against loops that make no progress: returns `false` after too many steps.
    pub(crate) fn step(&mut self) -> bool {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            panic!("parser stuck at {:?} ({:?})", self.current(), self.current_text());
        }
        true
    }

    // ----- markers -----

    pub(crate) fn start(&mut self) -> Marker {
        let pos = self.events.len() as u32;
        self.events.push(Event::Start { kind: None, forward_parent: None });
        Marker { pos, completed: false }
    }
}

impl Marker {
    pub(crate) fn complete(mut self, p: &mut Parser<'_>, kind: SyntaxKind) -> CompletedMarker {
        self.completed = true;
        match &mut p.events[self.pos as usize] {
            Event::Start { kind: slot, .. } => *slot = Some(kind),
            _ => unreachable!(),
        }
        p.events.push(Event::Finish);
        CompletedMarker { start_pos: self.pos, kind }
    }

    pub(crate) fn abandon(mut self, p: &mut Parser<'_>) {
        self.completed = true;
        if self.pos as usize == p.events.len() - 1 {
            if let Some(Event::Start { kind: None, forward_parent: None }) = p.events.last() {
                p.events.pop();
            }
        }
    }
}

impl Drop for Marker {
    fn drop(&mut self) {
        if !self.completed && !std::thread::panicking() {
            panic!("marker was neither completed nor abandoned");
        }
    }
}

impl CompletedMarker {
    /// Start a new node that will wrap this completed node.
    pub(crate) fn precede(self, p: &mut Parser<'_>) -> Marker {
        let new_pos = p.start();
        match &mut p.events[self.start_pos as usize] {
            Event::Start { forward_parent, .. } => {
                *forward_parent = Some(new_pos.pos - self.start_pos);
            }
            _ => unreachable!(),
        }
        new_pos
    }

    #[allow(dead_code)] // part of the parser toolkit, not needed by the current grammar
    pub(crate) fn kind(&self) -> SyntaxKind {
        self.kind
    }
}

/// Human readable name of a token kind for diagnostics.
pub(crate) fn describe(kind: SyntaxKind) -> String {
    if let Some(kw) = kind.keyword_text() {
        return format!("'{kw}'");
    }
    let s = match kind {
        L_CURLY => "'{'",
        R_CURLY => "'}'",
        L_PAREN => "'('",
        R_PAREN => "')'",
        L_BRACK => "'['",
        R_BRACK => "']'",
        COMMA => "','",
        COLON => "':'",
        SEMICOLON => "';'",
        STAR => "'*'",
        HASH => "'#'",
        DOT | STICKY_DOT => "'.'",
        EQ => "'='",
        NOT_EQUAL => "'!='",
        ARROW => "'->'",
        BACK_ARROW => "'<-'",
        BI_ARROW => "'<->'",
        ARROW_L_BRACK => "'-['",
        R_BRACK_ARROW => "']->'",
        R_BRACK_BI_ARROW => "']<->'",
        IDENT => "identifier",
        STRING => "string",
        MARKDOWN_STRING => "markdown string",
        NUMBER => "number",
        FLOAT => "float",
        PERCENT => "percentage",
        BOOLEAN => "boolean",
        URI => "uri",
        LIB_ICON => "icon",
        _ => return format!("{kind:?}"),
    };
    s.to_string()
}

/// Replay events into a green tree, attaching trivia Langium-style.
fn build_tree(text: &str, tokens: &[RawToken], mut events: Vec<Event>) -> GreenNode {
    let mut builder = GreenNodeBuilder::new();
    let mut raw_pos = 0usize;
    let mut root_started = false;

    let eat_trivia = |builder: &mut GreenNodeBuilder<'_>, raw_pos: &mut usize| {
        while let Some(t) = tokens.get(*raw_pos) {
            if !t.kind.is_trivia() {
                break;
            }
            builder.token(t.kind.into(), &text[t.range]);
            *raw_pos += 1;
        }
    };

    let mut forward_parents: Vec<SyntaxKind> = Vec::new();
    let mut depth = 0usize;
    for i in 0..events.len() {
        match std::mem::replace(&mut events[i], Event::Start { kind: None, forward_parent: None }) {
            Event::Start { kind: None, .. } => {}
            Event::Start { kind: Some(kind), forward_parent } => {
                forward_parents.clear();
                forward_parents.push(kind);
                let mut idx = i;
                let mut fp = forward_parent;
                while let Some(fwd) = fp {
                    idx += fwd as usize;
                    fp = match std::mem::replace(
                        &mut events[idx],
                        Event::Start { kind: None, forward_parent: None },
                    ) {
                        Event::Start { kind, forward_parent } => {
                            if let Some(kind) = kind {
                                forward_parents.push(kind);
                            }
                            forward_parent
                        }
                        _ => unreachable!(),
                    };
                }
                for kind in forward_parents.drain(..).rev() {
                    if root_started {
                        eat_trivia(&mut builder, &mut raw_pos);
                    }
                    builder.start_node(kind.into());
                    root_started = true;
                    depth += 1;
                }
            }
            Event::Finish => {
                depth -= 1;
                if depth == 0 {
                    // Trailing trivia belongs to the root, like Langium attaches
                    // leftover hidden tokens to the root node.
                    eat_trivia(&mut builder, &mut raw_pos);
                }
                builder.finish_node();
            }
            Event::Token { kind } => {
                eat_trivia(&mut builder, &mut raw_pos);
                let t = tokens[raw_pos];
                builder.token(kind.into(), &text[t.range]);
                raw_pos += 1;
            }
        }
    }
    debug_assert_eq!(raw_pos, tokens.len(), "not all tokens were consumed by the parser");
    builder.finish()
}
