//! M3/M4 acceptance: source repo authoring, upstream merges, experiments, links, lint, publish, contribute.
// Asserts on symlinked placements; Windows deploys copies (spec §8), so these run on Unix.
#![cfg(unix)]

mod common;
use common::*;
use std::path::{Path, PathBuf};

const PARA: &str = "# Hello\n\nIntro paragraph.\n\n## Steps\n\n1. Greet.\n2. Wave.\n\n## Notes\n\nKeep it short.\n";

fn setup() -> (Sandbox, PathBuf, PathBuf) {
    let s = Sandbox::new();
    let up = s.upstream(
        "acme",
        "skills",
        &[
            ("skills/hello/SKILL.md", &skill_md("hello", "Greets people politely. Use when the user asks for a greeting.", PARA)),
            ("skills/hello/LICENSE", "MIT License\n\nCopyright (c) 2024 Acme\n\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the \"Software\"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.\n"),
            ("skills/secret-sauce/SKILL.md", "---\nname: secret-sauce\ndescription: Proprietary recipe helper. Use when cooking the secret sauce.\nlicense: Proprietary. LICENSE.txt has complete terms\n---\nbody\n"),
            ("skills/secret-sauce/LICENSE.txt", "© 2025 Acme. All rights reserved. You may not distribute or create derivative works.\n"),
        ],
    );
    git(&up, &["tag", "v1.0.0"]);
    let ws = s.root().join("my-skills");
    std::fs::create_dir_all(&ws).unwrap();
    git(&ws, &["init", "-q", "-b", "main"]);
    s.ok_in(&ws, &["init"]);
    commit_all(&ws, "init source repo");
    (s, up, ws)
}

/// A bare distribution repository at `github.com/<owner>/<name>` holding one README.
fn publish_remote(s: &Sandbox, owner: &str, name: &str, readme: &str) -> PathBuf {
    let remote = s.fixtures.join(owner).join(name);
    std::fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    let seed = s.root().join(format!("seed-{name}"));
    git(&s.root(), &["clone", "-q", remote.to_str().unwrap(), seed.to_str().unwrap()]);
    write(&seed.join("README.md"), readme);
    commit_all(&seed, "readme");
    git(&seed, &["push", "-q", "origin", "HEAD:main"]);
    remote
}

/// A fresh clone of a bare remote, to inspect what was pushed.
fn checkout(s: &Sandbox, remote: &Path) -> PathBuf {
    let d = tempfile::Builder::new().prefix("check-").tempdir_in(s.root()).unwrap().keep();
    git(&s.root(), &["clone", "-q", remote.to_str().unwrap(), d.join("c").to_str().unwrap()]);
    d.join("c")
}

fn set_interval_zero(s: &Sandbox) {
    let p = s.config.join("tricks.toml");
    let t = read(&p);
    if !t.contains("fetch_interval") {
        std::fs::write(&p, t.replace("[settings]", "[settings]\nfetch_interval = \"0s\"")).unwrap();
    }
}

