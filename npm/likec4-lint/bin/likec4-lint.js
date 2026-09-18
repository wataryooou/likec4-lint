#!/usr/bin/env node
'use strict';

// Thin launcher: locates the platform-specific binary installed through one of the
// optionalDependencies of this package and execs it with the same arguments.

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const PLATFORM_PACKAGES = {
  'darwin-arm64': '@likec4-lint/darwin-arm64',
  'darwin-x64': '@likec4-lint/darwin-x64',
  'linux-arm64': '@likec4-lint/linux-arm64',
  'linux-x64': '@likec4-lint/linux-x64',
  'win32-x64': '@likec4-lint/win32-x64',
};

function fail(message) {
  process.stderr.write(`likec4-lint: ${message}\n`);
  process.exit(1);
}

function binaryPath() {
  const key = `${process.platform}-${process.arch}`;
  const pkg = PLATFORM_PACKAGES[key];
  if (!pkg) {
    fail(
      `no prebuilt binary for ${key}. Supported: ${Object.keys(PLATFORM_PACKAGES).join(', ')}.\n` +
        'Build from source instead: cargo install likec4-lint',
    );
  }
  const exe = process.platform === 'win32' ? 'likec4-lint.exe' : 'likec4-lint';
  let manifest;
  try {
    manifest = require.resolve(`${pkg}/package.json`);
  } catch {
    fail(
      `the platform package ${pkg} is not installed.\n` +
        'It is an optionalDependency of likec4-lint. Make sure optional dependencies are not ' +
        'disabled (npm --no-optional, pnpm/yarn equivalents) and reinstall. On Linux only glibc ' +
        'is supported (Alpine/musl is not); build from source with: cargo install likec4-lint',
    );
  }
  const bin = path.join(path.dirname(manifest), 'bin', exe);
  if (!fs.existsSync(bin)) {
    fail(`${pkg} is installed but ${bin} is missing. Try reinstalling likec4-lint.`);
  }
  return bin;
}

const result = spawnSync(binaryPath(), process.argv.slice(2), {
  stdio: 'inherit',
  windowsHide: true,
});

if (result.error) {
  fail(result.error.message);
}
if (result.signal) {
  // Re-raise the signal that killed the child so shells see the same termination.
  process.kill(process.pid, result.signal);
}
process.exit(result.status === null ? 1 : result.status);
