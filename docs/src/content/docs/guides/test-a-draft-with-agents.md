---
title: Test a draft with agents
description: Compare the current version of a skill with an experiment, side by side in real agents, then merge the experiment or drop it.
---

In this guide you keep the current `changelog-writer` linked for every project, test a terser draft of it in one project as an experiment, and then merge the experiment or throw it away. At the end, you try an upstream skill in a project before deciding whether to vendor it.

You need a [source repo](/tricks/concepts/source-repo/) (here `~/code/my-skills`) with `changelog-writer` committed on `main`, and a project to test in (`~/code/my-app`). The examples use Claude Code. See [Agents](/tricks/concepts/agents/) to link for others.

## 1. Link the current version everywhere

```bash
cd ~/code/my-skills
tricks link changelog-writer
```

```text
linked changelog-writer into user scope from main (working tree, live)
  claude   ~/.claude/skills/changelog-writer (link)
see links with `tricks list --links`; remove with `tricks unlink changelog-writer`
```

The user scope link points at your main checkout, so every project's agent loads the `main` version and sees your edits as soon as you save.

## 2. Start an experiment

```bash
tricks experiment start changelog-writer@terse
```

```text
$ git worktree add -q -b experiment/changelog-writer/terse .tricks/work/experiment--changelog-writer--terse HEAD
~/code/my-skills/.tricks/work/experiment--changelog-writer--terse/skills/changelog-writer
experiment changelog-writer@terse on branch experiment/changelog-writer/terse (`cd "$(tricks experiment start changelog-writer@terse)"` or `--shell` to work there)
  try it with agents: `tricks link changelog-writer@terse --to <project>`; commit with `tricks experiment commit changelog-writer@terse -m "…"`
  then `tricks experiment merge changelog-writer@terse` (or `--pr`), or `tricks experiment discard changelog-writer@terse`
```

The experiment is a git worktree of branch `experiment/changelog-writer/terse` at `.tricks/work/experiment--changelog-writer--terse`, which git ignores. Open it in your editor, or `cd "$(tricks experiment start changelog-writer@terse)"`. Editing it doesn't change what your agents load yet.

## 3. Link the experiment into one project

```bash
tricks link changelog-writer@terse --to ~/code/my-app
tricks list --links
```

```text
linked changelog-writer into ~/code/my-app from experiment/changelog-writer/terse (worktree, live, pinned)
  claude   ~/code/my-app/.claude/skills/changelog-writer (link)
see links with `tricks list --links`; remove with `tricks unlink changelog-writer`
user scope:
  changelog-writer             claude   ~/.claude/skills/changelog-writer (link)  main (working tree, live)
~/code/my-app:
  changelog-writer             claude   ~/code/my-app/.claude/skills/changelog-writer (link)  experiment/changelog-writer/terse (worktree, live, pinned)
```

The project link is pinned to the experiment, and the user scope link stays on `main`. The project link is added to `my-app`'s `.git/info/exclude`, so `git status` in `my-app` stays clean.

:::caution[Same name in two places]
When a skill in user scope and a project skill have the same name, Claude Code uses the one in user scope (your personal one). To make sure the agent in `my-app` loads the experiment, remove the user scope link while you compare (`tricks unlink changelog-writer --global`), and compare against `main` in another project instead (`tricks link changelog-writer --to ~/code/other-app`). Run `tricks link changelog-writer` again when you're done.
:::

## 4. Try it and iterate

Open the agent in the project and ask for release notes:

```bash
cd ~/code/my-app && claude
```

Then edit the experiment's `SKILL.md`. The link points into the worktree, so the agent in `my-app` gets what you saved. Start a new session if it has already loaded the skill. Other projects keep getting `main`. Compare the two by running the same prompt in `my-app` and in another project.

While you're iterating, you can check where each version comes from with `tricks list`:

```text
source repo my-skills (~/code/my-skills, branch main)
  changelog-writer       original · experiments: terse · linked: ~/code/my-app (experiment/changelog-writer/terse), user scope (main)
  skill-creator          from anthropics/skills//skills/skill-creator · lint 0E/2W
```

## 5. Commit the experiment

