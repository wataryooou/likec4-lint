//! Port of the Langium `AbstractFormatter` engine (langium 3.5.0, `lib/lsp/formatter.js`).
//!
//! Rules collect `(range, mode) -> Formatting` entries (see [`Collector`]); the engine then
//! walks the CST like Langium's `iterateCstFormatting`, turning entries into text edits,
//! and applies them. Whitespace tokens are ignored during the walk because Langium does
//! not keep them in its CST; comments are the Langium "hidden" leaves.

use std::collections::HashMap;

use likec4_syntax::{
    NodeOrToken, Range as TextRange, SyntaxElement, SyntaxNode, SyntaxToken, TextSize, WalkEvent,
};

use crate::FormatOptions;

/// One formatting move (Langium `FormattingMove`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Move {
    pub characters: Option<u32>,
    pub lines: Option<u32>,
    pub tabs: Option<u32>,
}

/// A formatting action (Langium `FormattingAction`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formatting {
    pub moves: Vec<Move>,
    pub allow_more: bool,
    pub allow_less: bool,
    pub priority: i32,
}

impl Formatting {
    fn single(mv: Move) -> Self {
        Formatting { moves: vec![mv], allow_more: false, allow_less: false, priority: 0 }
    }

    /// `Formatting.noSpace()`
    pub fn no_space() -> Self {
        Self::spaces(0)
    }

    /// `Formatting.oneSpace()`
    pub fn one_space() -> Self {
        Self::spaces(1)
    }

    /// `Formatting.spaces(n)`
    pub fn spaces(n: u32) -> Self {
        Self::single(Move { characters: Some(n), ..Move::default() })
    }

    /// `Formatting.newLine()`
    pub fn new_line() -> Self {
        Self::new_lines(1)
    }

    /// `Formatting.newLines(n)`
    pub fn new_lines(n: u32) -> Self {
        Self::single(Move { lines: Some(n), ..Move::default() })
    }

    /// `Formatting.indent()` — new line plus one indentation level.
    pub fn indent() -> Self {
        Self::single(Move { lines: Some(1), tabs: Some(1), ..Move::default() })
    }

    /// `Formatting.noIndent()` — keep the line breaks, remove indentation.
    pub fn no_indent() -> Self {
        Self::single(Move { tabs: Some(0), ..Move::default() })
    }

    /// A bare `{ tabs: n }` move (used by the multiline step rule).
    pub fn tabs(n: u32) -> Self {
        Self::single(Move { tabs: Some(n), ..Move::default() })
    }

    pub fn allow_more(mut self) -> Self {
        self.allow_more = true;
        self
    }

    pub fn allow_less(mut self) -> Self {
        self.allow_less = true;
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }
}

/// Whether a formatting is attached before or after a CST node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Prepend,
    Append,
}

/// Collects formatting entries keyed by `(range, mode)` exactly like Langium's
/// `nodeModeToKey` (`offset:end:mode`): two CST nodes with the same range share a key,
/// and a later entry replaces an earlier one unless it has a lower priority.
#[derive(Debug, Default)]
pub struct Collector {
    map: HashMap<(TextRange, Mode), Formatting>,
}

impl Collector {
    pub fn add(&mut self, range: TextRange, mode: Mode, formatting: Formatting) {
        let key = (range, mode);
        match self.map.get(&key) {
            Some(existing) if existing.priority > formatting.priority => {}
            _ => {
                self.map.insert(key, formatting);
            }
        }
    }

    pub fn prepend(&mut self, el: &SyntaxElement, formatting: Formatting) {
        self.add(el.text_range(), Mode::Prepend, formatting);
    }

    pub fn append(&mut self, el: &SyntaxElement, formatting: Formatting) {
        self.add(el.text_range(), Mode::Append, formatting);
    }

    pub fn surround(&mut self, el: &SyntaxElement, formatting: Formatting) {
        self.prepend(el, formatting.clone());
        self.append(el, formatting);
    }

    pub fn prepend_node(&mut self, node: &SyntaxNode, formatting: Formatting) {
        self.add(node.text_range(), Mode::Prepend, formatting);
    }

