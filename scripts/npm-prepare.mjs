#!/usr/bin/env node
// Stamps a real version into the checked-in npm package.json files ahead of
// publishing, and stages README.md / LICENSE into each package directory (npm
// bundles those two files regardless of the "files" field, but they are not
// tracked inside npm/ in git, so they must be copied in before `npm publish`).
//
// Usage:
//   node scripts/npm-prepare.mjs <version>
//
// <version> must be a bare semver (optionally with a prerelease suffix), e.g.
// 0.1.0 or 0.1.0-beta.1. This is normally invoked from CI (release.yml) with
// the version derived from the git tag, but can be run locally to inspect
// what a release would look like:
//   node scripts/npm-prepare.mjs 0.1.0
//   git diff npm/
//   git checkout -- npm/ && git clean -fdX npm/

import { readFileSync, writeFileSync, copyFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const VERSION_RE = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;

const repoRoot = path.resolve(fileURLToPath(new URL('.', import.meta.url)), '..');
const npmRoot = path.join(repoRoot, 'npm');

const PLATFORM_PACKAGES = [
  'darwin-arm64',
  'darwin-x64',
  'linux-arm64',
  'linux-x64',
  'win32-x64',
];

function main() {
  const version = process.argv[2];
  if (!version || !VERSION_RE.test(version)) {
    process.stderr.write(
      `npm-prepare: expected a semver version argument matching ${VERSION_RE}, got: ${version ?? '<none>'}\n`,
    );
    process.exit(1);
  }

  const packageDirs = [
    path.join(npmRoot, 'likec4-lint'),
    ...PLATFORM_PACKAGES.map((key) => path.join(npmRoot, '@likec4-lint', key)),
  ];

  // Main package: bump its own version and pin every optionalDependency to the
  // same version.
  writeJson(path.join(npmRoot, 'likec4-lint', 'package.json'), (pkg) => {
    pkg.version = version;
    if (pkg.optionalDependencies) {
      for (const dep of Object.keys(pkg.optionalDependencies)) {
        pkg.optionalDependencies[dep] = version;
      }
    }
  });

  // Platform packages: bump their own version.
  for (const key of PLATFORM_PACKAGES) {
    writeJson(path.join(npmRoot, '@likec4-lint', key, 'package.json'), (pkg) => {
      pkg.version = version;
    });
  }

  // Stage README.md / LICENSE into every package directory.
  for (const dir of packageDirs) {
    copyFileSync(path.join(repoRoot, 'README.md'), path.join(dir, 'README.md'));
    copyFileSync(path.join(repoRoot, 'LICENSE'), path.join(dir, 'LICENSE'));
  }

  process.stdout.write(`npm-prepare: stamped version ${version} into ${packageDirs.length} package(s)\n`);
}

function writeJson(file, mutate) {
  const pkg = JSON.parse(readFileSync(file, 'utf8'));
  mutate(pkg);
  writeFileSync(file, `${JSON.stringify(pkg, null, 2)}\n`);
}

main();
