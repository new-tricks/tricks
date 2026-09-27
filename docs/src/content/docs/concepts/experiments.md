---
title: Experiments
description: Try a change to a skill on its own branch and worktree, test it with agents, compare it, and merge it back or throw it away.
---

An experiment is a change to one skill that you try out on its own git branch, `experiment/<skill>/<name>`, checked out in its own worktree inside your [source repo](/tricks/concepts/source-repo/). You edit the skill there, [link](/tricks/concepts/links-and-trials/) it into a project to test it with an agent, commit, compare it with `main`, and then merge it or discard it. Your main checkout, and every link that isn't pinned to the experiment, stays as it was.

```bash
tricks experiment start changelog-writer@terse                   # branch experiment/changelog-writer/terse, in .tricks/work/
tricks link changelog-writer@terse --to ~/code/my-app            # this project's agents load the experiment
tricks experiment commit changelog-writer@terse -m "Terser output"
tricks diff changelog-writer head..terse                         # compare
tricks experiment merge changelog-writer@terse                   # merge it, remove its branch and worktree
```

For a step-by-step walkthrough, see [Test a draft with agents](/tricks/guides/test-a-draft-with-agents/).

## Start an experiment

[`tricks experiment start <skill>@<name>`](/tricks/reference/commands/experiment/) creates the branch `experiment/<skill>/<name>` from the current `HEAD` and checks it out as a git worktree at `.tricks/work/experiment--<skill>--<name>`. If the experiment exists already, `start` picks it up again. Names belong to a skill, so `changelog-writer@terse` and `skill-creator@terse` are two separate experiments.

The skill must be committed first, because the experiment starts from what is in `HEAD`:

```text
error: `greeter` is not committed yet; commit it first, so the experiment starts from it
```

`/.tricks/` is in `.gitignore` (`init` puts it there), so worktrees never show up as changes. Because they live inside the repo, they sit next to your skills in the editor, and a coding agent working in the repo can write to them.

`start` prints the skill's path in the worktree on stdout and its hints on stderr, so you can `cd` straight into it:

```bash
cd "$(tricks experiment start changelog-writer@terse)"
```

```text
$ git worktree add -q -b experiment/changelog-writer/terse .tricks/work/experiment--changelog-writer--terse HEAD
~/code/my-skills/.tricks/work/experiment--changelog-writer--terse/skills/changelog-writer
experiment changelog-writer@terse on branch experiment/changelog-writer/terse (`cd "$(tricks experiment start changelog-writer@terse)"` or `--shell` to work there)
  try it with agents: `tricks link changelog-writer@terse --to <project>`; commit with `tricks experiment commit changelog-writer@terse -m "…"`
  then `tricks experiment merge changelog-writer@terse` (or `--pr`), or `tricks experiment discard changelog-writer@terse`
```