    pub fn append_node(&mut self, node: &SyntaxNode, formatting: Formatting) {
        self.add(node.text_range(), Mode::Append, formatting);
    }

    pub fn prepend_token(&mut self, token: &SyntaxToken, formatting: Formatting) {
        self.add(token.text_range(), Mode::Prepend, formatting);
    }

    pub fn append_token(&mut self, token: &SyntaxToken, formatting: Formatting) {
        self.add(token.text_range(), Mode::Append, formatting);
    }

    pub fn surround_token(&mut self, token: &SyntaxToken, formatting: Formatting) {
        self.prepend_token(token, formatting.clone());
        self.append_token(token, formatting);
    }

    fn take(&mut self, range: TextRange, mode: Mode) -> Option<Formatting> {
        self.map.remove(&(range, mode))
    }
}

/// A replacement of `range` by `new_text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    pub range: TextRange,
    pub new_text: String,
}

/// Line/column lookup for byte offsets. Like the LSP text document Langium formats,
/// `\n`, `\r\n` and a lone `\r` each end a line.
#[derive(Debug)]
pub struct LineIndex {
    line_starts: Vec<u32>,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0u32];
        let bytes = text.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            let breaks = match b {
                b'\n' => true,
                b'\r' => bytes.get(i + 1) != Some(&b'\n'),
                _ => false,
            };
            if breaks {
                line_starts.push(i as u32 + 1);
            }
        }
        LineIndex { line_starts }
    }

    /// Zero-based line of a byte offset.
    pub fn line(&self, offset: TextSize) -> usize {
        let offset: u32 = offset.into();
        match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        }
    }

    /// Byte offset of the start of `line`.
    pub fn line_start(&self, line: usize) -> TextSize {
        TextSize::new(self.line_starts[line])
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

/// True for CST elements Langium keeps in its tree (everything but whitespace).
fn is_cst_element(el: &SyntaxElement) -> bool {
    match el {
        NodeOrToken::Node(_) => true,
        NodeOrToken::Token(t) => !t.kind().is_whitespace(),
    }
}

fn is_hidden(el: &SyntaxElement) -> bool {
    matches!(el, NodeOrToken::Token(t) if t.kind().is_comment())
}

/// Langium `getNextNode(node, hidden = true)`: next sibling (comments included, whitespace
/// excluded), walking up to ancestors when at the end of the parent.
fn next_cst_element(el: &SyntaxElement) -> Option<SyntaxElement> {
    let mut current = el.clone();
    loop {
        let mut sibling = current.next_sibling_or_token();
        while let Some(s) = sibling {
            if is_cst_element(&s) {
                return Some(s);
            }
            sibling = s.next_sibling_or_token();
        }
        let parent = current.parent()?;
        current = NodeOrToken::Node(parent);
    }
}

struct Context<'a> {
    text: &'a str,
    lines: LineIndex,
    options: &'a FormatOptions,
    indentation: u32,
}

impl Context<'_> {
    fn indent_unit(&self) -> String {
        if self.options.insert_spaces {
            " ".repeat(self.options.indent_width)
        } else {
            "\t".to_string()
        }
    }

    fn tab_size(&self) -> usize {
        self.options.indent_width
    }

    fn line_of(&self, offset: TextSize) -> usize {
        self.lines.line(offset)
    }

    /// `getIndentationCharacterCount`
    fn indentation_chars(&self, mv: Option<&Move>) -> usize {
        let mut indentation = self.indentation as usize;
        if let Some(tabs) = mv.and_then(|m| m.tabs) {
            indentation += tabs as usize;
        }
        (if self.options.insert_spaces { self.tab_size() } else { 1 }) * indentation
    }

    /// `getExistingIndentationCharacterCount`
    fn existing_indentation_chars(&self, text: &str) -> usize {
        let tab = " ".repeat(self.tab_size());
        if self.options.insert_spaces {
            text.replace('\t', &tab).len()
        } else {
            text.replace(&tab, "\t").len()
        }
    }
}

