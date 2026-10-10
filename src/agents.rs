//! Agent integrations (spec §8): primary directories and link capability per platform.

use anyhow::{Result, bail};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Agent {
    pub id: &'static str,
    pub display: &'static str,
    /// Primary user-scope directory, relative to home.
    pub user_dir: &'static str,
    /// Primary project-scope directory, relative to the project root.
    pub project_dir: &'static str,
    /// Other user-scope directories the agent loads skills from, relative to home.
    pub also_reads: &'static [&'static str],
    /// Signs that the agent is installed: directories under home, and programs on PATH.
    pub detect_dirs: &'static [&'static str],
    pub detect_bins: &'static [&'static str],
    /// Agent versions this integration was verified against (September 2026).
    pub tested: &'static str,
}

pub const AGENTS: [Agent; 4] = [
    Agent {
        id: "claude",
        display: "Claude Code",
        user_dir: ".claude/skills",
        project_dir: ".claude/skills",
        also_reads: &[],
        detect_dirs: &[".claude"],
        detect_bins: &["claude"],
        tested: "2.1.280",
    },
    Agent {
        id: "codex",
        display: "Codex",
        user_dir: ".agents/skills",
        project_dir: ".agents/skills",
        also_reads: &[".codex/skills"],
        detect_dirs: &[".codex"],
        detect_bins: &["codex"],
        tested: "0.140.0",
    },
    Agent {
        id: "cursor",
        display: "Cursor",
        user_dir: ".cursor/skills",
        project_dir: ".cursor/skills",
        also_reads: &[".agents/skills", ".claude/skills", ".codex/skills"],
        detect_dirs: &[".cursor"],
        detect_bins: &["cursor", "cursor-agent"],
        tested: "3.11.19",
    },
    Agent {
        id: "copilot",
        display: "GitHub Copilot",
        user_dir: ".copilot/skills",
        project_dir: ".github/skills",
        also_reads: &[".claude/skills", ".agents/skills"],
        detect_dirs: &[".copilot"],
        detect_bins: &["copilot"],
        tested: "VS Code 1.128.1",
    },
];

/// Agents installed on this machine (for the first-run user config), in table order.
pub fn detect(home: &Path) -> Vec<&'static Agent> {
    let path: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    let on_path = |bin: &str| path.iter().any(|d| d.join(bin).is_file() || (cfg!(windows) && d.join(format!("{bin}.exe")).is_file()));
    let vscode_copilot = |a: &Agent| {
        a.id == "copilot"
            && std::fs::read_dir(home.join(".vscode/extensions"))
                .map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().starts_with("github.copilot")))
                .unwrap_or(false)
    };
    AGENTS
        .iter()
        .filter(|a| a.detect_dirs.iter().any(|d| home.join(d).is_dir()) || a.detect_bins.iter().any(|b| on_path(b)) || vscode_copilot(a))
        .collect()
}

