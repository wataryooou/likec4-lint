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
