#!/usr/bin/env bash
# Network-free smoke test for the npm launcher (npm/likec4-lint/bin/likec4-lint.js).
#
# Builds a throwaway node_modules layout by hand -- the way npm would lay out
# likec4-lint plus its resolved platform optionalDependency -- and runs the
# shim against a real (already built) likec4-lint binary. This does not touch
# the network or any registry.
#
# Usage: scripts/npm-smoke.sh [path/to/likec4-lint binary]
#        default: target/release/likec4-lint (or .exe on Windows)
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

key="$(node -p "process.platform + '-' + process.arch")"
case "$key" in
  win32-*) exe_name="likec4-lint.exe" ;;
  *) exe_name="likec4-lint" ;;
esac

binary="${1:-target/release/${exe_name}}"

platform_pkg_dir="npm/@likec4-lint/${key}"
if [ ! -f "${platform_pkg_dir}/package.json" ]; then
  echo "npm-smoke: no platform package for ${key} (expected ${platform_pkg_dir}/package.json)" >&2
  exit 1
fi

if [ ! -f "$binary" ]; then
  echo "npm-smoke: binary not found at ${binary}" >&2
  echo "npm-smoke: build it first, e.g. cargo build --release --locked -p likec4-lint" >&2
  exit 1
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# --- lay out node_modules by hand, as npm would after resolving optionalDependencies ---
mkdir -p "$tmp/node_modules/@likec4-lint/${key}/bin"
cp "${platform_pkg_dir}/package.json" "$tmp/node_modules/@likec4-lint/${key}/package.json"
cp "$binary" "$tmp/node_modules/@likec4-lint/${key}/bin/${exe_name}"
chmod 755 "$tmp/node_modules/@likec4-lint/${key}/bin/${exe_name}"

mkdir -p "$tmp/node_modules/likec4-lint/bin"
cp "npm/likec4-lint/package.json" "$tmp/node_modules/likec4-lint/package.json"
cp "npm/likec4-lint/bin/likec4-lint.js" "$tmp/node_modules/likec4-lint/bin/likec4-lint.js"
chmod 755 "$tmp/node_modules/likec4-lint/bin/likec4-lint.js"

shim="$tmp/node_modules/likec4-lint/bin/likec4-lint.js"

# --- A: --version is forwarded to the real binary ---
version_output="$(node "$shim" --version)"
case "$version_output" in
  "likec4-lint "*) ;;
  *)
    echo "npm-smoke: expected \`node shim --version\` to start with 'likec4-lint ', got: ${version_output}" >&2
    exit 1
    ;;
esac

# --- B: exit codes propagate through the shim ---
if ! node "$shim" lint --list-rules >/dev/null; then
  echo "npm-smoke: expected \`lint --list-rules\` to exit 0 through the shim" >&2
  exit 1
fi

unformatted="$tmp/unformatted.c4"
printf 'specification {\nelement system\n}\n' > "$unformatted"
set +e
node "$shim" format --check "$unformatted" >/dev/null 2>&1
check_status=$?
set -e
if [ "$check_status" -ne 1 ]; then
  echo "npm-smoke: expected \`format --check\` on an unformatted file to exit 1 through the shim, got ${check_status}" >&2
  exit 1
fi

# --- C: the platform package tarball preserves the executable bit (skip on Windows: tar/chmod bits are not meaningful there) ---
if [ "${key%%-*}" != "win32" ]; then
  pack_dir="$tmp/pack/@likec4-lint/${key}"
  mkdir -p "$pack_dir/bin"
  cp "${platform_pkg_dir}/package.json" "$pack_dir/package.json"
  cp "$binary" "$pack_dir/bin/${exe_name}"
  chmod 755 "$pack_dir/bin/${exe_name}"

  tarball="$(cd "$pack_dir" && npm pack --silent 2>/dev/null)"
  tarball_path="$pack_dir/$tarball"
  mode_line="$(tar -tzvf "$tarball_path" | grep "bin/${exe_name}$" || true)"
  if [ -z "$mode_line" ]; then
    echo "npm-smoke: could not find bin/${exe_name} in packed tarball ${tarball_path}" >&2
    exit 1
  fi
  case "$mode_line" in
    *x*) ;;
    *)
      echo "npm-smoke: bin/${exe_name} lost its executable bit in the packed tarball: ${mode_line}" >&2
      exit 1
      ;;
  esac
fi

echo "npm smoke test passed (${key})"