#[test]
fn init_agent_skill_is_placed_in_the_repository_not_user_scope() {
    let s = Sandbox::new();
    let ws = s.root().join("team-skills");
    std::fs::create_dir_all(&ws).unwrap();
    git(&ws, &["init", "-q", "-b", "main"]);
    let r = s.json_in(&ws, &["init", "--agent-skill"]);
    let placed: Vec<String> = r["agent_skill"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect();
    assert_eq!(placed.len(), 2, "{r}");
    assert!(placed.iter().all(|p| p.starts_with(ws.to_str().unwrap())), "{placed:?}");
    assert!(ws.join(".claude/skills/new-tricks/SKILL.md").exists());
    assert!(!s.home.join(".claude/skills/new-tricks").exists(), "nothing at user scope");
    // Git-excluded, so the repository stays clean apart from what init itself wrote.
    let status = git(&ws, &["status", "--porcelain", "--untracked-files=all"]);
    assert!(!status.contains("new-tricks"), "{status}");
}

#[test]
fn init_vendor_and_block_class_confirmation() {
    let (s, _up, ws) = setup();
    let gi = read(&ws.join(".gitignore"));
    assert!(gi.contains("/.tricks/") && !gi.contains("tricks.work.toml"), "{gi}");
    assert!(read(&s.config.join("tricks.toml")).contains("[source-repos]"));
    let r = s.json_in(&ws, &["vendor", "acme/skills//hello"]);
    assert_eq!(r["upstream"], "github.com/acme/skills//skills/hello");
    assert_eq!(r["license"]["class"], "allow");
    assert!(ws.join("skills/hello/SKILL.md").exists());
    let m = read(&ws.join("tricks.toml"));
    assert!(m.contains("[skills.hello]") && m.contains("upstream = \"github.com/acme/skills//skills/hello\""), "{m}");
    assert!(read(&ws.join("tricks.lock")).contains("base = "));
    // Proprietary skill: vendoring needs explicit confirmation.
    let err = s.fail_in(&ws, &["vendor", "acme/skills//secret-sauce"]);
    assert!(err.contains("confirmation required") && err.contains("may prohibit"), "{err}");
    s.ok_in(&ws, &["vendor", "acme/skills//secret-sauce", "--yes"]);
    // Search/show report vendored state.
    let sh = s.json_in(&ws, &["info", "acme/skills//hello"]);
    assert_eq!(sh["vendored"], true);
    // A copy of the upstream skill made earlier: vendored from that copy, at its base.
    let base = git(&_up, &["rev-parse", "v1.0.0"]);
    let copy = s.root().join("elsewhere/hello-copy");
    write(&copy.join("SKILL.md"), &read(&ws.join("skills/hello/SKILL.md")).replace("Intro paragraph.", "My old copy."));
    let r = s.json_in(&ws, &["vendor", "acme/skills//hello", "--name", "hello-copy", "--from", copy.to_str().unwrap(), "--base", &base]);
    assert_eq!(r["name"], "hello-copy", "{r}");
    assert_eq!(r["upstream"], "github.com/acme/skills//skills/hello");
    assert_eq!(r["base"], base.as_str());
    assert!(read(&ws.join("skills/hello-copy/SKILL.md")).contains("My old copy."));
    // Folders are not vendored: they are created as local originals.
    let err = s.fail_in(&ws, &["vendor", copy.to_str().unwrap()]);
    assert!(err.contains("tricks create"), "{err}");
    let c = s.json_in(&ws, &["create", "mine", "--from", copy.to_str().unwrap()]);
    assert_eq!(c["upstream"], serde_json::Value::Null, "{c}");
    assert!(read(&ws.join("tricks.toml")).contains("[skills.mine]"));
    // Removing a skill drops its folder, manifest entry and lock entry.
    s.ok_in(&ws, &["remove", "mine", "--yes"]);
    assert!(!ws.join("skills/mine").exists());
    assert!(!read(&ws.join("tricks.toml")).contains("[skills.mine]"));
}

#[test]
fn clean_merge_preserves_customization_and_is_left_uncommitted() {
    let (s, up, ws) = setup();
    s.ok_in(&ws, &["vendor", "acme/skills//hello"]);
    commit_all(&ws, "vendor hello");
    // Customize the intro paragraph.
    let p = ws.join("skills/hello/SKILL.md");
    std::fs::write(&p, read(&p).replace("Intro paragraph.", "My custom intro.")).unwrap();
    commit_all(&ws, "customize intro");
    // Link it: a link to the main checkout deploys it as it is.
    s.ok_in(&ws, &["link", "--agents", "claude"]);
    let deployed = s.home.join(".claude/skills/hello/SKILL.md");
    assert!(read(&deployed).contains("My custom intro."));
    // Upstream changes a different paragraph and adds a file.
    let up_md = up.join("skills/hello/SKILL.md");
    std::fs::write(&up_md, read(&up_md).replace("Keep it short.", "Keep it short and friendly.")).unwrap();
    write(&up.join("skills/hello/references/tips.md"), "tips\n");
    commit_all(&up, "upstream improvements");
    git(&up, &["tag", "v1.1.0"]);
    set_interval_zero(&s);
    let out = s.json_in(&ws, &["outdated", "--diff"]);
    assert_eq!(out["items"][0]["state"], "update-available", "{out}");
    assert!(out["items"][0]["incoming"].to_string().contains("tips.md"), "{out}");
    assert!(out["diffs"].to_string().contains("Keep it short and friendly."), "incoming diff: {out}");
    assert!(git(&ws, &["status", "--porcelain"]).is_empty(), "outdated changes nothing");
    let st = s.json_in(&ws, &["list"]);
    assert_eq!(st["source_repo"]["skills"][0]["update_available"], "v1.1.0", "{st}");
    // C → R preview: shows what the merge would produce without touching the working tree.
    let cand = s.json_in(&ws, &["update", "hello", "--dry-run"]).to_string();
    assert!(cand.contains("friendly") && cand.contains("tips.md"), "{cand}");
    assert!(git(&ws, &["status", "--porcelain"]).is_empty(), "candidate must not modify the working tree");
    let r = s.json_in(&ws, &["update", "hello"]);
    assert_eq!(r["items"][0]["state"], "merged", "{r}");
    let merged = read(&p);
    assert!(merged.contains("My custom intro.") && merged.contains("Keep it short and friendly."), "{merged}");
    assert!(ws.join("skills/hello/references/tips.md").exists());
    // Left uncommitted, lock base bumped.
    assert!(!git(&ws, &["status", "--porcelain"]).is_empty());
    let up_head = git(&up, &["rev-parse", "HEAD"]);
    assert!(read(&ws.join("tricks.lock")).contains(&up_head));
    // Links to the main checkout see the merge result straight away, uncommitted.
    assert!(read(&deployed).contains("friendly"), "the link deploys the working tree as it is");
    commit_all(&ws, "merge upstream v1.1.0");
    // Diff views: B→C shows only my customization.
    let d = s.json_in(&ws, &["diff", "hello", "base.."]);
    let text = d.to_string();
    assert!(text.contains("My custom intro") && !text.contains("friendly"), "{text}");
}

#[test]
fn overlapping_change_conflicts_then_continue_or_abort() {
    let (s, up, ws) = setup();
    s.ok_in(&ws, &["vendor", "acme/skills//hello"]);
    let p = ws.join("skills/hello/SKILL.md");
    std::fs::write(&p, read(&p).replace("Keep it short.", "Keep it VERY short.")).unwrap();
    commit_all(&ws, "vendor + customize");
    let up_md = up.join("skills/hello/SKILL.md");
    std::fs::write(&up_md, read(&up_md).replace("Keep it short.", "Keep it brief.")).unwrap();
    commit_all(&up, "conflicting upstream");
    git(&up, &["tag", "v1.1.0"]);
    set_interval_zero(&s);
    let lock_before = read(&ws.join("tricks.lock"));
    // Stopping on conflicts exits non-zero, so scripts notice.
    let r = s.json_any_in(&ws, &["update"]);
    assert_eq!(r["items"][0]["state"], "conflicts", "{r}");
    assert!(read(&p).contains("<<<<<<<"));
    assert_eq!(read(&ws.join("tricks.lock")), lock_before, "base must not move while conflicted");
    // Another merge refuses while one is in progress.
    let err = s.fail_in(&ws, &["update"]);
    assert!(err.contains("in progress"), "{err}");
    // --continue refuses with markers present.
    let err = s.fail_in(&ws, &["update", "--continue"]);
    assert!(err.contains("unresolved"), "{err}");
    // Abort restores my version.
    s.ok_in(&ws, &["update", "--abort"]);
    assert!(read(&p).contains("Keep it VERY short.") && !read(&p).contains("<<<<<<<"));
    // Redo and resolve.
    let _ = s.cmd(&ws, &["update"]);
    let resolved = read(&p)
        .lines()
        .filter(|l| !l.starts_with("<<<<<<<") && !l.starts_with("=======") && !l.starts_with(">>>>>>>") && !l.contains("Keep it brief."))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&p, resolved).unwrap();
    s.ok_in(&ws, &["update", "--continue"]);
    assert!(read(&ws.join("tricks.lock")).contains(&git(&up, &["rev-parse", "HEAD"])));
}

