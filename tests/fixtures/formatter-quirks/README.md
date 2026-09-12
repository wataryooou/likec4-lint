# Formatter quirk fixtures

Generated with the official `likec4 format` (v1.59.3, default options) to pin down
behaviours of `LikeC4Formatter.ts` that are easy to get wrong when porting:

- `quirks`: `tag { color }` body untouched, `extend a -> b { }` body not re-indented,
  top-level `deployment {` keeps its indentation while its `}` moves to column 0,
  deployment view `style { }` body not re-indented, `rank { }` body untouched.
- `braces`: `likec4lib` has two brace pairs (no interior formatting, both `}` at column 0),
  single-line bodies `{a}` → `{ a}`, `{  b  }` → `{ b }`.
- `steps`: multiline step series wrap every arrow (including the first step's), a single
  step with a multiline custom block is left alone, custom blocks stay on the arrow line.
- `adjacent-nodes`: two statements with no whitespace between them
  (`title 'x'description 'y'`, `b = system {}c = system`) stay on one line.
  `indentContentInBraces` skips an interior node that overlaps the previous one
  (`utils.areOverlap`, whose range check is inclusive at both ends, so "starts exactly where
  the previous node ends" counts as overlapping). The official CLI leaves the input unchanged.

Fixtures whose expected output is not the official first pass:

- `same-gap-last-wins`: `import{a}from`, `try'x'{` and `include b -[uses]->c`. The official
  formatter emits two edits for the same empty gap (for example `}` append oneSpace and `from`
  prepend oneSpace) and applies both, so its first pass prints `import  { a }  from`,
  `try  'x'  {` and `-[uses]->  c`; a second official pass collapses them to one space.
  likec4-fmt lets the later edit win, so its output differs from the official first pass but
  equals the official fixed point (the second pass), which is what the expected file holds.

Fixtures that are deliberately not idempotent (listed in `NON_IDEMPOTENT_FIXTURES` in
`crates/likec4-fmt/tests/fixtures.rs`):

- `comment-tab-indent-non-idempotent`: a comment indented with two tabs where two spaces are
  expected. Langium measures the existing indentation in columns (a tab counts as `tabSize`)
  but removes one raw character per column, so the first official pass strips both tabs and
  leaves the comment at column 0, and the second pass indents it with two spaces. likec4-fmt
  reproduces the first pass byte for byte. This only affects space indentation; with
  `--use-tabs` (which has no official oracle, the CLI formats with spaces only) likec4-fmt
  removes whole indentation units instead, so tab-mode output is a fixed point.
