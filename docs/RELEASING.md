# Releasing

`likec4-lint` publishes to two registries from one tag push: crates.io (the
Rust crates) and npm (`likec4-lint` plus five `@likec4-lint/<platform>`
binary packages). Both are driven by `.github/workflows/release.yml`.

## Version bump

1. Bump `version` under `[workspace.package]` in `Cargo.toml`.
2. Bump the `version` of each path dependency under `[workspace.dependencies]`
   (`likec4-syntax`, `likec4-fmt`, `likec4-rules`) to match.
3. Run `cargo update -w` to refresh `Cargo.lock`.
4. Leave everything under `npm/` alone. Every checked-in `npm/**/package.json`
   stays pinned at `0.0.0`; CI stamps the real version in at release time (see
   below).
5. Commit, then tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.

## What the tag push does

Pushing a `vX.Y.Z` tag runs `release.yml`:

1. `check-version` -- fails the run if the tag does not match
   `[workspace.package].version` in `Cargo.toml`.
2. `build` -- builds the `likec4-lint` binary for five targets (Linux
   x64/arm64, macOS x64/arm64, Windows x64) and uploads each as a
   `.tar.gz`/`.zip` artifact.
3. `release` -- downloads all build artifacts and creates a GitHub Release
   with generated notes.
4. `publish` and `publish-npm` -- run in parallel, both `needs: release`:
   - `publish` runs `cargo publish --workspace` against crates.io.
   - `publish-npm` stages each platform binary into
     `npm/@likec4-lint/<platform>/bin/`, runs
     `node scripts/npm-prepare.mjs "$version"` to stamp the version into the
     six `package.json` files, then publishes the five platform packages
     followed by the main `likec4-lint` package.

## One-time setup

### crates.io

Add a `CARGO_REGISTRY_TOKEN` repository secret. Without it, the `publish` job
fails on purpose (see the comment in `release.yml`) rather than silently
skipping, so that "Re-run failed jobs" works once the secret is added.

### npm

npm publishing uses trusted publishing (OIDC) -- no npm token is stored as a
secret. This needs one-time setup before the first tag push:

1. Create the `likec4-lint` npm organization (scope `@likec4-lint`). On the
   free plan, every scoped package must be published with public access,
   which is why every `package.json` under `npm/` sets
   `"publishConfig": { "access": "public" }`.
2. Enable two-factor authentication on the publishing npm account.
3. Bootstrap: reserve every package name and satisfy trusted publishing's
   "the package must already exist" prerequisite by publishing the
   checked-in `0.0.0` placeholders directly from a local checkout. Platform
   packages first, since the main package's `optionalDependencies` reference
   them (npm does not enforce this at publish time, but it keeps the
   registry state consistent with the dependency graph):

   ```sh
   npm login
   for dir in npm/@likec4-lint/*/ npm/likec4-lint/; do npm publish "$dir"; done
   ```

4. Register `release.yml` as the trusted publisher of each package (requires
   npm >= 11.15):

   ```sh
   for pkg in @likec4-lint/darwin-arm64 @likec4-lint/darwin-x64 @likec4-lint/linux-arm64 @likec4-lint/linux-x64 @likec4-lint/win32-x64 likec4-lint; do
     npx npm@latest trust github "$pkg" --repo wataryooou/likec4-lint --file release.yml --allow-publish --yes
   done
   npx npm@latest trust list likec4-lint
   ```

5. After the first real (non-placeholder) release has published
   successfully, deprecate the bootstrap placeholders so users who happen to
   resolve `0.0.0` see a warning instead of an empty binary package:

   ```sh
   for pkg in @likec4-lint/darwin-arm64 @likec4-lint/darwin-x64 @likec4-lint/linux-arm64 @likec4-lint/linux-x64 @likec4-lint/win32-x64 likec4-lint; do
     npm deprecate "$pkg@0.0.0" "bootstrap placeholder, install a newer version"
   done
   ```

## Recovering from a failed release run

Do not push a new tag for the same version: crates.io and npm both
permanently reject re-publishing an existing version, so a new tag would
need a version bump anyway.

- If the failure was environmental (a registry hiccup, a missing secret),
  fix it and use "Re-run failed jobs" on the *same* tag's run in the GitHub
  Actions UI. `check-version`, `build` and `release` are safe to re-run.
- If the failure is in the workflow itself, re-running does not help: a
  re-run uses the workflow file as of the tag. Fix the workflow on `main`,
  then publish the npm packages of the existing release from there:

  ```sh
  gh workflow run release.yml -f tag=vX.Y.Z
  ```

  This runs only the `publish-npm` job. It downloads the archives from the
  GitHub Release instead of rebuilding, and skips every package whose exact
  version is already on npm, so a partially published release resumes with
  the remaining packages.
- `publish` (crates.io) relies on `cargo publish --workspace`; it has not
  been verified to skip already-published crates, so a partial crates.io
  failure may need to be finished by hand. Check the job log for which
  crates were uploaded before re-running.
