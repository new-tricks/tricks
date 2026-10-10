//! Thin wrapper over the system `git` (spec §4: system git for writes, inheriting the
//! user's credential helpers) plus fetch-only upstream mirrors.

use crate::id::SourceId;
use crate::paths::Paths;
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU8, Ordering};

/// Which commands are echoed to stderr as `$ git …`: 0 none, 1 the ones that change a
/// repository (`run`), 2 every command (`--verbose`).
static ECHO: AtomicU8 = AtomicU8::new(0);

pub fn set_echo(level: u8) {
    ECHO.store(level, Ordering::Relaxed);
}

/// Print a command the way you could run it yourself: `$ git -C <dir> <args>` (no `-C`
/// when `dir` is the current directory). Paths under the current directory are shown
/// relative to it, others under home with `~`.
pub fn echo(dir: &Path, program: &str, args: &[&str], min: u8) {
    if ECHO.load(Ordering::Relaxed) < min {
        return;
    }
    let here = std::env::current_dir().ok().and_then(|d| crate::paths::canon(&d).ok());
    let there = crate::paths::canon(dir).unwrap_or(dir.to_path_buf());
    let show = |p: &Path| -> String {
        if let Some(rel) = here.as_deref().and_then(|h| p.strip_prefix(h).ok()) {
            return if rel.as_os_str().is_empty() { ".".into() } else { rel.display().to_string() };
        }
        match std::env::var_os("HOME").map(PathBuf::from) {
            Some(h) if p.starts_with(&h) => format!("~/{}", p.strip_prefix(&h).unwrap().display()),
            _ => p.display().to_string(),
        }
    };
    let mut line = format!("$ {program}");
    let elsewhere = here.as_deref() != Some(there.as_path());
    if elsewhere && program == "git" {
        line.push_str(" -C ");
        line.push_str(&shell_quote(&show(&there)));
    }
    for a in args {
        let p = Path::new(a);
        let a = if p.is_absolute() { show(&crate::paths::canon(p).unwrap_or(p.to_path_buf())) } else { a.to_string() };
        line.push(' ');
        line.push_str(&shell_quote(&a));
    }
    if elsewhere && program != "git" {
        line.push_str(&format!("   (in {})", show(&there)));
    }
    eprintln!("{line}");
}

fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,~^{}".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// A git command that changes a repository you work with (commit, branch, worktree,
/// merge, push): echoed by default so you can see, repeat or undo what New Tricks did.
pub fn run(dir: &Path, args: &[&str]) -> Result<String> {
    echo(dir, "git", args, 1);
    let out = exec(dir, args)?;
    Ok(String::from_utf8_lossy(&out).trim_end().to_string())
}

/// A read-only or internal git command (echoed only with `--verbose`).
pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = git_raw(dir, args)?;
    Ok(String::from_utf8_lossy(&out).trim_end().to_string())
}

pub fn git_raw(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    echo(dir, "git", args, 2);
    exec(dir, args)
}

