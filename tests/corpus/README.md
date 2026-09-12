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
- To check these oracles (and the fixtures under `../fixtures/`) against a newer official CLI
  without regenerating them, run `LIKEC4=path/to/likec4 scripts/check-oracle-drift.sh` from the
  repository root; `.github/workflows/oracle-drift.yml` does the same weekly against `likec4@latest`.