#[test]
fn lint_blocks_publish_and_publish_generates_ecosystem_files() {
    let (s, _up, ws) = setup();
    s.ok_in(&ws, &["vendor", "acme/skills//hello"]);
    s.ok_in(
        &ws,
        &["create", "greeter", "--description", "Writes greetings for any occasion. Use when the user asks for a greeting card text."],
    );
    write(&ws.join("skills/greeter/notes/ideas.md"), "private notes\n");
    write(
        &ws.join("skills/greeter/SKILL.md"),
        &read(&ws.join("skills/greeter/SKILL.md")).replace("---\n\n#", "metadata:\n  tricks-lint-disable: NT203\n---\n\n#"),
    );
    let remote = publish_remote(&s, "acme", "acme-skills-public", "hand-written readme\n");
    let m = read(&ws.join("tricks.toml"));
    std::fs::write(
        ws.join("tricks.toml"),
        format!("{m}\n[publish.targets.public]\nrepo = \"acme/acme-skills-public\"\nskills = [\"*\"]\nexclude = [\"notes/**\"]\n"),
    )
    .unwrap();
    commit_all(&ws, "setup");

    // A lint error blocks publishing.
    let md = ws.join("skills/greeter/SKILL.md");
    let good = read(&md);
    std::fs::write(&md, good.replace("name: greeter", "name: Greeter")).unwrap();
    commit_all(&ws, "break name");
    let r = s.json_any_in(&ws, &["publish", "public", "--dry-run"]);
    assert_eq!(r["blocked"], true);
    assert!(r["gates"].to_string().contains("NT102"), "{}", r["gates"]);
    std::fs::write(&md, good).unwrap();
    commit_all(&ws, "fix name");

    // Dirty source repo blocks a real publish.
    write(&ws.join("scratch.txt"), "x");
    let o = s.cmd(&ws, &["--json", "publish", "public", "--bump", "minor", "--push", "--yes"]);
    assert!(!o.status.success());
    std::fs::remove_file(ws.join("scratch.txt")).unwrap();

    // A real publish has to say where the result goes.
    let err = s.fail_in(&ws, &["publish", "public", "--bump", "minor", "--yes"]);
    assert!(err.contains("--push") && err.contains("--pr"), "{err}");

    let r = s.json_in(&ws, &["publish", "public", "--bump", "minor", "--push", "--yes"]);
    assert_eq!(r["blocked"], false, "{r}");
    assert_eq!(r["version"], "0.1.0");
    assert_eq!(r["tag"], "v0.1.0");
    assert_eq!(r["pushed"], true);
    let target = checkout(&s, &remote);
    assert!(target.join("skills/hello/SKILL.md").exists());
    assert!(target.join("skills/hello/LICENSE").exists(), "upstream licence carried");
    assert!(!target.join("skills/greeter/notes").exists(), "exclude globs applied");
    let greeter = read(&target.join("skills/greeter/SKILL.md"));
    assert!(greeter.contains("version: \"0.1.0\"") && !greeter.contains("tricks-lint-disable"), "{greeter}");
    let mp: serde_json::Value = serde_json::from_str(&read(&target.join(".claude-plugin/marketplace.json"))).unwrap();
    assert_eq!(mp["name"], "acme-skills-public");
    assert_eq!(mp["plugins"].as_array().unwrap().len(), 1, "single plugin by default");
    assert_eq!(mp["plugins"][0]["source"], "./");
    assert_eq!(mp["plugins"][0]["strict"], false);
    assert!(read(&target.join("apm.yml")).contains("version: 0.1.0"));
    assert!(read(&target.join("PROVENANCE.md")).contains("github.com/acme/skills//skills/hello"));
    assert!(read(&target.join("CHANGELOG.md")).contains("## v0.1.0"));
    let msg = git(&target, &["log", "-1", "--format=%B"]);
    assert!(msg.contains("Tricks-Source: local:my-skills@"), "a source repo without a remote is named, not located: {msg}");
    let root = ws.canonicalize().unwrap();
    for published in [read(&target.join("PROVENANCE.md")), msg.clone()] {
        assert!(!published.contains(root.to_str().unwrap()) && !published.contains(ws.to_str().unwrap()), "local path leaked: {published}");
    }
    assert_eq!(git(&remote, &["tag", "--list"]), "v0.1.0");

    // Re-publish after deselecting a skill: removes it, keeps hand-added files.
    let m = read(&ws.join("tricks.toml")).replace("skills = [\"*\"]", "skills = [\"greeter\"]");
    std::fs::write(ws.join("tricks.toml"), m + "\n[publish.targets.public.plugins]\n").unwrap();
    commit_all(&ws, "only greeter");
    let r = s.json_in(&ws, &["publish", "public", "--push", "--yes"]);
    assert_eq!(r["suggested_bump"], "major", "{r}");
    let target = checkout(&s, &remote);
    assert!(!target.join("skills/hello").exists());
    assert_eq!(read(&target.join("README.md")), "hand-written readme\n");
}

