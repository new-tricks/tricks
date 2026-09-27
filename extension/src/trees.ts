import * as path from "path";
import * as vscode from "vscode";
import { LinkInfo, Model, RepoSkill } from "./model";

export class SkillItem extends vscode.TreeItem {
  constructor(
    public readonly skillName: string,
    label: string,
    collapsible: vscode.TreeItemCollapsibleState,
    public readonly kind: "repoSkill" | "detail" | "link" | "experiment",
    public readonly data?: RepoSkill | LinkInfo,
    /** Experiment items: the experiment's name. */
    public readonly experiment?: string,
  ) {
    super(label, collapsible);
  }
}

/** Source repo view: skills with state badges (spec §14). */
export class SourceRepoTree implements vscode.TreeDataProvider<SkillItem> {
  private readonly emitter = new vscode.EventEmitter<SkillItem | undefined>();
  readonly onDidChangeTreeData = this.emitter.event;

  constructor(private readonly model: Model) {
    model.onDidChange(() => this.emitter.fire(undefined));
  }

  getTreeItem(e: SkillItem): vscode.TreeItem {
    return e;
  }

  getChildren(e?: SkillItem): SkillItem[] {
    const ws = this.model.status?.source_repo;
    if (!ws) return [];
    if (!e) {
      return ws.skills.map((s) => {
        const it = new SkillItem(s.name, s.name, vscode.TreeItemCollapsibleState.Collapsed, "repoSkill", s);
        const badges: string[] = [];
        if (s.merge_in_progress) badges.push("updating");
        if (s.update_available) badges.push(`update ${s.update_available}`);
        if (s.customized) badges.push("customized");
        if (s.lint_errors) badges.push(`${s.lint_errors} lint error${s.lint_errors > 1 ? "s" : ""}`);
        if (s.experiments.length === 1) badges.push(`experiment ${s.experiments[0]}`);
        else if (s.experiments.length) badges.push(`${s.experiments.length} experiments`);
        if (s.uncommitted) badges.push("uncommitted");
        if (s.branches.length) badges.push(`${s.branches.length} branch${s.branches.length > 1 ? "es" : ""}`);
        it.description = badges.join(" · ");
        const ctx = ["repoSkill"];
        if (s.upstream) ctx.push("vendored");
        if (s.update_available) ctx.push("updateReady");
        if (s.merge_in_progress) ctx.push("updating");
        if (s.experiments.length) ctx.push("hasExperiments");
        it.contextValue = ctx.join(" ");
        it.iconPath = new vscode.ThemeIcon(
          s.merge_in_progress ? "git-merge" : s.lint_errors ? "error" : s.update_available ? "cloud-download" : s.upstream ? "repo-forked" : "file-code",
        );
        it.tooltip = new vscode.MarkdownString(
          [
            `**${s.name}** — \`${s.path}\``,
            s.upstream ? `upstream \`${s.upstream}\` @ \`${(s.base ?? "").slice(0, 9)}\`` : "local original",
            s.license ? `licence ${s.license.spdx ?? "none"} (${s.license.class})` : "",
            `lint: ${s.lint_errors} error(s), ${s.lint_warnings} warning(s)`,
          ]
            .filter(Boolean)
            .join("\n\n"),
        );
        it.command = { command: "tricks.openSkill", title: "Open", arguments: [it] };
        return it;
      });
    }
    const s = e.data as RepoSkill;
    const d = (label: string, icon: string, cmd?: vscode.Command) => {
      const it = new SkillItem(e.skillName, label, vscode.TreeItemCollapsibleState.None, "detail");
      it.iconPath = new vscode.ThemeIcon(icon);
      if (cmd) it.command = cmd;
      return it;
    };
    const out: SkillItem[] = [];
    out.push(d(s.upstream ? `from ${s.upstream.replace(/^github\.com\//, "")}` : "local original", s.upstream ? "repo" : "home"));
    if (s.update_available) out.push(d(`upstream has ${s.update_available} — update`, "cloud-download", { command: "tricks.update", title: "Update", arguments: [e] }));
    if (s.merge_in_progress) {
      out.push(d("update in progress — continue", "check", { command: "tricks.updateContinue", title: "Continue", arguments: [e] }));
      out.push(d("abort update", "discard", { command: "tricks.updateAbort", title: "Abort", arguments: [e] }));
    }
    if (s.upstream) out.push(d("show changes…", "diff", { command: "tricks.changes", title: "Changes", arguments: [e] }));
    for (const x of s.experiments) {
      const it = new SkillItem(e.skillName, `experiment ${x}`, vscode.TreeItemCollapsibleState.None, "experiment", s, x);
      it.iconPath = new vscode.ThemeIcon("beaker");
      it.tooltip = `${e.skillName}@${x} on branch experiment/${e.skillName}/${x}`;
      it.contextValue = "experiment";
      it.command = { command: "tricks.openSkill", title: "Open", arguments: [it] };
      out.push(it);
    }
    for (const b of s.branches) {
      const it = d(`branch ${b}`, "git-branch");
      it.contextValue = "branch";
      out.push(it);
    }
    if (s.dev_links) out.push(d(`${s.dev_links} link(s)`, "link"));
    return out;
  }