/// The fewest agents whose user-scope directories reach every agent in `selected`, so
/// that no agent loads the same skill twice. An agent is reached through another's
/// directory only if it reads that directory and can load what gets placed there (a
/// link it follows, or a copy).
pub fn cover_user_scope(selected: &[&'static Agent], target: &Path, force_copy: bool) -> Vec<&'static Agent> {
    let reaches = |by: &Agent, x: &Agent| {
        if by.id == x.id {
            return true;
        }
        let copy = force_copy || !by.follows_links(target);
        x.also_reads.contains(&by.user_dir) && (copy || x.follows_links(target))
    };
    let mut uncovered: Vec<&'static Agent> = selected.to_vec();
    let mut chosen: Vec<&'static Agent> = Vec::new();
    while !uncovered.is_empty() {
        let best = selected
            .iter()
            .max_by_key(|a| {
                (uncovered.iter().filter(|x| reaches(a, x)).count(), std::cmp::Reverse(selected.iter().position(|s| s.id == a.id)))
            })
            .copied()
            .unwrap();
        uncovered.retain(|x| !reaches(best, x));
        chosen.push(best);
    }
    // Keep the configured order.
    selected.iter().filter(|a| chosen.iter().any(|c| c.id == a.id)).copied().collect()
}

pub fn get(id: &str) -> Result<&'static Agent> {
    let id = match id {
        "claude-code" | "claude_code" => "claude",
        "github-copilot" | "copilot-cli" | "vscode" => "copilot",
        "cursor-agent" => "cursor",
        "openai-codex" => "codex",
        other => other,
    };
    match AGENTS.iter().find(|a| a.id == id) {
        Some(a) => Ok(a),
        None => bail!("unknown agent `{id}` (claude | codex | cursor | copilot)"),
    }
}

pub fn parse_list(s: &[String]) -> Result<Vec<&'static Agent>> {
    let mut out: Vec<&'static Agent> = Vec::new();
    for item in s.iter().flat_map(|x| x.split(',')).map(str::trim).filter(|x| !x.is_empty()) {
        if item == "all" {
            return Ok(AGENTS.iter().collect());
        }
        let a = get(item)?;
        if !out.iter().any(|x| x.id == a.id) {
            out.push(a);
        }
    }
    Ok(out)
}

impl Agent {
    pub fn user_path(&self, home: &Path) -> PathBuf {
        home.join(self.user_dir)
    }

    pub fn project_path(&self, root: &Path) -> PathBuf {
        root.join(self.project_dir)
    }

    /// Whether this agent reliably follows directory links to `target` on this
    /// platform (spec §8 link-capability table). Overridable for testing.
    pub fn follows_links(&self, target: &Path) -> bool {
        if let Ok(v) = std::env::var("TRICKS_LINK_MODE") {
            // e.g. "copy", "link", or "copilot=link,claude=copy"
            if v == "copy" {
                return false;
            }
            if v == "link" {
                return true;
            }
            for pair in v.split(',') {
                if let Some((a, m)) = pair.split_once('=')
                    && a.trim() == self.id
                {
                    return m.trim() == "link";
                }
            }
        }
        if cfg!(windows) {
            return false; // junction bugs (anthropics/claude-code#41177 and others)
        }
        match self.id {
            // microsoft/vscode#315979: symlinked skills listed but `skill()` fails.
            "copilot" => false,
            // Cursor skips hidden dot-directories; avoid links into one on Linux.
            "cursor" => !(cfg!(target_os = "linux") && has_hidden_component(target)),
            _ => true,
        }
    }
}

fn has_hidden_component(p: &Path) -> bool {
    p.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s.starts_with('.') && s != "." && s != ".."
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&Agent]) -> Vec<&'static str> {
        v.iter().map(|a| a.id).collect()
    }

    #[test]
    #[cfg(not(windows))]
    fn user_scope_cover_skips_agents_that_read_other_directories() {
        let all: Vec<&Agent> = AGENTS.iter().collect();
        let target = Path::new("/data/store/abc");
        // Cursor reads ~/.claude/skills and ~/.agents/skills; Copilot reads them too but
        // cannot load linked skills there, so it keeps its own (copied) directory.
        assert_eq!(ids(&cover_user_scope(&all, target, false)), vec!["claude", "codex", "copilot"]);
        // With copies everywhere, Copilot is reached through ~/.claude/skills as well.
        assert_eq!(ids(&cover_user_scope(&all, target, true)), vec!["claude", "codex"]);
        let two: Vec<&Agent> = ["claude", "cursor"].iter().map(|i| get(i).unwrap()).collect();
        assert_eq!(ids(&cover_user_scope(&two, target, false)), vec!["claude"]);
        let one: Vec<&Agent> = vec![get("cursor").unwrap()];
        assert_eq!(ids(&cover_user_scope(&one, target, false)), vec!["cursor"]);
    }
}
