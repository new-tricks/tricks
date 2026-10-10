# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

New Tricks' own conventions:

- A pull request that changes what users notice adds its entry under
  [Unreleased]. `release.sh` dates that section for the release, and a
  release's section is its GitHub release's notes.
- Before 1.0 nothing is kept for compatibility. A breaking change goes under
  Changed or Removed, starts with **Breaking:**, and says what to do instead.
- The VS Code extension's own notes, which the Marketplace shows, are in
  [extension/CHANGELOG.md](extension/CHANGELOG.md).
- 0.6.0 and earlier had their notes generated from pull request titles; their
  sections here are written after the fact.

## [Unreleased]

### Added

- `cargo install tricks --locked` installs New Tricks from
  [crates.io](https://crates.io/crates/tricks), where every release is
  published. `tricks upgrade` leaves a Cargo install to Cargo.

### Changed

- The crate is at the root of the repository: from a checkout,
  `cargo install --path . --locked` (was `--path crates/newtricks`).

## [0.7.1] - 2026-09-27

### Fixed

- `tricks update` exits 1 when it stops on conflicts or a skill fails
  (`--dry-run` still exits 0), so scripts and CI notice.
- Inside an experiment's worktree, `tricks diff <skill>` compares that
  checkout: `working` and `head` are the experiment's.
- Links that are copies say `copy` instead of `live` in `link`,
  `list --links` and the VS Code Links view.
- VS Code: `tricks.toml` completion and validation match the configuration
  New Tricks reads (policies `review`, `pinned`, `paused`;
  `lint.strict-spec`; removed keys such as `use` dropped).

### Security

- Published provenance never contains credentials or local paths.
  `PROVENANCE.md` and the `Tricks-Source:` commit trailer name the source
  repo by its `origin` remote with any `user:token@` removed, or
  `local:<folder>` when there is no remote. Before, a credentialed remote URL
  was published as is, and a source repo without a remote published its local
  path. If you published from a repo whose `origin` URL contains a token,
  check your distribution repository's history and rotate that token.

## [0.7.0] - 2026-09-27

### Added

- `tricks experiment start|list|commit|merge|discard|shell`: one skill on its
  own branch, `experiment/<skill>/<name>`, checked out in `.tricks/work/`.
  `merge` is a `git merge --no-ff` of the whole branch, then removes the
  branch and worktree (`--keep` keeps them); `--pr` opens a pull request
  instead.
- `link <skill>@<experiment|branch>` deploys that branch's checkout, live, in
  a worktree made on demand and removed with its last link;
  `link <skill>@<tag|commit>` deploys a frozen snapshot from the store.
- Git commands that change your repositories are printed on stderr as
  `$ git …` (`-v` for all of them, `-q` or `--json` for none).
- `create -b <branch>` and `vendor -b <branch>` switch the source repo to a
  branch first.
- `search --sort relevance|installs|stars|updated|name`, `--min-installs`,
  `--min-stars`, and `--facets`.
- Agents are detected on first run and written to your user config.
- VS Code: Start, Commit, Merge and Discard Experiment, and Open Experiment
  Folder.

### Changed

- **Breaking:** `link <skill>` deploys the skill in your main checkout as it
  is, uncommitted edits included; links no longer stay on the committed
  version during `tricks update`.
- `unlink` and `untry` without a skill (or with `--all`), and
  `experiment discard` when it would lose work, ask for confirmation (`--yes`
  to skip).
- At user scope a skill goes into the fewest agent directories that reach
  every configured agent, so no agent loads it twice.
- "User scope" replaces "user level" throughout; `--global` is unchanged.

### Removed

- **Breaking:** `edit`, `use` and `merge`, `tricks.work.toml`, `draft/<skill>`
  branches and skill-only merges (`--whole-branch`): use `tricks experiment`.

## [0.6.0] - 2026-09-25

### Added

- `tricks link <skill>@<branch>` pins one link to a branch, so experimenting
  on a branch no longer re-points your other links.

### Changed

- **Breaking:** `sync` is `update` and `self-update` is `upgrade`.
- **Breaking:** the Homebrew formula is `tricks` (was `newtricks`);
  `brew update` migrates existing installs.

## [0.5.0] - 2026-09-25

### Added

- `untry` and `list --trials`: trials (upstream skills you are trying) are
  kept apart from your source repo's links.

### Changed

- `view` prints the file as is; pipe it to render
  (`tricks view <skill> | glow -`).
- `edit` always works on a branch, checked out inside the source repo in
  `.tricks/work/`, and prints its path, so `cd "$(tricks edit pdf)"` works.

## [0.4.0] - 2026-09-25

### Changed

- **Breaking:** commands are named after what people guess: `info`/`view`,
  `create`/`remove`/`list`, `try`, `outdated`.

## [0.3.0] - 2026-09-25

### Changed

- **Breaking:** New Tricks works on one thing, the source repo. Installing and
  updating skills on your machine is left to APM, `npx skills` and plugin
  marketplaces.

## [0.2.1] - 2026-09-24

### Changed

- Live catalogs are searched concurrently, so cold searches finish roughly
  twice as fast.

## [0.2.0] - 2026-09-24

### Added

- Publish targets name the remote: `owner/repo`, `host/owner/repo`, a git URL
  or a path. A real publish needs `--push` or `--pr`.

### Changed

- `tricks init --agent-skill` installs the bundled skill into the repository
  being initialized, not at user scope.
- **Breaking:** clearer terms: the workspace is the **source repo**, workbench
  skills are **user skills**, and search sources are **catalogs**.

## [0.1.0] - 2026-09-24

### Added

- The first release: federated skill search with trust, licence and risk
  signals (including Tessl and ClawHub), vendoring with upstream merges, lint
  and publish, the VS Code extension and the Homebrew tap.

[Unreleased]: https://github.com/new-tricks/tricks/compare/v0.7.1...HEAD
[0.7.1]: https://github.com/new-tricks/tricks/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/new-tricks/tricks/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/new-tricks/tricks/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/new-tricks/tricks/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/new-tricks/tricks/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/new-tricks/tricks/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/new-tricks/tricks/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/new-tricks/tricks/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/new-tricks/tricks/releases/tag/v0.1.0
