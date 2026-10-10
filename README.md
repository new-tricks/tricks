# New Tricks

**Teach your agents new tricks.** New Tricks (`tricks`) is the design-time workbench for agent skills. You work in a **source repo** (a git repository holding your own skills and customized copies of upstream skills), and New Tricks helps you find prior art across fragmented catalogs, keep vendored skills merging upstream improvements, try experiments with real agents, lint them, and publish a repository that APM, `npx skills`, Claude plugin marketplaces, Copilot, Codex and Cursor can all install.

New Tricks does not manage the skills installed on your machine: that is what [APM](https://github.com/microsoft/apm), `npx skills` and plugin marketplaces are for, and what your published repository feeds. **Documentation: [new-tricks.github.io/tricks](https://new-tricks.github.io/tricks/)** (sources in [`docs/`](docs/)). See [SPEC.md](SPEC.md) for the full design.

```
discover ─► create / vendor ─► experiment ─► link & try with agents ─► merge ─► lint ─► publish
```

## Install

```bash
brew install new-tricks/tap/tricks
cargo install tricks --locked                  # from crates.io
cargo install --path . --locked                # from a checkout
```

Requires `git`. Uses your existing GitHub credentials (`GITHUB_TOKEN`, then `gh auth token`); nothing is stored.

The VS Code extension (also Cursor, Windsurf, VSCodium) lives in [`extension/`](extension/).

## Quick start

```bash
# Discover prior art: one search over skill repos, marketplaces, skills.sh, Tessl, ClawHub and GitHub
tricks search pdf forms --license allow --no-scripts
tricks search pdf --sort installs --min-stars 100     # --facets: counts per category, catalog, owner, licence, trust
tricks info anthropics/skills//skill-creator          # licence, risk, catalog signals, frontmatter, files
tricks view anthropics/skills//skill-creator | glow - # the content (pipe it to render)
tricks try anthropics/skills//webapp-testing          # try it in this project (git status stays clean)
tricks list --trials                                  # trials here (--all: everywhere)
tricks untry webapp-testing

# Start a source repo: any git repository
cd ~/code/my-skills && tricks init [--agent-skill]    # --agent-skill: teach this repo's agents New Tricks
tricks create changelog-writer --description "Writes release notes… Use when …"
tricks create my-skill --from ~/old/my-skill          # or take an existing folder
tricks create greeter -b add-greeter                  # on a new branch (vendor takes -b too)
tricks vendor anthropics/skills//skill-creator        # copy an upstream skill in, record its base
tricks vendor clawhub.ai/awspace/skills//pdf          # catalog-hosted skills too (SHA-256 verified per file)
tricks list                                           # skills, experiments, upstream changes

# Try your skills with real agents
tricks link                                           # every skill, user scope, from the main checkout: edits are live
tricks link changelog-writer --to ~/code/my-app       # or one skill into one project
tricks list --links                                   # this repo's links and what each deploys (--all: every source repo's)
tricks unlink                                         # remove them (asks first; --yes)

# Experiment: one skill, its own branch experiment/<skill>/<name> and worktree in .tricks/work/
cd "$(tricks experiment start changelog-writer@terse)"  # or --shell
tricks link changelog-writer@terse --to ~/code/my-app # this project's agents load it; other links stay on main
tricks link changelog-writer@v1.0.0 --to ~/code/my-app  # or pin a tag/commit (snapshot) or any branch (live)
tricks experiment commit changelog-writer@terse -m "Terser output"
tricks experiment list
tricks diff changelog-writer head..terse
tricks experiment merge changelog-writer@terse [--pr] [--keep]  # git merge --no-ff, then clean up
tricks experiment discard changelog-writer@terse

# Keep up with upstream
tricks outdated [--diff]                              # what changed upstream, with a risk summary
tricks update skill-creator [--dry-run]               # 3-way merge into yours, left uncommitted
tricks update --continue | --abort                    # after resolving conflicts
tricks diff skill-creator base..                      # what did I change?
tricks contribute skill-creator                       # offer your change upstream as a pull request

tricks lint [--fix] [--strict]                        # --strict: keys outside the spec are errors, like skills-ref
tricks remove my-skill
```

To have the bundled `new-tricks` agent skill everywhere, install it like any published skill: `npx skills add new-tricks/tricks`.

### Publish

```toml
# tricks.toml
[publish.targets.public]
repo    = "acme/my-skills-public"    # distribution repository: owner/repo, a git URL or a path
exclude = ["evals/**", "notes/**"]
```

```bash
tricks publish public --dry-run
tricks publish public --bump minor --push   # or --pr for a reviewed pull request
```

The target gets `skills/<name>/`, a Claude `marketplace.json`, `apm.yml`, `PROVENANCE.md` and `CHANGELOG.md`, and is tagged `vX.Y.Z`. Gates: committed source, zero lint errors, licence policy for vendored skills, leak check, risk diff. A vendored skill whose licence blocks publishing can be allowed with a written reason: `[skills.<name>] license-override = { justification = "…" }`.

## Skill references

```
[host/]owner/repo//path-or-name[@ref]
anthropics/skills//pdf                          → github.com/anthropics/skills//skills/pdf@<latest tag or default branch>
github.mit.edu/ist-org/skills//docx@v1.0.0
https://github.com/anthropics/skills/tree/main/skills/pdf
```

`//` separates the repository from the in-repo path (it is required). `@ref` is Go style: tag, branch, commit, or `latest`.

## Configuration

| File | Purpose |
|---|---|
| `~/.config/newtricks/tricks.toml` | User config: settings (the agents found on your machine are written on first run), catalogs (the recommended ones too), registered source repos |
| `<source-repo>/tricks.toml` / `.lock` | Source repo skills, upstreams, lint config, publish targets / recorded bases |

Data (store cache, mirrors, worktrees, `state.db`) lives in `~/Library/Application Support/newtricks` (macOS), `$XDG_DATA_HOME/newtricks` (Linux) or `%LOCALAPPDATA%\newtricks` (Windows).

Commands that change a repository print the git commands they run on stderr (`$ git …`); `-v` adds the read-only ones, `-q` and `--json` print none.

Environment overrides: `TRICKS_HOME`, `TRICKS_CONFIG_DIR`, `TRICKS_DATA_DIR`, `TRICKS_GITHUB_TOKEN`, `TRICKS_LINK_MODE` (`copy`, `link`, or `agent=mode,…`).

## Development

```bash
cargo test                      # unit + hermetic integration tests (local "GitHub" fixtures)
cargo clippy --all-targets
cd extension && npm ci && npx tsc -p . && npm test   # protocol + webview (jsdom) tests
```

Integration tests run the real binary against local repositories via `TRICKS_HOST_MAP="github.com=<dir>"`.

## Licence

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