#[test]
fn licence_gate_blocks_proprietary_vendored_skill_unless_overridden() {
    let (s, _up, ws) = setup();
    s.ok_in(&ws, &["vendor", "acme/skills//secret-sauce", "--yes"]);
    publish_remote(&s, "acme", "pub", "r\n");
    let m = read(&ws.join("tricks.toml"));
    std::fs::write(ws.join("tricks.toml"), format!("{m}\n[publish.targets.public]\nrepo = \"acme/pub\"\nmarketplace = \"acme-pub\"\n"))
        .unwrap();
    commit_all(&ws, "setup");
    let r = s.json_any_in(&ws, &["publish", "public", "--dry-run"]);
    assert_eq!(r["blocked"], true);
    let lic = r["gates"].as_array().unwrap().iter().find(|g| g["name"] == "licence").unwrap().clone();
    assert_eq!(lic["status"], "fail", "{lic}");
    assert!(lic["details"].to_string().contains("license-override"), "the gate says how to override: {lic}");
    let m = read(&ws.join("tricks.toml")).replace(
        "[skills.secret-sauce]\n",
        "[skills.secret-sauce]\nlicense-override = { justification = \"We hold a separate redistribution agreement with Acme.\" }\n",
    );
    std::fs::write(ws.join("tricks.toml"), m).unwrap();
    commit_all(&ws, "override");
    let r = s.json_in(&ws, &["publish", "public", "--dry-run"]);
    let lic = r["gates"].as_array().unwrap().iter().find(|g| g["name"] == "licence").unwrap().clone();
    assert_eq!(lic["status"], "warn", "{lic}");
}

