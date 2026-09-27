//! Discovery: `info` and `view`, and the catalogs written into a new user config.

mod common;
use common::*;

#[test]
fn info_shows_metadata_and_view_shows_content() {
    let s = Sandbox::new();
    s.upstream(
        "acme",
        "skills",
        &[
            ("skills/hello/SKILL.md", &skill_md("hello", "Say hello. Use when greeting.", "# Hello\n\nBe **kind**.\n")),
            ("skills/hello/references/tone.md", "# Tone\n\nWarm.\n"),
        ],
    );
    let info = s.json(&["info", "acme/skills//hello"]);
    assert_eq!(info["name"], "hello");
    assert!(info["frontmatter"].as_str().unwrap().contains("description: Say hello."), "{info}");
    assert!(info.get("body").is_none(), "info carries no body: {info}");
    assert!(info["files"].to_string().contains("references/tone.md"));
    let human = s.ok(&["info", "acme/skills//hello"]);
    assert!(human.contains("frontmatter:") && human.contains("tricks view"), "{human}");
    assert!(!human.contains("Be **kind**"), "{human}");

    // Not a terminal: the file is printed as is (agents and pipes get plain Markdown).
    // (Line endings normalized: git may check files out with CRLF on Windows.)
    let lf = |t: String| t.replace("\r\n", "\n");
    let raw = lf(s.ok(&["view", "acme/skills//hello"]));
    assert!(raw.starts_with("---\nname: hello") && raw.contains("Be **kind**."), "{raw}");
    let tone = lf(s.ok(&["view", "acme/skills//hello", "references/tone.md"]));
    assert_eq!(tone, "# Tone\n\nWarm.\n");
    let j = s.json(&["view", "acme/skills//hello", "references/tone.md"]);
    assert_eq!(j["path"], "references/tone.md");
    assert_eq!(lf(j["content"].as_str().unwrap().to_string()), "# Tone\n\nWarm.\n");
}

#[test]
fn a_new_user_config_lists_the_recommended_catalogs() {
    let s = Sandbox::new();
    let cfg = s.config.join("tricks.toml");
    std::fs::remove_file(&cfg).unwrap();
    let l = s.json(&["--offline", "catalog", "list"]);
    let keys: Vec<&str> = l.as_array().unwrap().iter().map(|c| c["key"].as_str().unwrap()).collect();
    assert!(keys.contains(&"github.com/anthropics/skills") && keys.len() >= 5, "{l}");
    let text = read(&cfg);
    assert!(text.contains("[catalogs]") && text.contains("\"github.com/anthropics/skills\" = {}") && text.contains("[settings]"), "{text}");
    // Removing one really removes it; --recommended brings it back.
    s.ok(&["--offline", "catalog", "remove", "github.com/obra/superpowers"]);
    assert!(!read(&cfg).contains("obra/superpowers"));
    let r = s.json(&["--offline", "catalog", "add", "--recommended"]);
    assert_eq!(r["added"], serde_json::json!(["github.com/obra/superpowers"]), "{r}");
    assert!(read(&cfg).contains("obra/superpowers"));
    // A config that exists is never rewritten with defaults.
    let s2 = Sandbox::new();
    s2.ok(&["--offline", "catalog", "list"]);
    assert!(!read(&s2.config.join("tricks.toml")).contains("anthropics"));
}