/// Run the collected formattings over the tree and return the edits (Langium
/// `iterateCstFormatting` + `avoidOverlappingEdits`).
pub fn compute_edits(
    root: &SyntaxNode,
    text: &str,
    options: &FormatOptions,
    mut collector: Collector,
) -> Vec<TextEdit> {
    let mut ctx = Context { text, lines: LineIndex::new(text), options, indentation: 0 };
    let mut edits: Vec<TextEdit> = Vec::new();
    let mut last_leaf: Option<SyntaxElement> = None;
    let mut indent_stack: Vec<u32> = Vec::new();

    for event in root.preorder_with_tokens() {
        match event {
            WalkEvent::Enter(el) => {
                if !is_cst_element(&el) {
                    continue;
                }
                let is_node = el.as_node().is_some();
                if is_node {
                    // `iterateCst` captures the indentation before the node's own prepend.
                    indent_stack.push(ctx.indentation);
                }
                let range = el.text_range();

                let prepend = collector.take(range, Mode::Prepend);
                if let Some(formatting) = &prepend {
                    edits.extend(create_text_edit(&mut ctx, last_leaf.as_ref(), &el, formatting));
                }
                if let Some(formatting) = collector.take(range, Mode::Append) {
                    if let Some(next) = next_cst_element(&el) {
                        edits.extend(create_text_edit(&mut ctx, Some(&el), &next, &formatting));
                    }
                }
                if prepend.is_none() && is_hidden(&el) {
                    edits.extend(create_hidden_text_edits(&ctx, last_leaf.as_ref(), &el, None));
                }
                if !is_node {
                    last_leaf = Some(el);
                }
            }
            WalkEvent::Leave(el) => {
                if el.as_node().is_some() {
                    if let Some(initial) = indent_stack.pop() {
                        ctx.indentation = initial;
                    }
                }
            }
        }
    }

    avoid_overlapping_edits(text, edits)
}

/// Langium `createTextEdit(a, b, formatting, context)`.
fn create_text_edit(
    ctx: &mut Context<'_>,
    a: Option<&SyntaxElement>,
    b: &SyntaxElement,
    formatting: &Formatting,
) -> Vec<TextEdit> {
    if is_hidden(b) {
        return create_hidden_text_edits(ctx, a, b, Some(formatting));
    }
    if let Some(a) = a {
        // Ignore the edit if the previous node ends after the current node starts.
        if a.text_range().end() > b.text_range().start() {
            return Vec::new();
        }
    }
    let start = a.map(|a| a.text_range().end()).unwrap_or(TextSize::new(0));
    let between = TextRange::new(start, b.text_range().start());
    let Some(mv) = find_fitting_move(ctx, between, &formatting.moves) else {
        return Vec::new();
    };
    let existing_indentation = ctx.indentation;
    ctx.indentation += mv.tabs.unwrap_or(0);
    let mut edits = Vec::new();
    if let Some(chars) = mv.characters {
        // Do not apply formatting on the same line if the preceding node is hidden.
        if !a.is_some_and(is_hidden) {
            edits.push(create_space_text_edit(ctx, between, chars, formatting));
        }
    } else if let Some(lines) = mv.lines {
        edits.push(create_line_text_edit(ctx, between, lines, formatting));
    } else if mv.tabs.is_some() {
        edits.push(create_tab_text_edit(ctx, between, a.is_some()));
    }
    if b.as_token().is_some() {
        ctx.indentation = existing_indentation;
    }
    edits
}

fn existing_newlines(ctx: &Context<'_>, range: TextRange) -> u32 {
    (ctx.line_of(range.end()) - ctx.line_of(range.start())) as u32
}

fn fit_into_options(value: u32, existing: u32, formatting: &Formatting) -> u32 {
    if formatting.allow_more {
        value.max(existing)
    } else if formatting.allow_less {
        value.min(existing)
    } else {
        value
    }
}

fn create_space_text_edit(
    ctx: &Context<'_>,
    range: TextRange,
    mut spaces: u32,
    formatting: &Formatting,
) -> TextEdit {
    if ctx.line_of(range.start()) == ctx.line_of(range.end()) {
        // Langium counts UTF-16 characters; the gap only contains ASCII whitespace/comments.
        let existing = ctx.text[range].chars().count() as u32;
        spaces = fit_into_options(spaces, existing, formatting);
    }
    TextEdit { range, new_text: " ".repeat(spaces as usize) }
}