```bash
tricks experiment commit changelog-writer@terse -m "Terser output"
```

```text
$ git -C .tricks/work/experiment--changelog-writer--terse add -A
$ git -C .tricks/work/experiment--changelog-writer--terse commit -q -m 'Terser output'
committed 43737f7bc on experiment/changelog-writer/terse
```

This commits everything you changed in the worktree, on the experiment's branch. Run it as often as you like, like any commit. The link keeps pointing at the worktree, so nothing changes for the agent. `tricks experiment list` shows how far the experiment has come:

```text
changelog-writer@terse         1 unmerged commit(s) · 1 link(s)
  ~/code/my-skills/.tricks/work/experiment--changelog-writer--terse/skills/changelog-writer
```

## 6. Compare the versions

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

`terse` is the experiment's name. `diff` finds its branch for you.

## 7a. Keep it: merge

```bash
tricks experiment merge changelog-writer@terse
tricks list --links
```

```text
$ git merge --no-ff --no-commit -q experiment/changelog-writer/terse
$ git commit -q -m 'Merge experiment changelog-writer@terse'
$ git worktree remove .tricks/work/experiment--changelog-writer--terse
$ git branch -d -q experiment/changelog-writer/terse
merged changelog-writer@terse into main (ac2ac2d5a)
  → ~/code/my-app/.claude/skills/changelog-writer (link)
  removed branch experiment/changelog-writer/terse and its worktree
user scope:
  changelog-writer             claude   ~/.claude/skills/changelog-writer (link)  main (working tree, live)
~/code/my-app:
  changelog-writer             claude   ~/code/my-app/.claude/skills/changelog-writer (link)  main (working tree, live)
```

`merge` merged the experiment branch into `main` with a regular merge commit, moved the pinned link back to the main checkout, which now has the change, and removed the branch and its worktree. To open a pull request on your source repo's remote instead, use `--pr`. [Merge it back](/tricks/concepts/experiments/#merge-it-back) covers the options and conflicts.

## 7b. Drop it: discard

If the experiment isn't better, throw it away. The user scope link never changed.

```bash
tricks experiment discard changelog-writer@terse
```

```text
  1 commit(s) not merged into main
Discard experiment changelog-writer@terse? [y/N]
```

Confirm, and `discard` removes the worktree and the branch, and moves the project link back to the main checkout:

```text
$ git worktree remove --force .tricks/work/experiment--changelog-writer--terse
$ git branch -D -q experiment/changelog-writer/terse
discarded changelog-writer@terse (branch experiment/changelog-writer/terse)
  → ~/code/my-app/.claude/skills/changelog-writer (link)
```

Run `tricks unlink changelog-writer --to ~/code/my-app` if you don't want the skill in that project any more.

## Test an upstream skill before vendoring it

Before you [vendor](/tricks/guides/customize-an-upstream-skill/) a skill someone else wrote, try it as is in a project. A trial is an exact revision from the store, and it's never updated:

```bash
cd ~/code/my-app
tricks try anthropics/skills//webapp-testing
tricks list --trials
```

```text
trying webapp-testing into ~/code/my-app
  claude   ~/code/my-app/.claude/skills/webapp-testing (link)
see trials with `tricks list --trials`; remove with `tricks untry webapp-testing`
~/code/my-app:
  anthropics/skills//skills/webapp-testing claude   ~/code/my-app/.claude/skills/webapp-testing (link)
```

Use it with the agent in `my-app`. Then remove the trial and, if you want to keep and customize the skill, vendor it into your source repo:

```bash
tricks untry webapp-testing
cd ~/code/my-skills
tricks vendor anthropics/skills//webapp-testing
```

```text
removed ~/code/my-app/.claude/skills/webapp-testing
added webapp-testing at skills/webapp-testing
  upstream github.com/anthropics/skills//skills/webapp-testing @ 33375500b
  licence  Apache-2.0 [allow]
  risk     4 script(s); 3 URL reference(s)
  not committed yet: review and `git commit` when ready; `tricks link webapp-testing` to try it
```

From now on it's a source repo skill: you `link` it, start experiments as above, and [merge upstream changes](/tricks/concepts/upstream/) into it.
