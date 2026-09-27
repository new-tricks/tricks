import * as assert from "assert";
import * as cp from "child_process";
import * as fs from "fs";
import * as path from "path";
import * as vscode from "vscode";
import { remoteUri, versionUri } from "../docs";

export async function run(): Promise<void> {
  const ext = vscode.extensions.getExtension("newtricks.new-tricks");
  assert.ok(ext, "extension not found");
  const api: any = await ext!.activate();
  // Commands are registered.
  const cmds = await vscode.commands.getCommands(true);
  for (const c of ["tricks.search", "tricks.update", "tricks.experimentStart", "tricks.experimentCommit", "tricks.experimentMerge", "tricks.experimentDiscard", "tricks.experimentOpenFolder", "tricks.createSkill", "tricks.removeSkill", "tricks.publish", "tricks.changes", "tricks.linkAll", "tricks.linkToProject", "tricks.try"]) {
    assert.ok(cmds.includes(c), `missing command ${c}`);
  }
  // Status via the real binary over JSON-RPC.
  await api.refreshAll();
  const st = api.model.status;
  assert.ok(st?.source_repo, `no source repo in status: ${api.model.error}`);
  const names = st.source_repo.skills.map((s: any) => s.name).sort();
  assert.deepStrictEqual(names, ["broken", "hello"]);
  const hello = st.source_repo.skills.find((s: any) => s.name === "hello");
  assert.strictEqual(hello.upstream, "github.com/acme/skills//skills/hello");
  // Lint diagnostics land in the Problems panel.
  const root = st.source_repo.root;
  const diags = vscode.languages.getDiagnostics(vscode.Uri.file(path.join(root, "skills/broken/SKILL.md")));
  assert.ok(diags.some((d) => String(d.code) === "NT302"), `expected NT302, got ${diags.map((d) => d.code).join(",")}`);
  // Virtual documents: remote preview and base version.
  // Round-trip through a string, as the Markdown preview does.
  const remote = await vscode.workspace.openTextDocument(vscode.Uri.parse(remoteUri("acme/skills//hello", "SKILL.md").toString()));
  assert.ok(remote.getText().includes("# Hello"), remote.getText());
  const base = await vscode.workspace.openTextDocument(vscode.Uri.parse(versionUri("hello", "base", "SKILL.md").toString()));
  assert.ok(base.getText().includes("Greets people"), base.getText());
  // Markdown preview of a remote skill opens without error.
  await vscode.commands.executeCommand("markdown.showPreview", remoteUri("acme/skills//hello", "SKILL.md"));
  // Status bar reflects lint state.
  assert.ok(String(api.status.text).length > 0);

  // Confirmations the core asks for are answered yes, and recorded.
  const prompts: string[] = [];
  api.client.confirm = async (prompt: string) => {
    prompts.push(prompt);
    return true;
  };

  // Link the source repo's skills for the agents, then remove the links again (removing
  // more than one asks first).
  await vscode.commands.executeCommand("tricks.linkAll");
  const linked = api.model.status.links;
  assert.ok(linked.length >= 2 && linked.every((l: any) => l.kind === "dev" && l.scope === "global"), JSON.stringify(linked));
  await vscode.commands.executeCommand("tricks.unlinkAll");
  assert.strictEqual(api.model.status.links.length, 0);
  assert.strictEqual(prompts.length, 1, "unlinking all asks for confirmation once");

  // Start an experiment, commit it and merge it back (the RPC calls Start Experiment…,
  // Commit Experiment… and Merge Experiment… make).
  const exp = await api.client.request("experiment/start", { spec: "hello@polish" });
  assert.strictEqual(exp.branch, "experiment/hello/polish", JSON.stringify(exp));
  fs.writeFileSync(path.join(exp.path, "SKILL.md"), fs.readFileSync(path.join(exp.path, "SKILL.md"), "utf8") + "\nPolished.\n");
  await api.refreshAll();
  assert.deepStrictEqual(api.model.skill("hello").experiments, ["polish"]);
  const committed = await api.client.request("experiment/commit", { spec: "hello@polish", message: "polish hello" });
  assert.ok(committed.commit && committed.branch === "experiment/hello/polish", JSON.stringify(committed));
  const merged = await api.client.request("experiment/merge", { spec: "hello@polish" });
  assert.ok(merged.commit && merged.into === "main" && merged.cleaned_up && !merged.conflicts.length, JSON.stringify(merged));
  assert.ok(fs.readFileSync(path.join(st.source_repo.root, "skills/hello/SKILL.md"), "utf8").includes("Polished."));
  await api.refreshAll();
  assert.deepStrictEqual(api.model.skill("hello").experiments, []);

  // Discover: messages from the webview go through the real core.
  const posts: any[] = [];
  const sub = api.discover.onDidPost((m: any) => posts.push(m));
  await api.discover.handle({ type: "search", params: { query: "greets", limit: 40 } });
  assert.deepStrictEqual(posts[0], { type: "status", text: "Searching…" });
  assert.strictEqual(posts[1]?.type, "results", JSON.stringify(posts));
  const hit = posts[1].results.find((r: any) => r.id === "github.com/acme/skills//skills/hello");
  assert.ok(hit, JSON.stringify(posts[1].results.map((r: any) => r.id)));
  assert.strictEqual(hit.license_class, "allow");
  assert.strictEqual(hit.vendored, true);
  await api.discover.handle({ type: "copy", id: hit.id });
  assert.strictEqual(await vscode.env.clipboard.readText(), hit.id);
  // The Discover command focuses the view and hands it the query.
  posts.length = 0;
  await vscode.commands.executeCommand("tricks.search", "hello");
  assert.ok(posts.some((m) => m.type === "setQuery" && m.query === "hello"), JSON.stringify(posts));
  sub.dispose();

  // Publish: the pre-flight panel opens with the core's report, and publishing from it
  // goes through the core's confirmation and pushes to the target repository.
  prompts.length = 0;
  await vscode.commands.executeCommand("tricks.publish");
  const panel = api.PublishPanel.current;
  assert.ok(panel, "publish panel did not open");
  assert.strictEqual(panel.target, "public");
  assert.strictEqual(panel.report.blocked, false, JSON.stringify(panel.report.gates));
  assert.deepStrictEqual(panel.report.skills, ["hello"]);
  assert.ok(panel.panel.webview.html.includes("Publish <code>public</code>"));
  const published = await panel.handle({ type: "publish", bump: "minor", push: true });
  assert.ok(published?.commit, JSON.stringify(published));
  assert.strictEqual(published.tag, "v0.1.0");
  assert.strictEqual(prompts.length, 1, "publishing asks for confirmation once");
  assert.strictEqual(api.PublishPanel.current, undefined, "panel closes after publishing");
  const pub = path.join(root, "..", "pub.git");
  assert.ok(cp.execFileSync("git", ["show", "main:skills/hello/SKILL.md"], { cwd: pub, encoding: "utf8" }).includes("name: hello"));
  assert.strictEqual(cp.execFileSync("git", ["tag"], { cwd: pub, encoding: "utf8" }).trim(), "v0.1.0");
  console.log("extension integration tests passed");
}