The first line is the git command `start` ran. Commands that change a repository [echo their git commands](#the-git-commands-new-tricks-runs) like this.

`--shell` (or `tricks experiment shell <skill>@<name>` later) opens your `$SHELL` in the experiment's skill folder (`%COMSPEC%` on Windows) and sets `TRICKS_EXPERIMENT=<skill>@<name>`, for example `TRICKS_EXPERIMENT=changelog-writer@terse`, which you can show in your prompt. Type `exit` to return. `tricks` commands you run inside a worktree act on its source repo, and inside an experiment's worktree `commit`, `merge`, `discard` and `shell` work without naming the experiment.

## Test it with agents

`start` doesn't re-point any links. Your user scope link keeps deploying the main checkout. To have an agent load the experiment, pin a link to it:

```bash
tricks link changelog-writer@terse --to ~/code/my-app
```

```text
linked changelog-writer into ~/code/my-app from experiment/changelog-writer/terse (worktree, live, pinned)
  claude   ~/code/my-app/.claude/skills/changelog-writer (link)
```

The link points into the worktree, so every save shows up in the agent. [What a link deploys](/tricks/concepts/links-and-trials/#what-a-link-deploys) explains the other kinds of link.

## See your experiments

[`tricks experiment list`](/tricks/reference/commands/experiment/) shows every experiment of the source repo (or one skill's, with `tricks experiment list <skill>`) and its path:

```text
changelog-writer@terse         1 unmerged commit(s) · 1 link(s)
  ~/code/my-skills/.tricks/work/experiment--changelog-writer--terse/skills/changelog-writer
```

| Note | Meaning |
|---|---|
| `<n> unmerged commit(s)` / `no unmerged commits` | Commits on the experiment that aren't in the branch your main checkout is on |
| `uncommitted changes` | The worktree has changes git hasn't committed |
| `not checked out` | The branch exists but has no worktree. `experiment start` checks it out again |
| `<n> link(s)` | Links pinned to the experiment |
| `pull request <url> (open)` | A pull request opened with `merge --pr`, and its state |

`tricks list` also names each skill's experiments: `changelog-writer  original · experiments: terse · linked: ~/code/my-app (experiment/changelog-writer/terse), user scope (main)`.

## Commit

```bash
tricks experiment commit changelog-writer@terse -m "Terser output"
```

```text
$ git -C .tricks/work/experiment--changelog-writer--terse add -A
$ git -C .tricks/work/experiment--changelog-writer--terse commit -q -m 'Terser output'
committed 43737f7bc on experiment/changelog-writer/terse
```

`commit` stages and commits everything that changed in the experiment's worktree, not just the skill's folder, on the experiment's branch. When it detects a coding agent from its environment (Claude Code, Codex, Cursor or Copilot), it adds a `Tricks-Agent:` trailer (`--trailer 'Tricks-Agent: claude-code'`), so you can tell agent commits from your own:

```text
committed e689074f1 on experiment/changelog-writer/terse (Tricks-Agent: claude-code)
```

You can also commit in the worktree with plain git or your editor. New Tricks picks up whatever is on the branch.

## Compare versions

[`tricks diff <skill> [<from>..<to>]`](/tricks/reference/commands/diff/) compares two versions of a skill inside the source repo:

| Name | Version |
|---|---|
| An experiment name | The skill as committed on that experiment of the skill, for example `terse` for `experiment/changelog-writer/terse` |
| A branch or commit | The skill as committed there, for example `main` or `3f2a1c9` |
| `head` | The skill at `HEAD` of the checkout you run `diff` in (your main checkout, or an experiment's worktree) |
| `working` | The skill in that checkout's working tree |
| `base` | The upstream revision a vendored skill was last updated from |

The default range is `head..working`. `a..` means `a..working`, and `..b` means `head..b`.

```bash
tricks diff changelog-writer head..terse
```

```text
--- head/changelog-writer/SKILL.md
+++ terse/changelog-writer/SKILL.md
@@ -9,4 +9,4 @@
 
 1. List the pull requests merged since the last tag.
 2. Group them under Added, Changed, Fixed and Removed.
-3. Write one sentence per change, explaining what it means for users.
+3. One line per change, at most 12 words. No preamble.
```

An experiment or branch name means its committed tip. `working` and `head` are the checkout you run `diff` in: your main checkout, or, inside an experiment's worktree, the experiment. So `tricks diff <skill>` run inside the worktree shows the experiment's uncommitted changes. `diff` doesn't compare with upstream: it points you to [`outdated --diff` and `update --dry-run`](/tricks/concepts/upstream/) instead.

## Merge it back

[`tricks experiment merge <skill>@<name>`](/tricks/reference/commands/experiment/) merges the whole experiment branch into the branch your main checkout is on, with a normal `git merge --no-ff`:

```bash
tricks experiment merge changelog-writer@terse
```

```text
$ git merge --no-ff --no-commit -q experiment/changelog-writer/terse
$ git commit -q -m 'Merge experiment changelog-writer@terse'
$ git worktree remove .tricks/work/experiment--changelog-writer--terse
$ git branch -d -q experiment/changelog-writer/terse
merged changelog-writer@terse into main (ac2ac2d5a)
  → ~/code/my-app/.claude/skills/changelog-writer (link)
  removed branch experiment/changelog-writer/terse and its worktree
```

Everything the experiment committed comes over, including changes outside the skill's folder. `merge` refuses to run while the experiment or your main checkout has uncommitted changes. After the merge, links that were pinned to the experiment follow the main checkout again (they show `main (working tree, live)`), and the worktree and branch are removed.

| Option | Effect |
|---|---|
| `--keep` | Keep the branch and worktree after merging |
| `--pr` | Push the branch to the source repo's `origin` and open a pull request with `gh`, instead of merging locally |
| `-m <message>` | The merge commit message, or the pull request title. The default is `Merge experiment <skill>@<name>` |

### Conflicts

If the merge doesn't apply cleanly, it stops and leaves the conflicts for git:

```text
merging changelog-writer@verbose into main stopped on conflicts:
    CONFLICT skills/changelog-writer/SKILL.md
  resolve them and `git merge --continue`, then `tricks experiment merge changelog-writer@verbose` to clean up
```

Resolve the conflict markers and finish with `git merge --continue`. Then run `experiment merge` again. With nothing left to merge, it only cleans up: `changelog-writer@verbose is already in main`, then the links and the branch as after a clean merge.

### Merge through a pull request

```bash
tricks experiment merge changelog-writer@terse --pr
```

```text
$ git push -q -u origin experiment/changelog-writer/terse
$ gh pr create --base main --head experiment/changelog-writer/terse --title 'Merge experiment changelog-writer@terse' --body 'Merges the `changelog-writer@terse` experiment (opened by New Tricks).'
pull request for changelog-writer@terse into main: https://github.com/you/my-skills/pull/12
  the worktree stays for review fixes: commit them and run `tricks experiment merge changelog-writer@terse --pr` again
  once it lands, `tricks experiment merge changelog-writer@terse` (after pulling) or `discard` removes the experiment
```

Nothing else changes: the worktree and your links stay, so you can fix what reviewers ask for. Running `--pr` again pushes the new commits to the open pull request. While it is open, a local `merge` refuses. Once it has been merged on the server, pull, and `experiment merge` just cleans up.

## Discard it

```bash
tricks experiment discard changelog-writer@emoji
```

`discard` removes the experiment's worktree and branch. Links pinned to it follow the main checkout again. If that loses commits or uncommitted changes, it asks first. Without a terminal, it stops and tells you what would be lost:

```text
error: confirmation required: Discard experiment changelog-writer@emoji? (re-run with --yes to confirm)
  1 commit(s) not merged into main
```

```text
$ git worktree remove --force .tricks/work/experiment--changelog-writer--emoji
$ git branch -D -q experiment/changelog-writer/emoji
discarded changelog-writer@emoji (branch experiment/changelog-writer/emoji)
```

## The git commands New Tricks runs

Every command that changes a repository prints the git commands it runs on stderr, as `$ git -C <dir> …` (without `-C` when the directory is the current one). That covers worktrees, `switch`, `commit`, `merge`, `push`, deleting branches, the commits, tags and pushes of [`publish`](/tricks/concepts/publishing/), the push of `contribute`, and `gh pr create` and `gh repo fork`. You can see what happened to your repository and repeat it by hand.

`-v` (`--verbose`) also prints the read-only commands. `-q` (`--quiet`) and `--json` print none.

## Using git directly

Experiments are ordinary branches and worktrees. You can commit, rebase, cherry-pick or push them with git or your editor, and New Tricks picks up the result.

If you'd rather not use experiments at all, work on any branch with git and test it with `tricks link <skill>@<branch>`: the link follows that branch's checkout, live, and New Tricks makes a worktree for it if it isn't checked out anywhere (see [What a link deploys](/tricks/concepts/links-and-trials/#what-a-link-deploys)). `tricks diff <skill> head..<branch>` compares it, and you merge it with git as usual.