fn create_line_text_edit(
    ctx: &Context<'_>,
    range: TextRange,
    lines: u32,
    formatting: &Formatting,
) -> TextEdit {
    let existing = existing_newlines(ctx, range);
    let lines = fit_into_options(lines, existing, formatting);
    let node_indent = ctx.indent_unit().repeat(ctx.indentation as usize);
    TextEdit { range, new_text: format!("{}{}", "\n".repeat(lines as usize), node_indent) }
}

fn create_tab_text_edit(ctx: &Context<'_>, range: TextRange, has_previous: bool) -> TextEdit {
    let node_indent = ctx.indent_unit().repeat(ctx.indentation as usize);
    let minimum_lines = if has_previous { 1 } else { 0 };
    let lines = existing_newlines(ctx, range).max(minimum_lines);
    TextEdit { range, new_text: format!("{}{}", "\n".repeat(lines as usize), node_indent) }
}

fn find_fitting_move(ctx: &Context<'_>, range: TextRange, moves: &[Move]) -> Option<Move> {
    match moves {
        [] => None,
        [single] => Some(*single),
        _ => {
            let existing_lines = existing_newlines(ctx, range);
            for mv in moves {
                match mv.lines {
                    Some(lines) if existing_lines <= lines => return Some(*mv),
                    None if existing_lines == 0 => return Some(*mv),
                    _ => {}
                }
            }
            moves.last().copied()
        }
    }
}

/// Langium `createHiddenTextEdits(previous, hidden, formatting, context)`: re-indents a
/// comment that starts on its own line.
fn create_hidden_text_edits(
    ctx: &Context<'_>,
    previous: Option<&SyntaxElement>,
    hidden: &SyntaxElement,
    formatting: Option<&Formatting>,
) -> Vec<TextEdit> {
    let hidden_range = hidden.text_range();
    let start_line = ctx.line_of(hidden_range.start());
    if let Some(prev) = previous {
        if ctx.line_of(prev.text_range().end()) == start_line {
            return Vec::new();
        }
    }
    let line_start = ctx.lines.line_start(start_line);
    let start_range = TextRange::new(line_start, hidden_range.start());
    let hidden_start_text = &ctx.text[start_range];
    let moves: &[Move] = formatting.map(|f| f.moves.as_slice()).unwrap_or(&[]);
    let mv = find_fitting_move(ctx, start_range, moves);
    let hidden_start_char = ctx.existing_indentation_chars(hidden_start_text);
    let expected_start_char = ctx.indentation_chars(mv.as_ref());
    let increase = expected_start_char as i64 - hidden_start_char as i64;
    if increase == 0 {
        return Vec::new();
    }
    let hidden_text = &ctx.text[hidden_range];
    let mut lines: Vec<String> = hidden_text.split('\n').map(str::to_string).collect();
    lines[0] = format!("{hidden_start_text}{}", lines[0]);
    let mut edits = Vec::new();
    for (i, line_text) in lines.iter().enumerate() {
        let current_line = start_line + i;
        if current_line >= ctx.lines.line_count() {
            break;
        }
        let pos = ctx.lines.line_start(current_line);
        if increase > 0 {
            let unit = if ctx.options.insert_spaces { " " } else { "\t" };
            edits.push(TextEdit { range: TextRange::empty(pos), new_text: unit.repeat(increase as usize) });
        } else {
            let decrease = increase.unsigned_abs() as usize;
            let leading = line_text.bytes().take_while(|&c| c == b' ' || c == b'\t').count();
            let remove = if ctx.options.insert_spaces {
                // Langium removes one raw character per column. This over-removes when the
                // comment is indented with tabs (a tab counts as `tab_size` columns), which
                // makes the official formatter non-idempotent on such comments; kept for parity.
                leading.min(decrease)
            } else {
                // Deviation from Langium: with tabs, columns are counted in indentation units
                // (`tab_size` spaces or one tab), so removing one raw space per unit leaves the
                // measured indentation unchanged and the comment drifts one space per pass.
                // Remove the shortest prefix that brings the measured indentation down by
                // `decrease` units instead, which makes the result a fixed point.
                let whitespace = &line_text[..leading];
                let target = ctx.existing_indentation_chars(whitespace).saturating_sub(decrease);
                (0..=leading)
                    .find(|&n| ctx.existing_indentation_chars(&whitespace[n..]) <= target)
                    .unwrap_or(leading)
            };
            edits.push(TextEdit {
                range: TextRange::new(pos, pos + TextSize::new(remove as u32)),
                new_text: String::new(),
            });
        }
    }
    edits
}