#[test]
fn contribute_dry_run_contains_only_the_customization() {
    let (s, up, ws) = setup();
    s.ok_in(&ws, &["vendor", "acme/skills//hello"]);
    let p = ws.join("skills/hello/SKILL.md");
    std::fs::write(&p, read(&p).replace("2. Wave.", "2. Wave.\n3. Smile.")).unwrap();
    commit_all(&ws, "add a step");
    // Upstream moved on meanwhile (non-overlapping).
    let up_md = up.join("skills/hello/SKILL.md");
    std::fs::write(&up_md, read(&up_md).replace("Intro paragraph.", "Intro paragraph, revised.")).unwrap();
    commit_all(&up, "upstream edit");
    let r = s.json_in(&ws, &["contribute", "hello", "--dry-run"]);
    assert_eq!(r["files"].as_array().unwrap().len(), 1, "{r}");
    let wt = PathBuf::from(r["worktree"].as_str().unwrap());
    let content = read(&wt.join("skills/hello/SKILL.md"));
    assert!(content.contains("3. Smile.") && content.contains("revised"), "{content}");
    let diff = git(&wt, &["show", "--stat", "HEAD"]);
    assert!(diff.contains("skills/hello/SKILL.md") && !diff.contains("tricks"), "{diff}");
}

#[test]
fn update_follows_an_upstream_rename() {
    let (s, up, ws) = setup();
    s.ok_in(&ws, &["vendor", "acme/skills//hello@main"]);
    commit_all(&ws, "vendor hello");
    // Upstream reorganizes: skills/hello → skills/greetings/hello, and edits it.
    std::fs::create_dir_all(up.join("skills/greetings")).unwrap();
    git(&up, &["mv", "skills/hello", "skills/greetings/hello"]);
    let md = up.join("skills/greetings/hello/SKILL.md");
    std::fs::write(&md, read(&md).replace("Keep it short.", "Keep it short. Moved.")).unwrap();
    commit_all(&up, "reorganize");
    set_interval_zero(&s);
    let r = s.json_in(&ws, &["update"]);
    assert_eq!(r["items"][0]["state"], "merged", "{r}");
    assert!(read(&ws.join("skills/hello/SKILL.md")).contains("Moved."));
    assert!(read(&ws.join("tricks.lock")).contains("upstream_path = \"skills/greetings/hello\""), "{}", read(&ws.join("tricks.lock")));
    commit_all(&ws, "merge");
    // The next check follows the new path.
    let out = s.json_in(&ws, &["outdated"]);
    assert_eq!(out["items"][0]["state"], "up-to-date", "{out}");
    let d = s.json_in(&ws, &["diff", "hello", "base.."]);
    assert!(d.as_array().unwrap().is_empty(), "{d}");
}

fn greeter(s: &Sandbox, ws: &Path, name: &str) {
    s.ok_in(ws, &["create", name, "--description", "Writes greetings for any occasion. Use when the user asks for a greeting card text."]);
}

fn edit(p: &Path, from: &str, to: &str) {
    std::fs::write(p, read(p).replace(from, to)).unwrap();
}