  refresh(): void {
    this.emitter.fire(undefined);
  }
}

/** Links view: source repo skills (dev) and upstream skills under trial, deployed for testing. */
export class LinksTree implements vscode.TreeDataProvider<SkillItem> {
  private readonly emitter = new vscode.EventEmitter<SkillItem | undefined>();
  readonly onDidChangeTreeData = this.emitter.event;

  constructor(private readonly model: Model) {
    model.onDidChange(() => this.emitter.fire(undefined));
  }

  getTreeItem(e: SkillItem): vscode.TreeItem {
    return e;
  }

  getChildren(e?: SkillItem): SkillItem[] {
    const links = [...(this.model.status?.links ?? []), ...(this.model.status?.trials ?? [])];
    if (!e) {
      const groups: SkillItem[] = [];
      for (const [kind, label, icon] of [
        ["dev", "Source repo skills", "file-code"],
        ["trial", "Trying", "beaker"],
      ] as const) {
        const n = links.filter((l) => l.kind === kind).length;
        if (!n) continue;
        const g = new SkillItem(kind, `${label} (${n})`, vscode.TreeItemCollapsibleState.Expanded, "detail");
        g.iconPath = new vscode.ThemeIcon(icon);
        g.contextValue = kind === "dev" ? "linkGroup" : "trialGroup";
        groups.push(g);
      }
      return groups;
    }
    return links.filter((l) => l.kind === e.skillName).map(linkItem);
  }
}

/** Display name of a link's skill: the source repo skill name or the upstream id. */
export function linkSkillName(l: LinkInfo): string {
  if (l.skill.startsWith("ws:")) return l.skill.slice(l.skill.lastIndexOf("//") + 2);
  return l.skill.replace(/^github\.com\//, "");
}

/**
 * What a source repo skill's link deploys, as the CLI shows it: `main (working tree, live)`,
 * `experiment/pdf/terse (worktree, live, pinned)`, `v1.2.0 @ 3f2a1c9 (snapshot, pinned)`; a copy
 * says `copy` instead of `live` (it is refreshed only when the link is placed again).
 */
export function linkBranch(l: LinkInfo): string {
  if (!l.source) return "";
  const b = l.branch ?? "detached";
  const live = l.mode === "copy" ? "copy" : "live";
  const kind = l.source === "working-tree" ? ["working tree", live] : l.source === "worktree" ? ["worktree", live] : ["snapshot"];
  const notes = [...kind, l.pinned ? "pinned" : ""].filter(Boolean);
  const at = l.source === "snapshot" && l.commit ? ` @ ${l.commit.slice(0, 7)}` : "";
  return `${b}${at}${notes.length ? ` (${notes.join(", ")})` : ""}`;
}

function linkItem(l: LinkInfo): SkillItem {
  const scope = l.scope === "global" ? "user scope" : path.basename(l.scope);
  const it = new SkillItem(l.skill, `${linkSkillName(l)} · ${l.agent}`, vscode.TreeItemCollapsibleState.None, "link", l);
  const from = linkBranch(l);
  it.description = `${scope}${from ? ` · ${from}` : ""} · ${l.mode}${l.health !== "ok" ? ` · ${l.health}` : ""}`;
  it.tooltip = `${l.path}\n${l.skill}`;
  it.iconPath = new vscode.ThemeIcon(l.health === "ok" ? "pass" : "warning");
  it.contextValue = l.kind === "trial" ? "trial" : "link";
  it.resourceUri = vscode.Uri.file(l.path);
  return it;
}
