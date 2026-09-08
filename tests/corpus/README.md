# Test corpus

- `examples/` — copied verbatim from `likec4/likec4` (`examples/**/*.c4`, MIT, see `LICENSE-likec4`).
- `examples-formatted/` — the same files after running the official `likec4 format`
  (likec4 v1.59.3, default settings: 2-space indent, `quoteStyle: auto`).
  These are the formatter oracle: `likec4-fmt` must reproduce them byte for byte.
- `../fixtures/formatter/` — input/expected pairs extracted from
  `packages/language-server/src/formatting/LikeC4Formatter.spec.ts` (run with `quoteStyle: single`).
- Project marker files (`.likec4rc`, `likec4.config.json`, `likec4.config.mjs`) are copied from
  the official `examples/` unchanged. They partition the corpus into 10 projects (plus the
  implicit default project); without them `cloud-system` and `multi-project/projectA`, which
  are identical copies, would be reported as duplicates.
