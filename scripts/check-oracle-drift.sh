#!/usr/bin/env bash
# Checks the formatter oracle against an official `likec4` CLI, without regenerating anything.
#
# 1. tests/corpus/examples, formatted by the official CLI, must equal tests/corpus/examples-formatted.
# 2. Every tests/fixtures/formatter-cli and formatter-quirks input, formatted by the official CLI,
#    must equal its .expected.c4 (a second official pass for SECOND_PASS_FIXTURES).
# 3. The upstream examples (likec4/likec4 `examples/`) must parse with likec4-lint, and
#    likec4-lint `format --check` must accept the official formatter's fixed point of them.
#
# Usage: scripts/check-oracle-drift.sh [--upstream DIR | --no-upstream]
#   --upstream DIR   use a local checkout of likec4/likec4 instead of cloning it
#   --no-upstream    skip step 3
#
# Environment:
#   LIKEC4        official CLI (default: `likec4` on PATH), e.g. .oracle/node_modules/.bin/likec4
#   LIKEC4_LINT   likec4-lint binary (default: build target/release/likec4-lint)
#
# Exit status: 0 no drift, 1 drift found, 2 usage or setup error.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
UPSTREAM_REPO=https://github.com/likec4/likec4

# Fixtures whose expected output is the official formatter's second pass (its fixed point),
# because its first pass is unstable there. See tests/fixtures/formatter-quirks/README.md and
# the "Formatter compatibility" section of README.md.
SECOND_PASS_FIXTURES=(
  formatter-cli/18-metadata-array-values
  formatter-quirks/same-gap-last-wins
)

usage() {
  sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

UPSTREAM=""
NO_UPSTREAM=0
while [ $# -gt 0 ]; do
  case "$1" in
    --upstream)
      [ $# -ge 2 ] || { usage >&2; exit 2; }
      UPSTREAM=$2
      shift 2
      ;;
    --no-upstream)
      NO_UPSTREAM=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

LIKEC4=${LIKEC4:-likec4}
if ! command -v "$LIKEC4" >/dev/null 2>&1; then
  echo "official likec4 CLI not found: $LIKEC4 (set LIKEC4=path/to/likec4)" >&2
  exit 2
fi
case "$LIKEC4" in
  */*) LIKEC4=$(cd "$(dirname "$LIKEC4")" && pwd)/$(basename "$LIKEC4") ;;
esac

if [ -z "${LIKEC4_LINT:-}" ]; then
  (cd "$ROOT" && cargo build --release --locked -p likec4-lint)
  LIKEC4_LINT=$ROOT/target/release/likec4-lint
fi

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

echo "official likec4: $("$LIKEC4" --version 2>/dev/null | tail -n 1)"
echo "likec4-lint:     $LIKEC4_LINT"

FAILURES=()

# fail <title> [file-with-details]
fail() {
  FAILURES+=("$1")
  echo
  echo "DRIFT: $1"
  if [ $# -ge 2 ] && [ -s "$2" ]; then
    head -n 200 "$2"
    if [ "$(wc -l <"$2")" -gt 200 ]; then
      echo "... (truncated, $(wc -l <"$2") lines)"
    fi
  fi
}

# official_fmt <dir>: format every document under <dir> in place with the official CLI.
# The CLI exits 0 even when it logs unresolved references or syntax errors (a document with a
# syntax error is left unchanged), so a non-zero exit is a genuine failure.
official_fmt() {
  local dir=$1
  if ! (cd "$(dirname "$dir")" && "$LIKEC4" fmt "$(basename "$dir")" >"$WORK/fmt.log" 2>&1); then
    echo "likec4 fmt failed on $dir:" >&2
    cat "$WORK/fmt.log" >&2
    exit 2
  fi
}

is_second_pass() {
  local fixture
  for fixture in "${SECOND_PASS_FIXTURES[@]}"; do
    [ "$fixture" = "$1" ] && return 0
  done
  return 1
}

# 1. corpus
echo
echo "== tests/corpus/examples -> tests/corpus/examples-formatted"
cp -R "$ROOT/tests/corpus/examples" "$WORK/corpus"
official_fmt "$WORK/corpus"
if diff -r -u "$ROOT/tests/corpus/examples-formatted" "$WORK/corpus" >"$WORK/corpus.diff"; then
  echo "ok"
else
  fail "tests/corpus/examples-formatted differs from the official output" "$WORK/corpus.diff"
fi

# 2. fixtures
for dir in formatter-cli formatter-quirks; do
  echo
  echo "== tests/fixtures/$dir"
  count=0
  for input in "$ROOT/tests/fixtures/$dir"/*.input.c4; do
    name=$(basename "$input" .input.c4)
    expected=$ROOT/tests/fixtures/$dir/$name.expected.c4
    case_dir=$WORK/$dir/$name
    mkdir -p "$case_dir"
    cp "$input" "$case_dir/$name.c4"
    official_fmt "$case_dir"
    if is_second_pass "$dir/$name"; then
      official_fmt "$case_dir"
    fi
    if ! diff -u "$expected" "$case_dir/$name.c4" >"$case_dir.diff"; then
      fail "tests/fixtures/$dir/$name.expected.c4 differs from the official output" "$case_dir.diff"
    fi
    count=$((count + 1))
  done
  echo "$count fixture(s) checked"
done

# 3. upstream examples
if [ "$NO_UPSTREAM" -eq 0 ]; then
  echo
  echo "== upstream examples"
  if [ -z "$UPSTREAM" ]; then
    git clone --quiet --depth 1 "$UPSTREAM_REPO" "$WORK/upstream-src"
    UPSTREAM=$WORK/upstream-src
  fi
  if [ ! -d "$UPSTREAM/examples" ]; then
    echo "not a likec4 checkout (no examples/ directory): $UPSTREAM" >&2
    exit 2
  fi
  echo "upstream: $UPSTREAM ($(git -C "$UPSTREAM" rev-parse --short HEAD 2>/dev/null || echo 'not a git checkout'))"
  cp -R "$UPSTREAM/examples" "$WORK/upstream-examples"

  "$LIKEC4_LINT" lint --quiet --color never "$WORK/upstream-examples" >"$WORK/upstream-lint.txt" 2>&1 || true
  syntax_errors=$(grep -c '^error\[syntax-error\]' "$WORK/upstream-lint.txt" || true)
  if [ "$syntax_errors" -eq 0 ]; then
    echo "parser: ok"
  else
    grep -A 4 '^error\[syntax-error\]' "$WORK/upstream-lint.txt" >"$WORK/upstream-syntax.txt" || true
    fail "upstream examples: likec4-lint reports $syntax_errors syntax error(s) the official CLI accepts" "$WORK/upstream-syntax.txt"
  fi

  # Two official passes: the first pass is not a fixed point for a few constructs (see
  # SECOND_PASS_FIXTURES), likec4-lint lands on the fixed point directly.
  official_fmt "$WORK/upstream-examples"
  official_fmt "$WORK/upstream-examples"
  if "$LIKEC4_LINT" format --check --diff --color never "$WORK/upstream-examples" >"$WORK/upstream-fmt.txt" 2>&1; then
    echo "formatter: ok"
  else
    fail "upstream examples: likec4-lint would reformat the official formatter's output" "$WORK/upstream-fmt.txt"
  fi
fi

echo
if [ ${#FAILURES[@]} -eq 0 ]; then
  echo "no drift"
  exit 0
fi
echo "${#FAILURES[@]} drift(s) found:"
for f in "${FAILURES[@]}"; do
  echo "  - $f"
done
exit 1