#[test]
fn experiments_and_what_links_deploy() {
    let (s, _up, ws) = setup();
    greeter(&s, &ws, "greeter");
    commit_all(&ws, "new greeter");
    let err = s.fail_in(&ws, &["experiment", "commit", "greeter@terse", "-m", "x"]);
    assert!(err.contains("not checked out"), "{err}");
    let err = s.fail_in(&ws, &["experiment", "start", "greeter"]);
    assert!(err.contains("<skill>@<name>"), "{err}");
    // An experiment is branch experiment/<skill>/<name>, checked out inside the repo.
    let e = s.json_in(&ws, &["experiment", "start", "greeter@terse"]);
    assert_eq!(e["branch"], "experiment/greeter/terse", "{e}");
    let path = PathBuf::from(e["path"].as_str().unwrap());
    let worktree = PathBuf::from(e["worktree"].as_str().unwrap());
    assert!(path.starts_with(ws.join(".tricks/work/experiment--greeter--terse")), "{e}");
    assert!(git(&ws, &["status", "--porcelain"]).is_empty(), "worktrees are ignored (init added /.tricks/)");
    // `cd "$(tricks experiment start greeter@terse)"`: stdout is just the path; commands in it act on the repo.
    let o = s.cmd(&ws, &["experiment", "start", "greeter@terse"]);
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), path.to_str().unwrap());
    let inside = s.json_in(&path, &["list"]);
    assert_eq!(inside["source_repo"]["root"], ws.to_str().unwrap(), "{inside}");
    // `experiment shell` runs $SHELL there.
    let fake = s.root().join("fake-shell.sh");
    let out = s.root().join("shell-out.txt");
    std::fs::write(&fake, format!("#!/bin/sh\npwd > {}\necho \"$TRICKS_EXPERIMENT\" >> {}\n", out.display(), out.display())).unwrap();
    std::fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut s = s;
    s.env.push(("SHELL".into(), fake.to_string_lossy().into()));
    s.ok_in(&ws, &["experiment", "shell", "greeter@terse"]);
    let recorded = read(&out);
    assert!(
        recorded.contains(".tricks/work/experiment--greeter--terse/skills/greeter") && recorded.contains("greeter@terse"),
        "{recorded}"
    );

    // A link without a ref deploys the main checkout, live.
    s.ok_in(&ws, &["link", "--agents", "claude"]);
    let deployed = s.home.join(".claude/skills/greeter");
    assert_eq!(std::fs::read_link(&deployed).unwrap(), ws.join("skills/greeter"));
    // `<skill>@<name>` is the experiment: its worktree, live (uncommitted edits too).
    let app = s.project("app");
    let l = s.json_in(&ws, &["link", "greeter@terse", "--to", app.to_str().unwrap(), "--agents", "claude"]);
    assert_eq!(l["links"][0]["branch"], "experiment/greeter/terse", "{l}");
    let pinned = app.join(".claude/skills/greeter");
    assert_eq!(std::fs::read_link(&pinned).unwrap(), path);
    edit(&path.join("SKILL.md"), "## Instructions", "## Instructions (terse)");
    assert!(read(&pinned.join("SKILL.md")).contains("(terse)"), "live, uncommitted");
    assert!(!read(&deployed.join("SKILL.md")).contains("(terse)"));
    // Inside the experiment, `diff`'s `working` and `head` are the experiment's.
    let d = s.ok_in(&path, &["diff", "greeter"]);
    assert!(d.contains("+## Instructions (terse)"), "{d}");
    assert!(s.ok_in(&ws, &["diff", "greeter"]).contains("no differences"), "the main checkout is unchanged");
    let links = s.json_in(&ws, &["list", "--links"]);
    let by_scope =
        |links: &serde_json::Value, scope: &str| links["links"].as_array().unwrap().iter().find(|x| x["scope"] == scope).cloned().unwrap();
    let g = by_scope(&links, "global");
    assert_eq!(
        (g["branch"].as_str(), g["source"].as_str(), g["pinned"].as_bool()),
        (Some("main"), Some("working-tree"), Some(false)),
        "{g}"
    );
    let app_key = app.canonicalize().unwrap().to_str().unwrap().to_string();
    let a = by_scope(&links, &app_key);
    assert_eq!(
        (a["branch"].as_str(), a["source"].as_str(), a["pinned"].as_bool()),
        (Some("experiment/greeter/terse"), Some("worktree"), Some(true)),
        "{a}"
    );
    let human = s.ok_in(&ws, &["list", "--links"]);
    assert!(human.contains("main (working tree, live)") && human.contains("experiment/greeter/terse (worktree, live, pinned)"), "{human}");
    let status = s.ok_in(&ws, &["list"]);
    assert!(status.contains("user scope (main)") && status.contains("experiments: terse"), "{status}");

    // Commit in the experiment, by name or from inside its worktree.
    let c = s.json_in(&ws, &["experiment", "commit", "greeter@terse", "-m", "terse variant"]);
    assert_eq!(c["branch"], "experiment/greeter/terse", "{c}");
    assert!(git(&worktree, &["status", "--porcelain"]).is_empty());
    edit(&path.join("SKILL.md"), "(terse)", "(terser)");
    let c = s.json_in(&path, &["experiment", "commit", "-m", "terser"]);
    assert!(c["commit"].is_string(), "{c}");
    let l = s.json_in(&ws, &["experiment", "list"]);
    assert_eq!((l[0]["ahead"].as_u64(), l[0]["links"].as_array().unwrap().len()), (Some(2), 1), "{l}");
    let d = s.json_in(&ws, &["diff", "greeter", "head..terse"]).to_string();
    assert!(d.contains("(terser)"), "diff takes experiment names: {d}");

    // Unpinned links follow the main checkout to whatever branch it is on.
    git(&ws, &["switch", "-q", "-c", "feature"]);
    let links = s.json_in(&ws, &["list", "--links"]);
    assert_eq!(by_scope(&links, "global")["branch"], "feature", "{links}");
    git(&ws, &["switch", "-q", "main"]);

    // A plain branch gets a worktree of its own while a link needs it.
    git(&ws, &["branch", "plain"]);
    let app2 = s.project("app2");
    s.ok_in(&ws, &["link", "greeter@plain", "--to", app2.to_str().unwrap(), "--agents", "claude"]);
    let plain_wt = ws.join(".tricks/work/plain");
    assert_eq!(std::fs::read_link(app2.join(".claude/skills/greeter")).unwrap(), plain_wt.join("skills/greeter"));
    s.ok_in(&ws, &["unlink", "greeter", "--to", app2.to_str().unwrap()]);
    assert!(!plain_wt.exists(), "the link's worktree goes with the last link");
    assert!(git(&ws, &["branch", "--list", "plain"]).contains("plain"), "the branch stays");

    // A tag or commit is a frozen snapshot in the store.
    git(&ws, &["tag", "v1"]);
    let app3 = s.project("app3");
    s.ok_in(&ws, &["link", "greeter@v1", "--to", app3.to_str().unwrap(), "--agents", "claude"]);
    let t = std::fs::read_link(app3.join(".claude/skills/greeter")).unwrap();
    assert!(t.starts_with(&s.data), "snapshot in the store: {}", t.display());
    let tip = git(&ws, &["rev-parse", "--short=7", "v1"]);
    assert!(s.ok_in(&ws, &["list", "--links"]).contains(&format!("v1 @ {tip} (snapshot, pinned)")));
    let err = s.fail_in(&ws, &["link", "greeter@nope", "--to", app3.to_str().unwrap()]);
    assert!(err.contains("no experiment `greeter@nope`"), "{err}");

    // Copies do not follow edits: they say `copy`, not `live`.
    let app4 = s.project("app4");
    let out = s.ok_in(&ws, &["link", "greeter", "--to", app4.to_str().unwrap(), "--agents", "claude", "--copy"]);
    assert!(out.contains("from main (working tree, copy)"), "{out}");
    assert!(s.ok_in(&ws, &["list", "--links"]).contains("main (working tree, copy)"));

    // `link greeter` (no ref) un-pins it.
    s.ok_in(&ws, &["link", "greeter", "--to", app.to_str().unwrap(), "--agents", "claude"]);
    assert_eq!(std::fs::read_link(&pinned).unwrap(), ws.join("skills/greeter"));
}