#[test]
fn search_sorts_and_reports_facets() {
    let s = Sandbox::new();
    s.upstream(
        "acme",
        "skills",
        &[
            ("skills/zeta-notes/SKILL.md", &skill_md("zeta-notes", "Take meeting notes. Use when summarizing a meeting.", "body\n")),
            ("skills/alpha-notes/SKILL.md", &skill_md("alpha-notes", "Write release notes. Use when tagging a release.", "body\n")),
        ],
    );
    s.upstream(
        "other",
        "kit",
        &[("skills/beta-notes/SKILL.md", &skill_md("beta-notes", "Notes for talks. Use when preparing a talk.", "body\n"))],
    );
    s.ok(&["catalog", "add", "acme/skills"]);
    s.ok(&["catalog", "add", "other/kit"]);
    let r = s.json(&["search", "--no-live", "--sort", "name", "notes"]);
    let names: Vec<&str> = r.as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["alpha-notes", "beta-notes", "zeta-notes"], "{r}");
    let f = s.json(&["search", "--no-live", "--facets", "notes"]);
    assert_eq!(f["total"], 3, "{f}");
    assert_eq!(f["facets"]["owners"], serde_json::json!([["acme", 2], ["other", 1]]), "{f}");
    let human = s.ok(&["search", "--no-live", "--facets", "notes"]);
    assert!(human.contains("3 matching skill(s)") && human.contains("owners:"), "{human}");
    let r = s.json(&["search", "--no-live", "--min-installs", "1", "notes"]);
    assert!(r.as_array().unwrap().is_empty(), "no installs recorded: {r}");
    let err = s.fail_in(&s.root(), &["search", "--sort", "popular", "notes"]);
    assert!(err.contains("unknown sort `popular`"), "{err}");
}

#[test]
#[cfg(unix)]
fn first_run_detects_agents_and_links_once_per_agent() {
    let mut s = Sandbox::new();
    std::fs::remove_file(s.config.join("tricks.toml")).unwrap();
    for d in [".codex", ".cursor"] {
        std::fs::create_dir_all(s.home.join(d)).unwrap();
    }
    // Keep agents installed on this machine out of it.
    s.env.push(("PATH".into(), "/usr/bin:/bin".into()));
    s.ok(&["--offline", "catalog", "list"]);
    let cfg = read(&s.config.join("tricks.toml"));
    assert!(cfg.contains(r#"agents = ["codex", "cursor"]"#), "{cfg}");
    // Cursor loads ~/.agents/skills, where Codex's links go, so it is not linked twice;
    // except on Linux here, where the link would point into a hidden directory (the test's
    // temporary one), which Cursor skips, so it gets a copy of its own.
    let ws = s.project("my-skills");
    git(&ws, &["init", "-q", "-b", "main"]);
    s.ok_in(&ws, &["init"]);
    s.ok_in(&ws, &["create", "greeter", "--description", "Greets people. Use when the user asks for a greeting."]);
    let o = s.cmd(&ws, &["link"]);
    assert!(s.home.join(".agents/skills/greeter").exists());
    let hidden = ws.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.'));
    if cfg!(target_os = "linux") && hidden {
        assert!(s.home.join(".cursor/skills/greeter/SKILL.md").is_file(), "{o:?}");
        return;
    }
    assert!(String::from_utf8_lossy(&o.stderr).contains("cursor loads user-scope skills from ~/.agents/skills"), "{o:?}");
    assert!(!s.home.join(".cursor/skills/greeter").exists());
    // Named agents are always linked.
    s.ok_in(&ws, &["link", "--agents", "cursor"]);
    assert!(s.home.join(".cursor/skills/greeter").exists());
}

#[test]
#[cfg(unix)]
fn create_on_a_branch() {
    let s = Sandbox::new();
    let ws = s.project("my-skills");
    git(&ws, &["init", "-q", "-b", "main"]);
    s.ok_in(&ws, &["init"]);
    commit_all(&ws, "init");
    let o =
        s.cmd(&ws, &["create", "greeter", "-b", "add-greeter", "--description", "Greets people. Use when the user asks for a greeting."]);
    assert!(o.status.success(), "{o:?}");
    assert!(String::from_utf8_lossy(&o.stderr).contains("$ git switch -q -c add-greeter"), "{o:?}");
    assert_eq!(git(&ws, &["branch", "--show-current"]), "add-greeter");
    assert!(read(&ws.join("tricks.toml")).contains("[skills.greeter]"));
}