fn exec(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let o = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !o.status.success() {
        bail!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(o.stdout)
}

pub fn git_ok(dir: &Path, args: &[&str]) -> bool {
    echo(dir, "git", args, 2);
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn git_with_input(dir: &Path, args: &[&str], input: &[u8]) -> Result<String> {
    echo(dir, "git", args, 2);
    let mut child =
        Command::new("git").args(args).current_dir(dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(input)?;
    let o = child.wait_with_output()?;
    if !o.status.success() {
        bail!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim_end().to_string())
}

/// Test/enterprise hook: `TRICKS_HOST_MAP="github.com=/fixtures;git.corp=/srv"` maps a
/// host to a local directory of repositories (`<dir>/<owner>/<repo>`).
pub fn host_map(host: &str) -> Option<PathBuf> {
    let m = std::env::var("TRICKS_HOST_MAP").ok()?;
    m.split(';').find_map(|pair| {
        let (h, p) = pair.split_once('=')?;
        (h.trim() == host).then(|| PathBuf::from(p.trim()))
    })
}

pub fn clone_url(src: &SourceId) -> String {
    if let Some(base) = host_map(&src.host) {
        return base.join(&src.repo_path).to_string_lossy().to_string();
    }
    format!("https://{}/{}.git", src.host, src.repo_path)
}

#[derive(Debug, Clone, Default)]
pub struct RemoteRefs {
    pub default_branch: Option<String>,
    pub heads: BTreeMap<String, String>,
    /// Tag name -> commit (peeled).
    pub tags: BTreeMap<String, String>,
}

impl RemoteRefs {
    pub fn all_names(&self) -> Vec<String> {
        self.heads.keys().chain(self.tags.keys()).cloned().collect()
    }
}

/// `git ls-remote --symref` against any host, using the user's credentials.
pub fn ls_remote(url: &str) -> Result<RemoteRefs> {
    let tmp = std::env::temp_dir();
    let out = git(&tmp, &["ls-remote", "--symref", url, "HEAD", "refs/heads/*", "refs/tags/*"])
        .with_context(|| format!("listing refs of {url}"))?;
    let mut r = RemoteRefs::default();
    for line in out.lines() {
        if let Some(rest) = line.strip_prefix("ref: ") {
            // ref: refs/heads/main\tHEAD
            if let Some((target, name)) = rest.split_once('\t')
                && name == "HEAD"
            {
                r.default_branch = target.strip_prefix("refs/heads/").map(|s| s.to_string());
            }
            continue;
        }
        let Some((sha, name)) = line.split_once('\t') else { continue };
        if let Some(b) = name.strip_prefix("refs/heads/") {
            r.heads.insert(b.to_string(), sha.to_string());
        } else if let Some(t) = name.strip_prefix("refs/tags/") {
            if let Some(t) = t.strip_suffix("^{}") {
                r.tags.insert(t.to_string(), sha.to_string()); // peeled wins
            } else {
                r.tags.entry(t.to_string()).or_insert_with(|| sha.to_string());
            }
        }
    }
    Ok(r)
}

/// A fetch-only bare mirror of an upstream repository. Never pushed.
#[derive(Debug, Clone)]
pub struct Mirror {
    pub source: SourceId,
    pub dir: PathBuf,
}

impl Mirror {
    pub fn path_for(paths: &Paths, src: &SourceId) -> PathBuf {
        paths.repos().join(&src.host).join(format!("{}.git", src.repo_path))
    }

    /// Open the mirror, cloning if needed and fetching when `fetch` is true.
    pub fn open(paths: &Paths, src: &SourceId, fetch: bool) -> Result<Mirror> {
        let dir = Self::path_for(paths, src);
        let url = clone_url(src);
        if !dir.join("HEAD").exists() {
            std::fs::create_dir_all(dir.parent().unwrap())?;
            let tmp = dir.with_extension("tmp-clone");
            let _ = std::fs::remove_dir_all(&tmp);
            let parent = dir.parent().unwrap();
            git(parent, &["clone", "--bare", "--quiet", &url, &tmp.to_string_lossy()]).with_context(|| format!("cloning {src}"))?;
            std::fs::rename(&tmp, &dir)?;
        } else if fetch {
            git(&dir, &["fetch", "--quiet", "--prune", "--force", &url, "+refs/heads/*:refs/heads/*", "+refs/tags/*:refs/tags/*"])
                .with_context(|| format!("fetching {src}"))?;
        }
        Ok(Mirror { source: src.clone(), dir })
    }

    /// Ensure a specific commit is present (fetching by SHA if needed).
    pub fn ensure_commit(&self, commit: &str) -> Result<()> {
        if self.has_commit(commit) {
            return Ok(());
        }
        let url = clone_url(&self.source);
        let _ = git(&self.dir, &["fetch", "--quiet", &url, commit]);
        if !self.has_commit(commit) {
            bail!("commit {commit} not found in {}", self.source);
        }
        Ok(())
    }

    pub fn has_commit(&self, commit: &str) -> bool {
        git_ok(&self.dir, &["cat-file", "-e", &format!("{commit}^{{commit}}")])
    }

    pub fn rev_parse(&self, rev: &str) -> Result<String> {
        git(&self.dir, &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])
            .map_err(|_| anyhow!("cannot resolve `{rev}` in {}", self.source))
    }

    pub fn default_branch(&self) -> Option<String> {
        git(&self.dir, &["symbolic-ref", "--short", "HEAD"]).ok()
    }

    /// Tree SHA of `path` at `commit` (`.` = root). None if absent.
    pub fn tree_at(&self, commit: &str, path: &str) -> Option<String> {
        let spec = if path == "." { format!("{commit}^{{tree}}") } else { format!("{commit}:{path}") };
        let sha = git(&self.dir, &["rev-parse", "--verify", "--quiet", &spec]).ok()?;
        let kind = git(&self.dir, &["cat-file", "-t", &sha]).ok()?;
        (kind == "tree").then_some(sha)
    }

    /// All `(path, tree_sha)` of directories containing a `SKILL.md` at `commit`.
    pub fn skill_dirs(&self, commit: &str) -> Result<Vec<(String, String)>> {
        let out = git(&self.dir, &["ls-tree", "-r", "-t", "--full-tree", commit])?;
        let mut trees: BTreeMap<String, String> = BTreeMap::new();
        let mut skills = Vec::new();
        for line in out.lines() {
            let Some((meta, path)) = line.split_once('\t') else { continue };
            let parts: Vec<&str> = meta.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }
            if parts[1] == "tree" {
                trees.insert(path.to_string(), parts[2].to_string());
            } else if parts[1] == "blob" && path.rsplit('/').next() == Some("SKILL.md") {
                let dir = path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_else(|| ".".to_string());
                skills.push(dir);
            }
        }
        let root = git(&self.dir, &["rev-parse", &format!("{commit}^{{tree}}")])?;
        Ok(skills
            .into_iter()
            .filter_map(|d| {
                let t = if d == "." { Some(root.clone()) } else { trees.get(&d).cloned() };
                t.map(|t| (d, t))
            })
            .collect())
    }

    pub fn read_file(&self, commit: &str, path: &str) -> Result<Vec<u8>> {
        git_raw(&self.dir, &["show", &format!("{commit}:{path}")])
    }

    pub fn file_exists(&self, commit: &str, path: &str) -> bool {
        git_ok(&self.dir, &["cat-file", "-e", &format!("{commit}:{path}")])
    }

    /// List files (relative to `dir_path`) in a directory at `commit`.
    pub fn list_files(&self, commit: &str, dir_path: &str) -> Result<Vec<String>> {
        let spec = if dir_path == "." { commit.to_string() } else { format!("{commit}:{dir_path}") };
        let out = git(&self.dir, &["ls-tree", "-r", "--name-only", &spec])?;
        Ok(out.lines().map(|s| s.to_string()).collect())
    }

    /// Materialize `path` at `commit` into `dest` (which must not exist).
    pub fn export_dir(&self, commit: &str, path: &str, dest: &Path) -> Result<()> {
        let spec = if path == "." { format!("{commit}^{{tree}}") } else { format!("{commit}:{path}") };
        let tarball = git_raw(&self.dir, &["archive", "--format=tar", &spec])?;
        std::fs::create_dir_all(dest)?;
        let mut ar = tar::Archive::new(&tarball[..]);
        ar.set_preserve_permissions(true);
        ar.unpack(dest).with_context(|| format!("unpacking into {}", dest.display()))?;
        Ok(())
    }

    pub fn commit_time(&self, commit: &str) -> Option<i64> {
        git(&self.dir, &["log", "-1", "--format=%ct", commit]).ok()?.parse().ok()
    }

    /// Detect a rename of `path` between two commits (git rename detection).
    pub fn renamed_path(&self, from: &str, to: &str, path: &str) -> Option<String> {
        let out = git(&self.dir, &["diff", "--name-status", "-M", from, to]).ok()?;
        let prefix = format!("{path}/");
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for line in out.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() == 3
                && cols[0].starts_with('R')
                && let Some(rest) = cols[1].strip_prefix(&prefix)
                && let Some(newdir) = cols[2].strip_suffix(rest).map(|s| s.trim_end_matches('/'))
            {
                *counts.entry(newdir.to_string()).or_default() += 1;
            }
        }
        counts.into_iter().max_by_key(|(_, c)| *c).map(|(d, _)| d)
    }
}

