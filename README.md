# likec4-lint

Fast linter and formatter for the [LikeC4](https://likec4.dev) architecture-as-code DSL,
written in Rust in the spirit of [Biome](https://biomejs.dev) and [oxlint](https://oxc.rs).

- **Formatter** — byte-for-byte compatible with the official `likec4 format`
  (the Langium formatter that also powers VS Code "Format Document"), without Node.js
  or a language-server start-up. Verified against 140+ oracle fixtures generated from
  the official implementation.
- **Linter** — project-wide rules the official toolchain does not offer: unknown or unused
  kinds/tags, duplicate elements and views, unresolved references, empty bodies, naming
  conventions, required tags, and more. Configurable per rule.
- **Lossless parser** — a hand-written lexer and recursive-descent parser producing a
  `rowan` syntax tree; every byte of the source is preserved, comments included.

## Why

LikeC4 ships a TypeScript formatter and a validator (`likec4 format`, `likec4 validate`),
but no configurable, rule-based linter, and every tool needs the full language server
(Node.js + Langium + workspace indexing) to run. `likec4-lint` parses, formats and lints a
project in milliseconds and is a single static binary that fits into pre-commit hooks and CI.

## Install

```sh
cargo install --path crates/likec4-cli
# or build from source
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

Files are discovered recursively (`*.c4`, `*.likec4`, `*.like-c4`), `.gitignore` and
`node_modules` are respected. Exit codes: `0` clean, `1` findings, `2` usage/IO error.

### Configuration

`likec4-lint.toml`, searched upwards from the working directory (or `--config FILE`):

```toml
[format]
quote_style = "auto"   # auto | single | double | ignore  (same as VS Code `likec4.formatting.quoteStyle`)
indent_width = 2

[lint]
exclude = ["**/generated/**"]

[lint.rules]
unused-tag = "off"
naming-convention = { level = "warning", pattern = "^[a-z][a-zA-Z0-9]*$" }
require-tags = { level = "error", kinds = ["system"], tags = ["owner"] }
```

## Lint rules

`likec4-lint lint --list-rules` prints the current list. Defaults:

| rule | level | what it reports |
| --- | --- | --- |
| `syntax-error` | error | parser diagnostics |
| `unknown-element-kind`, `unknown-deployment-node-kind`, `unknown-relationship-kind`, `unknown-tag`, `unknown-custom-color` | error | used but not declared in any `specification` of the project |
| `duplicate-element`, `duplicate-view`, `duplicate-spec` | error | the same FQN, view name, kind, tag, colour or global declared twice |
| `unresolved-reference` | error | relation endpoint, `extend`, `instanceOf`, `navigateTo`, `view ... of`, `extends`, `global predicate` / `global style` target that does not exist |
| `invalid-color` | error | bad hex length, rgb component or alpha out of range |
| `self-relation` | warning | `a -> a` in model or deployment relations |
| `unused-element-kind`, `unused-tag`, `unused-relationship-kind` | warning | declared but never used |
| `empty-body` | warning | `{ }` with nothing (or only comments) inside |
| `view-without-rules` | warning | a view with no `include` / `exclude` / `global predicate` |
| `deprecated-element-predicate` | warning | `element.kind = x` / `element.tag = #x` expressions |
| `reserved-name` | warning | element or view named `element`, `model`, `group`, `node`, ... |
| `multiple-specifications` | warning | more than one `specification` block in a document |
| `invalid-opacity` | warning | opacity outside 0%..100% |
| `naming-convention` | off | names must match the configured `pattern` |
| `require-title` | off | elements must have a title |
| `require-tags` | off | elements of the configured `kinds` must carry the configured `tags` |

Rules are project-aware: files are grouped by the nearest `likec4.config.*` / `.likec4rc`,
and linting a single file or subdirectory loads the rest of its project as context.

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
| `tests/fixtures/formatter-quirks` | odd behaviours of the official formatter, pinned on purpose | 3 |
| `tests/corpus/examples` | the official `examples/` projects, before/after `likec4 format` | 39 |

Every fixture must match byte for byte and be idempotent. One deliberate deviation:
the official formatter oscillates on `key: value` (metadata attributes and `link:`),
alternating between `key: v` and `key : v` on every run; `likec4-lint` produces the
stable `key: v`.

## Repository layout

| crate | purpose |
| --- | --- |
| `crates/likec4-syntax` | lexer, parser, syntax tree, typed AST accessors |
| `crates/likec4-fmt` | formatter |
| `crates/likec4-lint` | project model and lint rules |
| `crates/likec4-cli` | the `likec4-lint` binary |

Design notes live in [`docs/DESIGN.md`](docs/DESIGN.md).

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Regenerating oracle fixtures requires the official CLI: `npm i likec4@1.59.3` and
`likec4 fmt <dir>`; see `tests/corpus/README.md` and `tests/fixtures/formatter-cli/README.md`.

## License

MIT. The example projects under `tests/corpus/examples*` are copied from
[likec4/likec4](https://github.com/likec4/likec4) (MIT, see `tests/corpus/LICENSE-likec4`).