#[test]
fn experiment_merge_takes_the_whole_branch_and_cleans_up() {
    let (mut s, _up, ws) = setup();
    for n in ["greeter", "farewell"] {
        greeter(&s, &ws, n);
    }
    commit_all(&ws, "two skills");
    s.ok_in(&ws, &["link", "--agents", "claude"]);
    // The git commands that change the repo are echoed.
    let o = s.cmd(&ws, &["experiment", "start", "greeter@terse"]);
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("$ git worktree add -q -b experiment/greeter/terse"), "{err}");
    let wt = ws.join(".tricks/work/experiment--greeter--terse");
    // An experiment of greeter that needs a change to farewell too.
    edit(&wt.join("skills/greeter/SKILL.md"), "## Instructions", "## Instructions (terse)");
    edit(&wt.join("skills/farewell/SKILL.md"), "## Instructions", "## Instructions (changed too)");
    let err = s.fail_in(&ws, &["experiment", "merge", "greeter@terse"]);
    assert!(err.contains("uncommitted changes") && err.contains("experiment commit"), "{err}");
    s.ok_in(&ws, &["experiment", "commit", "greeter@terse", "-m", "terse greeter"]);
    // Meanwhile main moves on elsewhere in greeter.
    let mg = ws.join("skills/greeter/SKILL.md");
    edit(&mg, "## Examples", "## Examples (main)");
    commit_all(&ws, "main edit");
    let app = s.project("app");
    s.ok_in(&ws, &["link", "greeter@terse", "--to", app.to_str().unwrap(), "--agents", "claude"]);
    let pinned = app.join(".claude/skills/greeter");
    assert_eq!(std::fs::read_link(&pinned).unwrap(), wt.join("skills/greeter"));

    let r = s.json_in(&ws, &["experiment", "merge", "greeter@terse"]);
    assert!(r["commit"].is_string() && r["cleaned_up"] == true, "{r}");
    let merged = read(&mg);
    assert!(merged.contains("(terse)") && merged.contains("(main)"), "{merged}");
    assert!(read(&ws.join("skills/farewell/SKILL.md")).contains("changed too"), "the whole experiment is merged");
    assert!(git(&ws, &["status", "--porcelain"]).is_empty(), "committed");
    assert_eq!(git(&ws, &["log", "-1", "--format=%s"]), "Merge experiment greeter@terse");
    assert!(git(&ws, &["branch", "--list", "experiment/*"]).is_empty(), "branch removed");
    assert!(!wt.exists(), "worktree removed");
    assert_eq!(std::fs::read_link(&pinned).unwrap(), ws.join("skills/greeter"), "pinned links follow the main checkout");
    let links = s.json_in(&ws, &["list", "--links"]);
    assert!(links["links"].as_array().unwrap().iter().all(|l| l["pinned"] == false), "{links}");

    // Discarding loses commits, so it asks.
    s.ok_in(&ws, &["experiment", "start", "greeter@verbose"]);
    let vwt = ws.join(".tricks/work/experiment--greeter--verbose");
    edit(&vwt.join("skills/greeter/SKILL.md"), "## Instructions", "## Instructions (verbose)");
    s.ok_in(&ws, &["experiment", "commit", "greeter@verbose", "-m", "verbose"]);
    let err = s.fail_in(&ws, &["experiment", "discard", "greeter@verbose"]);
    assert!(err.contains("confirmation required: Discard experiment greeter@verbose?") && err.contains("1 commit(s)"), "{err}");
    s.ok_in(&ws, &["experiment", "discard", "greeter@verbose", "--yes"]);
    assert!(!vwt.exists() && git(&ws, &["branch", "--list", "experiment/*"]).is_empty());
    // An experiment with nothing in it goes without asking.
    s.ok_in(&ws, &["experiment", "start", "greeter@empty"]);
    s.ok_in(&ws, &["experiment", "discard", "greeter@empty"]);

    // --pr pushes the experiment and opens a pull request; the worktree stays for fixes.
    let remote = s.fixtures.join("acme/my-skills.git");
    std::fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    git(&ws, &["remote", "add", "origin", remote.to_str().unwrap()]);
    let bin = s.root().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("gh"), "#!/bin/sh\necho https://github.com/acme/my-skills/pull/7\n").unwrap();
    std::fs::set_permissions(bin.join("gh"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    s.env.push(("PATH".into(), format!("{}:{}", bin.display(), std::env::var("PATH").unwrap())));
    s.ok_in(&ws, &["experiment", "start", "greeter@review"]);
    let rwt = ws.join(".tricks/work/experiment--greeter--review");
    edit(&rwt.join("skills/greeter/SKILL.md"), "## Instructions", "## Instructions (review)");
    s.ok_in(&ws, &["experiment", "commit", "greeter@review", "-m", "review me"]);
    let r = s.json_in(&ws, &["experiment", "merge", "greeter@review", "--pr"]);
    assert_eq!(r["pr_url"], "https://github.com/acme/my-skills/pull/7", "{r}");
    assert!(rwt.exists(), "worktree kept for review fixes");
    assert!(!git(&remote, &["branch", "--list", "experiment/greeter/review"]).is_empty(), "pushed");
    let l = s.json_in(&ws, &["experiment", "list", "greeter"]);
    assert_eq!(l[0]["pr"], "https://github.com/acme/my-skills/pull/7", "{l}");
    // Again: pushes the fixes to the same pull request.
    edit(&rwt.join("skills/greeter/SKILL.md"), "(review)", "(reviewed)");
    s.ok_in(&ws, &["experiment", "commit", "greeter@review", "-m", "fix"]);
    let o = s.cmd(&ws, &["experiment", "merge", "greeter@review", "--pr"]);
    assert!(String::from_utf8_lossy(&o.stderr).contains("pushed new commits to the open pull request"), "{o:?}");
    assert_eq!(git(&remote, &["rev-parse", "experiment/greeter/review"]), git(&rwt, &["rev-parse", "HEAD"]));
}
