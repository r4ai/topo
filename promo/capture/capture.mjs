// Captures the real product surfaces used by the promo video:
//   - GUI frames, written by the app itself through the promo hook (see promo-hook.patch)
//   - CLI output of the real `topo` binary
// Usage: TOPO_BIN=<dir with topo, topo-gui(hooked)> WORK=<scratch dir> node capture.mjs
import { execFileSync, spawnSync } from "node:child_process";
import { cpSync, mkdirSync, readdirSync, rmSync, writeFileSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const BIN = resolve(process.env.TOPO_BIN);
const WORK = resolve(process.env.WORK);
const OUT = resolve(here, "../assets/shots");
const TOPO = join(BIN, "topo");
const GUI = join(BIN, "topo-gui");

rmSync(WORK, { recursive: true, force: true });
rmSync(OUT, { recursive: true, force: true });
mkdirSync(OUT, { recursive: true });

const topo = (cwd, args, input) => execFileSync(TOPO, args, { cwd, input, encoding: "utf8" });

// ---- workspace ---------------------------------------------------------
const ws = join(WORK, "launch");
mkdirSync(ws, { recursive: true });
topo(ws, ["init"]);
const batch = [
  { op: "add", ref: "m", title: "v1.0 Release", kind: "milestone" },
  { op: "add", ref: "req", title: "Define requirements" },
  { op: "add", ref: "ci", title: "Set up CI" },
  { op: "add", ref: "ui", title: "Design UI", depends_on: ["$req"] },
  { op: "add", ref: "db", title: "Design DB schema", depends_on: ["$req"] },
  { op: "add", ref: "auth", title: "Write auth spec", depends_on: ["$req"] },
  { op: "add", ref: "fe", title: "Build frontend", depends_on: ["$ui"] },
  { op: "add", ref: "api", title: "Build API", depends_on: ["$db", "$auth"] },
  { op: "add", ref: "test", title: "Integration tests", depends_on: ["$api", "$fe", "$ci"], in: ["$m"] },
  { op: "status", id: "$req", status: "done" },
];
const ids = JSON.parse(topo(ws, ["apply", "--json"], JSON.stringify(batch))).refs;

const agentBatch = [
  { op: "add", ref: "m2", title: "Launch campaign", kind: "milestone" },
  { op: "add", ref: "price", title: "Draft pricing page", depends_on: [ids.req], in: ["$m2"] },
  { op: "add", ref: "mail", title: "Write onboarding emails", depends_on: [ids.req] },
  { op: "add", ref: "stats", title: "Set up analytics", depends_on: [ids.ci], in: ["$m2"] },
  { op: "add", ref: "blog", title: "Publish blog post", depends_on: ["$price", "$mail"], in: ["$m2"] },
  { op: "add", ref: "demo", title: "Record demo video", depends_on: [ids.fe], in: ["$m2"] },
];
writeFileSync(join(WORK, "agent.json"), JSON.stringify(agentBatch, null, 2));

// ---- GUI script --------------------------------------------------------
const lines = [];
const shot = (name) => lines.push(`shot ${join(OUT, name + ".png")}`);
const cmd = (...l) => lines.push(...l);
const smooth = (t) => t * t * (3 - 2 * t);

// 1. Build the graph by hand: Tab creates a follow-up, a drag adds it to the milestone.
const frameConnect = `frame 1.25 0.5 0.42 ${ids.api} ${ids.test} ${ids.m}`;
cmd(frameConnect);
shot("c_init");
cmd(`select ${ids.api}`);
shot("c_sel");
cmd("followup");
shot("c_type_00");
const title = "Write docs";
for (let i = 1; i <= title.length; i++) {
  cmd(`type ${title.slice(0, i)}`);
  shot(`c_type_${String(i).padStart(2, "0")}`);
}
cmd("submit", "noanim", frameConnect, "notoast");
shot("c_created");
const DRAG = 24;
for (let i = 0; i < DRAG; i++) {
  cmd(`dragto @sel ${ids.m} ${smooth((i + 1) / DRAG).toFixed(4)}`);
  shot(`c_drag_${String(i).padStart(2, "0")}`);
}
cmd(`drop @sel ${ids.m}`, "noanim", frameConnect, "notoast");
shot("c_linked_sel");

// 2. Pull back to the whole graph with the milestone still selected: its critical path is lit.
const PULL = 30;
cmd("mark a", "fitall", "mark b");
for (let i = 0; i < PULL; i++) {
  cmd(`mix ${(i / (PULL - 1)).toFixed(4)}`);
  shot(`y_${String(i).padStart(2, "0")}`);
}
shot("e_crit");
cmd("select -");
shot("e_init");

// 3. Zoom in on the work that is ready, then finish it: dependents unblock.
const ZOOM = 40;
cmd("mark a", `frame 1.5 0.5 0.5 ${ids.db} ${ids.auth} ${ids.ui} ${ids.api} ${ids.fe}`, "mark b");
for (let i = 0; i < ZOOM; i++) {
  cmd(`mix ${(i / (ZOOM - 1)).toFixed(4)}`);
  shot(`z_${String(i).padStart(2, "0")}`);
}
shot("r_0");
cmd(`status ${ids.db} done`);
shot("r_1");
cmd(`status ${ids.auth} done`);
shot("r_2");
cmd(`status ${ids.ui} done`);
shot("r_3");
cmd(`status ${ids.api} doing`);
shot("r_4");

// 4. Whole window again; a CLI edit shows up live.
cmd("fitall");
shot("e_after");
cmd(`sh cd ${ws} && ${TOPO} status ${ids.api} done`, "wait 900");
shot("e_sync");
cmd("notoast");
shot("a_before");

// 5. An agent applies a batch atomically.
cmd(`sh cd ${ws} && ${TOPO} apply ${join(WORK, "agent.json")}`, "wait 900", "fitall");
shot("a_toast");
cmd("notoast");
shot("a_after");
cmd("help 1");
shot("h_help");
cmd("quit");

writeFileSync(join(WORK, "gui.promo"), lines.join("\n") + "\n");

// Snapshot for the CLI captures before the GUI session mutates the workspace.
const wsCli = join(WORK, "cli", "launch");
cpSync(ws, wsCli, { recursive: true });

const run = spawnSync(GUI, ["."], {
  cwd: ws,
  env: { ...process.env, TOPO_PROMO: join(WORK, "gui.promo"), TOPO_WIN: "1440x810", TOPO_TITLE: "topo — ~/work/launch" },
  encoding: "utf8",
});
if (run.status !== 0) throw new Error("gui capture failed:\n" + run.stderr);

// ---- CLI ---------------------------------------------------------------
// Same states as the GUI session, replayed on the snapshot (same node ids).
const cli = {};
const git = (...args) => execFileSync("git", args, { cwd: wsCli, encoding: "utf8" });
const docs = JSON.parse(topo(wsCli, ["add", "Write docs", "--dep", ids.api, "--in", ids.m, "--json"]));
ids.docs = docs.id ?? docs;
git("init", "-q");
git("add", "-A");
git("-c", "user.name=topo", "-c", "user.email=topo@example.com", "commit", "-qm", "plan v1.0");
cli.ready0 = topo(wsCli, ["ready"]);
cli.milestones0 = topo(wsCli, ["milestones"]);
cli.tree0 = topo(wsCli, ["graph", "--format", "tree"]);
cli.show_api = topo(wsCli, ["show", ids.api]);
cli.file_api = readFileSync(join(wsCli, ".topo/nodes", ids.api + ".md"), "utf8");
cli.done_db = topo(wsCli, ["status", ids.db, "done"]);
cli.ready1 = topo(wsCli, ["ready"]);
cli.done_auth = topo(wsCli, ["status", ids.auth, "done"]);
cli.ready2 = topo(wsCli, ["ready"]);
cli.diff = git("diff", "--no-color");
cli.done_ui = topo(wsCli, ["status", ids.ui, "done"]);
cli.ready3 = topo(wsCli, ["ready"]);
cli.milestones3 = topo(wsCli, ["milestones"]);
topo(wsCli, ["status", ids.api, "doing"]);
// The real TUI, before and after the same CLI edit the GUI picks up live.
execFileSync(
  "uv",
  ["run", "--quiet", "--with", "pyte", join(here, "tui.py"), TOPO, wsCli, join(OUT, "tui.json"), "74", "17",
    "dump:before", `sh:${TOPO} status ${ids.api} done`, "dump:after", "keys:2", "dump:milestones"],
  { stdio: "inherit" },
);
cli.ready_json = topo(wsCli, ["ready", "--json"]);
cli.apply = topo(wsCli, ["apply", join(WORK, "agent.json")]);
cli.agent_batch = readFileSync(join(WORK, "agent.json"), "utf8");
writeFileSync(join(OUT, "cli.json"), JSON.stringify({ ids, cli }, null, 2));

// One bundle for the page: where every node sits in every frame, plus the CLI and TUI text.
const shots = {};
for (const f of readdirSync(OUT)) {
  if (f.endsWith(".png.json")) shots[f.replace(".png.json", "")] = JSON.parse(readFileSync(join(OUT, f), "utf8"));
}
const tui = JSON.parse(readFileSync(join(OUT, "tui.json"), "utf8"));
writeFileSync(join(OUT, "../data.json"), JSON.stringify({ shots, ids, cli, tui }));
console.log(`captured ${Object.keys(shots).length} frames`);