/// Top-level of the git repository containing `dir`, if any.
pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    git(dir, &["rev-parse", "--show-toplevel"]).ok().map(PathBuf::from)
}

/// Resolve the exclude file for the repository containing `dir` (works for worktrees
/// and submodules where `.git` is a file).
pub fn exclude_file(dir: &Path) -> Option<PathBuf> {
    let root = repo_root(dir)?;
    let p = git(&root, &["rev-parse", "--git-path", "info/exclude"]).ok()?;
    let p = PathBuf::from(p);
    Some(if p.is_absolute() { p } else { root.join(p) })
}

pub fn is_clean(dir: &Path) -> Result<bool> {
    Ok(git(dir, &["status", "--porcelain"])?.trim().is_empty())
}

pub fn head_commit(dir: &Path) -> Result<String> {
    git(dir, &["rev-parse", "HEAD"])
}

pub fn current_branch(dir: &Path) -> Option<String> {
    git(dir, &["symbolic-ref", "--short", "HEAD"]).ok()
}

pub fn branch_exists(dir: &Path, branch: &str) -> bool {
    git_ok(dir, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
}

/// Checkouts of the repository at `dir`: (path, branch), the main checkout first.
pub fn worktrees(dir: &Path) -> Vec<(PathBuf, Option<String>)> {
    let out = git(dir, &["worktree", "list", "--porcelain"]).unwrap_or_default();
    let mut v: Vec<(PathBuf, Option<String>)> = Vec::new();
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            let p = PathBuf::from(p);
            v.push((crate::paths::canon(&p).unwrap_or(p), None));
        } else if let Some(b) = line.strip_prefix("branch refs/heads/")
            && let Some(last) = v.last_mut()
        {
            last.1 = Some(b.to_string());
        }
    }
    v
}

/// Where `branch` is checked out, if anywhere.
pub fn checkout_of(dir: &Path, branch: &str) -> Option<PathBuf> {
    worktrees(dir).into_iter().find(|(_, b)| b.as_deref() == Some(branch)).map(|(p, _)| p)
}

/// Run `gh` (echoed like the git commands that change something).
pub fn gh(dir: &Path, args: &[&str]) -> Result<String> {
    echo(dir, "gh", args, 1);
    let out =
        Command::new("gh").args(args).current_dir(dir).output().with_context(|| format!("running gh {}", args.first().unwrap_or(&"")))?;
    if !out.status.success() {
        bail!("gh {} failed: {}", args.iter().take(2).copied().collect::<Vec<_>>().join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