/// Langium `avoidOverlappingEdits` + `isNecessary`.
fn avoid_overlapping_edits(text: &str, text_edits: Vec<TextEdit>) -> Vec<TextEdit> {
    let mut edits: Vec<TextEdit> = Vec::new();
    for edit in text_edits {
        while let Some(last) = edits.last() {
            // Deviation from Langium: two edits at the same *empty* gap (e.g. `key` append
            // oneSpace and `:` prepend noSpace in `key: value`) are both kept by Langium,
            // which makes the official formatter oscillate between `key: v` and `key : v`.
            // We let the later edit win, like for non-empty gaps, so formatting is idempotent.
            let same_empty_gap = last.range.is_empty() && edit.range.start() == last.range.start();
            if edit.range.start() < last.range.end() || same_empty_gap {
                edits.pop();
            } else {
                break;
            }
        }
        edits.push(edit);
    }
    edits.into_iter().filter(|e| e.new_text != text[e.range].replace('\r', "")).collect()
}

/// Apply non-overlapping edits (sorted by start, stable) to `text`.
pub fn apply_edits(text: &str, mut edits: Vec<TextEdit>) -> String {
    edits.sort_by_key(|e| (e.range.start(), e.range.end()));
    let mut out = String::with_capacity(text.len());
    let mut pos = TextSize::new(0);
    for edit in edits {
        if edit.range.start() < pos {
            // Overlapping edit (cannot happen after `avoid_overlapping_edits`); skip it.
            continue;
        }
        out.push_str(&text[TextRange::new(pos, edit.range.start())]);
        out.push_str(&edit.new_text);
        pos = edit.range.end();
    }
    out.push_str(&text[TextRange::new(pos, TextSize::new(text.len() as u32))]);
    out
}

/// True when `node` spans more than one line (Langium `isMultiline`).
pub fn is_multiline(lines: &LineIndex, node: &SyntaxNode) -> bool {
    let r = node.text_range();
    lines.line(r.start()) != lines.line(r.end())
}

/// Line difference helpers used by step formatting (Langium `utils.isSameLine`).
pub fn is_same_line(lines: &LineIndex, a: TextRange, b: TextRange) -> bool {
    let (prev, next) = if lines.line(a.start()) < lines.line(b.start()) { (a, b) } else { (b, a) };
    lines.line(prev.end()) == lines.line(next.start())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_index_treats_cr_crlf_and_lf_as_line_breaks() {
        let text = "a\r\nb\rc\nd";
        let index = LineIndex::new(text);
        assert_eq!(index.line_count(), 4);
        let lines: Vec<usize> = (0..=text.len()).map(|o| index.line(TextSize::new(o as u32))).collect();
        assert_eq!(lines, vec![0, 0, 0, 1, 1, 2, 2, 3, 3]);
        assert_eq!(index.line_start(1), TextSize::new(3));
        assert_eq!(index.line_start(2), TextSize::new(5));
        assert_eq!(index.line_start(3), TextSize::new(7));
    }

    #[test]
    fn comment_reindentation_with_tabs_is_idempotent() {
        let options = FormatOptions { insert_spaces: false, ..FormatOptions::default() };
        for input in [
            "model {\n    // comment\n  a = system\n}\n",
            "model {\n      /* block\n   ragged\n        */\n  a = system\n}\n",
            "model {\n a = system {\n     // deep\n }\n}\n",
        ] {
            let once = crate::format(input, &options).unwrap();
            let twice = crate::format(&once, &options).unwrap();
            assert_eq!(once, twice, "input: {input:?}");
        }
    }
}
