# Packaging and release

## Releasing

Two steps, because main only changes through reviewed pull requests ([`release.sh`](../release.sh), as brnr does it):

1. `./release.sh <version | major | minor | patch>` from an up-to-date main: checks formatting, lints and tests, then on a `release-<version>` branch bumps `Cargo.toml`, `Cargo.lock` and the extension's `package.json`/`package-lock.json`, dates CHANGELOG.md's Unreleased section for the version, and opens the release pull request. Edit the release notes there.
2. Once it is merged, `./release.sh tag`: checks CI passed on main, tags `v<version>`, follows the release workflow, then checks the release has every platform's binary, the tap points at the tag and crates.io has the version.

A release's notes are its section of [CHANGELOG.md](../CHANGELOG.md) ([Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/)): a pull request that changes what users notice adds its entry under `## [Unreleased]`, and `release.sh <bump>` refuses an empty one. The release workflow fails before building if the tag doesn't match `Cargo.toml` and `extension/package.json`, CHANGELOG.md has no section for it, or the crate doesn't package (`cargo publish --dry-run`). `./release.sh notes` prints what the release pull request would say; `./release.sh changelog [<version>]` prints a version's notes. `extension/CHANGELOG.md` is the Marketplace's and is kept by hand; the release pull request says whether it has the version.

## Channels

| Channel | How |
|---|---|
| GitHub Releases | Tag `vX.Y.Z` → `.github/workflows/release.yml` builds `tricks-<target>.tar.gz` + `.sha256` for macOS (arm64, x64), Linux (x64, arm64) and Windows (x64, arm64). `tricks upgrade` consumes these. |
| crates.io | [`tricks`](https://crates.io/crates/tricks): `cargo install tricks --locked`. The release workflow's `crates` job publishes each tag with [trusted publishing](https://crates.io/docs/trusted-publishing): crates.io trusts `release.yml` in `new-tricks/tricks`, in the `crates-io` environment, and the job trades GitHub's OIDC token for a short-lived crates.io token (`rust-lang/crates-io-auth-action`, revoked when the job ends). No token is stored. The package `include`s only what the build needs: `src/`, `skills/new-tricks/` (which `tricks init --agent-skill` installs), the README, LICENSE and NOTICE. A version can be yanked but not deleted or replaced. `upgrade` defers to `cargo install`. |
| Homebrew | Tap [`new-tricks/homebrew-tap`](https://github.com/new-tricks/homebrew-tap), formula `Formula/tricks.rb` (was `newtricks` until 0.6; the tap's `formula_renames.json` migrates existing installs; source copy in `packaging/homebrew/`). `brew install new-tricks/tap/tricks`. The release workflow renders `packaging/homebrew/tricks.rb` with the tag's source tarball `url`/`sha256` (`render.sh`), runs `brew style`, and pushes it to the tap using the `HOMEBREW_TAP_DEPLOY_KEY` secret (a write deploy key on the tap). Edit the template here, never the tap directly. Move to homebrew-core later. `upgrade` defers to `brew upgrade`. |
| VS Code Marketplace | Platform-specific VSIX per target, each bundling its binary in `extension/bin/`. Needs the `VSCE_PAT` secret and a registered publisher (the placeholder `publisher` in `extension/package.json` is `newtricks`). Without the secret the step is skipped; upload the VSIX files by hand at marketplace.visualstudio.com/manage. Temporary — see [Later](#later). |
| Open VSX (Cursor, Windsurf, VSCodium) | Same VSIX files; needs the `OVSX_PAT` secret. |

## Before the first release

1. **crates.io** (once): trusted publishing can only be set up for a crate that exists, so the first version is published by hand. Once the first release pull request is merged, and before `./release.sh tag`, create a crates.io API token scoped to `publish-new` (crate `tricks`), run `cargo publish --locked` from that main, then revoke the token. The `crates` job then finds the version published and skips it. On crates.io, under the crate's Settings → Trusted Publishing, add GitHub: owner `new-tricks`, repository `tricks`, workflow `release.yml`, environment `crates-io`. In this repository's Settings → Environments, create `crates-io` (optionally limited to `v*` tags). Later releases need no token; `crates` re-runs (Actions → Release → *Run workflow*) skip a version already published.
1. ~~GitHub org and repositories~~ — done: `new-tricks/tricks` and `new-tricks/homebrew-tap` (lowercase, matching the CLI, canonical skill IDs and GHCR/npm naming). The tap's formula is updated by the release workflow.
2. Register a VS Code Marketplace publisher and an Open VSX namespace; set `publisher`.
3. macOS signing and notarization: add an Apple Developer ID certificate (`APPLE_CERT_P12`, `APPLE_CERT_PASSWORD`, `APPLE_TEAM_ID`, `APPLE_ID`, `APPLE_APP_PASSWORD`) and replace the placeholder step with `codesign --options runtime` + `xcrun notarytool submit --wait`. Unsigned CLI binaries work when installed via Homebrew or from a tarball after `xattr -d com.apple.quarantine`.
4. Windows: optionally sign with Azure Trusted Signing.

## Re-running a release step

Actions → Release → *Run workflow* with an existing tag re-runs marketplace publishing, the Homebrew update and the crates.io publish (already published versions are skipped), without rebuilding.

## Later

- **Switch VS Code Marketplace publishing to a Microsoft Entra identity, with no secret stored.** Publish from GitHub Actions with `vsce publish --azure-credential`, authenticated by `azure/login` over OIDC (a federated credential on an Entra app registration or user-assigned managed identity, trusted for this repository's release workflow and added as a member of the `newtricks` publisher). Then delete the `VSCE_PAT` secret. Reasons: Azure DevOps is retiring PATs scoped to all accessible organizations, which Marketplace publishing requires; a stored PAT expires within a year and is a long-lived credential. Blocker: it needs an Entra directory (tenant) — a personal Microsoft account has none by default, and signing in to the Azure Portal with one fails with `AADSTS16000`.

## Local builds

```bash
cargo build --release                      # target/release/tricks
cargo package --locked --list              # what crates.io would get
cd extension && npm ci && npx tsc -p . && npx vsce package --no-dependencies
```

To test a platform VSIX locally, copy the release binary to `extension/bin/tricks` before packaging.
