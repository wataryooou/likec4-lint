# likec4-lint

Fast linter and formatter for the [LikeC4](https://likec4.dev) architecture-as-code DSL,
written in Rust in the spirit of [Biome](https://biomejs.dev) and [oxlint](https://oxc.rs).

This project is not affiliated with or endorsed by the LikeC4 project.

- Formatter: byte-for-byte compatible with the official `likec4 format`
  (the Langium formatter that also powers VS Code "Format Document"), without Node.js
  or a language-server start-up. Verified against 140+ oracle fixtures generated from
  the official implementation.
- Linter: 24 project-wide rules, configurable per rule. 10 of them go beyond what
  `likec4 validate` reports (unused kinds and tags, empty bodies, views without rules,
  deprecated predicates, naming conventions, required titles and tags); the other 14 cover
  the same ground as the official validator, so one fast pass also reports the basics.
- Lossless parser: a hand-written lexer and recursive-descent parser producing a
  `rowan` syntax tree; every byte of the source is preserved, comments included.

## Why

LikeC4 ships a TypeScript formatter and a validator (`likec4 format`, `likec4 validate`),
but no configurable, rule-based linter, and every tool needs the full language server
(Node.js + Langium + workspace indexing) to run. `likec4-lint` parses, formats and lints a
project in milliseconds and is a single static binary that fits into pre-commit hooks and CI.

### Use it together with `likec4 validate`

`likec4-lint` complements the official validator, it does not replace it. The official
validator implements semantic checks this tool does not (for example the parent-child
relationship check), so a clean `likec4-lint` run does not guarantee that `likec4 validate`
passes. Keep both: `likec4-lint` as the fast, rule-based gate in pre-commit hooks and CI,
`likec4 validate` as the source of truth for what the language server accepts. The
`in likec4 validate` column of the [rule table](#lint-rules) shows which rules overlap.

The parser and formatter track likec4 1.59.3. A weekly workflow
(`.github/workflows/oracle-drift.yml`) re-runs the formatter oracles and the upstream
examples against the latest `likec4` release on npm, so an upstream grammar or formatter
change is caught by CI rather than by users.

## Install

```sh
cargo install likec4-lint   # once published
# or from a checkout
cargo install --path crates/likec4-lint
# or build from source without installing
cargo build --release   # -> target/release/likec4-lint
```

## Usage

```sh
likec4-lint lint [PATHS...]            # lint (default: current directory)
likec4-lint format --check [PATHS...]  # report files that are not formatted (exit 1)
likec4-lint format --write [PATHS...]  # format in place
likec4-lint format --stdin < model.c4  # format stdin to stdout
likec4-lint check [PATHS...]           # lint + format --check
likec4-lint lint --list-rules          # show all rules and their default level
likec4-lint lint --format json         # machine-readable output
```

Other flags accepted by `format` (and, where they apply, by `lint` / `check`): `--diff`
(unified diff of files that need formatting, with `--check`), `--color auto|always|never`,
`--quiet` (diagnostics only, no per-file status or summary line), `--json` (shorthand for
`--format json`), `--indent-width N`, `--use-tabs`, `--stdin-filepath PATH` (display name for
`--stdin` input) and `--config FILE`. `format --stdin --check` is meant for pre-commit hooks
and `lint-staged`: it reads one document from stdin and, when it is not formatted, prints
nothing to stdout (a one-line `needs formatting <name>` notice goes to stderr instead) and
exits `1`.

Files are discovered recursively (`*.c4`, `*.likec4`, `*.like-c4`, case-insensitive),
`.gitignore` and `node_modules` are respected. `.gitignore` is honoured even outside a git
repository. Exit codes: `0` clean (a warning-only run is still `0`), `1` at least one error
diagnostic (this includes `needs-formatting`, `format-error` and `io-error`), `2` a usage,
configuration or I/O error.

### Configuration

`likec4-lint.toml`, searched upwards from the working directory (or `--config FILE`). The
search stops at the first directory that contains `.git` or at the home directory, whichever
comes first, so a config file above a repository is never picked up from inside it. The home
directory itself is never treated as a project root or as a context origin, even when it
holds a config file, a project marker or `.git`. The
config file that was used is printed at the end of the summary line and in `summary.config`
of the JSON output.

```toml
[format]
quote_style = "auto"   # auto | single | double | ignore  (same as VS Code `likec4.formatting.quoteStyle`)
indent_width = 2       # 1..=16
use_tabs = false

[lint]
exclude = ["**/generated/**"]

[lint.rules]
unused-tag = "off"
naming-convention = { level = "warning", pattern = "^[a-z][a-zA-Z0-9]*$" }
require-tags = { level = "error", kinds = ["system"], tags = ["owner"] }
```

Unknown keys and sections, an out-of-range `indent_width`, and an invalid `level` are
configuration errors (exit `2`). An unrecognised entry under `[lint.rules]` is reported as
the `unknown-rule` diagnostic, and an option a rule does not declare as `unknown-rule-option`
(both `warning`, no file). `[lint] exclude` only suppresses reporting: an excluded document is
still parsed and loaded as project context for `lint` / `check` (so it can still be referenced
from, say, an included file), it is just not itself reported on; `format` does not read or
write excluded documents at all. A path passed explicitly on the command line is always
processed regardless of `exclude`.

## Lint rules

`likec4-lint lint --list-rules` prints the current list. Defaults:

| rule | level | in `likec4 validate` | what it reports |
| --- | --- | --- | --- |
| `syntax-error` | error | yes | parser diagnostics |
| `unknown-element-kind`, `unknown-deployment-node-kind`, `unknown-relationship-kind`, `unknown-tag`, `unknown-custom-color` | error | yes | used but not declared in any `specification` of the project |
| `duplicate-element` | error | yes | the same FQN declared twice; elements and the deployment namespace (nodes and instances) are checked separately, so an element and a deployment node may share a name |
| `duplicate-view`, `duplicate-spec` | error | yes | the same view name, kind, tag, colour, predicate group or global style declared twice |
| `unresolved-reference` | error | yes | relation endpoint, `extend`, `instanceOf`, `navigateTo`, `view ... of`, `extends`, `global predicate` / `global style` target, or a view rule element (`include` / `exclude` / `style` / `rank`) that does not exist |
| `invalid-color` | error | yes | bad hex length, rgb component or alpha out of range |
| `self-relation` | warning | yes, as an error | `a -> a` in model or deployment relations, comparing both endpoints after resolving them from the enclosing scope; an endpoint that does not resolve is left to `unresolved-reference` instead |
| `unused-element-kind`, `unused-tag`, `unused-relationship-kind` | warning | no | declared but never used, as seen from the declaring project: a use in one of its own documents, or in a document that imports it, counts |
| `empty-body` | warning | no | `{ }` with nothing (or only comments) inside, for every brace-owning construct (specification/model/views/deployment/global blocks, element/relation/extend/deployment-node/instance bodies, `style`, `metadata`, `group`, `rank`, predicate/style groups, dynamic-view sub-flows); import lists and `likec4lib` are excluded |
| `view-without-rules` | warning | no | a view with no `include` / `exclude` / `global predicate`; dynamic views and views with `extends` are exempt |
| `deprecated-element-predicate` | warning | no | `element.kind = x` / `element.tag = #x` expressions |
| `reserved-name` | warning | yes, as an error | element or view named `element`, `model`, `group`, `node`, ... |
| `multiple-specifications` | warning | no | more than one `specification` block in a document |
| `invalid-opacity` | warning | yes | opacity outside 0%..100% |
| `naming-convention` | off | no | names must match the configured `pattern` regex; `targets` selects which of `element`, `view`, `deployment-node` are checked (default: `element`, `view`) |
| `require-title` | off | no | elements must have a title |
| `require-tags` | off | no | elements of the configured `kinds` must carry the configured `tags` |

The `in likec4 validate` column says whether the official validator reports the same problem,
possibly under another name or severity.

Rules are project-aware: files are grouped by the nearest `likec4.config.*` / `.likec4rc`
marker, and linting a single file or subdirectory loads the rest of its project as context.
When no marker is found above a requested path, the project root falls back to context: the
adopted config file's directory (only when the request is under it), the directory given on
the command line itself, or, for a file argument, the nearest ancestor containing `.git`,
else the current directory (if the file is under it), else the file's own parent directory.

## Performance

`format --check` over the 39 official example files (Apple Silicon, release build):

| tool | wall time |
| --- | --- |
| `likec4-lint format --check` | 0.01 s |
| `likec4 fmt --check` (official, Node.js) | 1.1 s |

## Formatter compatibility

The formatter is a port of Langium's `AbstractFormatter` engine and of every rule in
LikeC4's `LikeC4Formatter.ts` (likec4 v1.59.3). Compatibility is tested with:

| fixtures | source | count |
| --- | --- | --- |
| `tests/fixtures/formatter` | extracted from `LikeC4Formatter.spec.ts` | 48 |
| `tests/fixtures/formatter-cli` | hand-written corner cases run through `likec4 format` | 52 |
| `tests/fixtures/formatter-quirks` | odd behaviours of the official formatter, pinned on purpose | 6 |
| `tests/corpus/examples` | the official `examples/` projects, before/after `likec4 format` | 39 |

Every fixture must match byte for byte and be idempotent. Deliberate deviations:

1. When the official formatter produces two edits for the same empty gap — `key: value`
   (metadata attributes and `link:`), `import{a}from`, `try'x'{`, `-[uses]->b` — it applies
   both and oscillates between passes (`key: v` / `key : v` and so on). `likec4-lint` lets the
   later edit win, so it lands directly on the official formatter's fixed point (its second
   pass) instead of its unstable first pass.
2. A markdown string is left unnormalised when its content ends with the quote
   normalisation would switch to, because normalising it would produce text the official
   parser cannot lex.
3. If the formatted output would not parse, `likec4-lint` writes nothing and reports
   `format-error` rather than emitting unstable text.
4. `--use-tabs` is an extension the official CLI does not have (it only ever formats with
   spaces); tab-mode comment re-indentation is also idempotent, unlike the known space-mode
   limitation below.

Known limitations:

- A file with a UTF-8 BOM fails to lex and is returned unchanged (same as the official
  formatter).
- In space mode, a comment indented with tabs where spaces are expected is not idempotent:
  the first pass strips the tabs, the second re-indents with spaces (same first-pass
  behaviour as the official formatter,
  `tests/fixtures/formatter-quirks/comment-tab-indent-non-idempotent`).
- Nesting deeper than 512 levels is reported as a syntax error instead of formatted.

## Repository layout

| crate | purpose |
| --- | --- |
| `crates/likec4-syntax` | lexer, parser, syntax tree, typed AST accessors |
| `crates/likec4-fmt` | formatter |
| `crates/likec4-rules` | project model and lint rules |
| `crates/likec4-lint` | the `likec4-lint` binary |

Design notes live in [`docs/DESIGN.md`](docs/DESIGN.md).

## Development

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
cargo deny check
```

MSRV is Rust 1.88 (`rust-version` in `Cargo.toml`).

Regenerating oracle fixtures requires the official CLI: `npm i likec4@1.59.3` and
`likec4 fmt <dir>`; see `tests/corpus/README.md` and `tests/fixtures/formatter-cli/README.md`.

To check the fixtures and the upstream examples against a newer official CLI without
regenerating anything, run `LIKEC4=path/to/likec4 scripts/check-oracle-drift.sh`; it exits
`1` and prints the diffs when the official output has drifted. The same script runs weekly in
`.github/workflows/oracle-drift.yml` against `likec4@latest`.

## License

MIT. The example projects under `tests/corpus/examples*` are copied from
[likec4/likec4](https://github.com/likec4/likec4) (MIT, see `tests/corpus/LICENSE-likec4`).
