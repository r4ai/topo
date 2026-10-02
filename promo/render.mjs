// Renders the promo video frame by frame in headless Chrome and encodes it with ffmpeg.
//
//   node render.mjs                       full video → out/topo-pv.mp4
//   node render.mjs --from 8 --to 12      a section
//   node render.mjs --stills 0.5,4.2 --dir /tmp/x   PNG stills (for review)
//   node render.mjs --serve               serve the live preview at http://127.0.0.1:8765/
//
// Options: --fps 60 --samples 4 (motion blur sub-frames) --workers 6 --scale 1 --crf 15 --out file
import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, existsSync, writeFileSync, rmSync, statSync, createReadStream } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const args = Object.fromEntries(
  process.argv.slice(2).flatMap((a, i, all) => (a.startsWith("--") ? [[a.slice(2), all[i + 1] && !all[i + 1].startsWith("--") ? all[i + 1] : true]] : [])),
);
const fps = +(args.fps || 60);
const samples = +(args.samples || 4);
const workers = +(args.workers || 6);
const scale = +(args.scale || 1);
const DURATION = 58;

// ---- static server ---------------------------------------------------------------
const MIME = { ".html": "text/html", ".js": "text/javascript", ".json": "application/json", ".png": "image/png", ".wav": "audio/wav", ".svg": "image/svg+xml" };
const server = createServer((req, res) => {
  const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
  const file = join(root, path === "/" ? "index.html" : path);
  if (!file.startsWith(root) || !existsSync(file) || statSync(file).isDirectory()) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { "content-type": MIME[extname(file)] || "application/octet-stream", "cache-control": "no-store" });
  createReadStream(file).pipe(res);
});
// The camera moves inside the app are stored as video; the page reads them as frames.
const seqDir = join(root, "assets", "shots", "seq");
for (const f of readdirSync(join(root, "assets", "moves"))) {
  const key = f.replace(".mp4", "");
  const src = join(root, "assets", "moves", f);
  const first = join(seqDir, `s_${key}_000.jpg`);
  if (existsSync(first) && statSync(first).mtimeMs >= statSync(src).mtimeMs) continue;
  mkdirSync(seqDir, { recursive: true });
  spawnSync("ffmpeg", ["-v", "error", "-y", "-i", src, "-start_number", "0", "-q:v", "2", join(seqDir, `s_${key}_%03d.jpg`)], { stdio: "inherit" });
}

await new Promise((ok) => server.listen(args.serve ? 8765 : 0, "127.0.0.1", ok));
const port = server.address().port;
if (args.serve) {
  console.log(`preview: http://127.0.0.1:${port}/`);
  await new Promise(() => {});
}

// ---- headless Chrome over the DevTools protocol -------------------------------------
const CHROME = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const profile = mkdtempSync(join(tmpdir(), "topo-pv-"));
const chrome = spawn(CHROME, [
  "--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "--no-first-run", "--no-default-browser-check",
  "--hide-scrollbars", "--mute-audio", "--force-device-scale-factor=1", "--window-size=1920,1080", "--disable-background-timer-throttling",
  "--disable-renderer-backgrounding", "--disable-backgrounding-occluded-windows", "about:blank",
], { stdio: "ignore" });
const cleanup = () => {
  chrome.kill();
  server.close();
  try {
    rmSync(profile, { recursive: true, force: true });
  } catch {}
};
process.on("exit", cleanup);

const portFile = join(profile, "DevToolsActivePort");
for (let i = 0; i < 200 && !existsSync(portFile); i++) await new Promise((ok) => setTimeout(ok, 50));
const debugPort = readFileSync(portFile, "utf8").split("\n")[0];

async function page() {
  const target = await (await fetch(`http://127.0.0.1:${debugPort}/json/new?about:blank`, { method: "PUT" })).json();
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((ok, fail) => {
    ws.onopen = ok;
    ws.onerror = fail;
  });
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && waiting.has(msg.id)) {
      const { ok, fail } = waiting.get(msg.id);
      waiting.delete(msg.id);
      msg.error ? fail(new Error(msg.error.message)) : ok(msg.result);
    }
  };
  const send = (method, params = {}) =>
    new Promise((ok, fail) => {
      waiting.set(++id, { ok, fail });
      ws.send(JSON.stringify({ id, method, params }));
    });
  const evaluate = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result.value;
  };
  await send("Page.enable");
  await send("Page.navigate", { url: `http://127.0.0.1:${port}/index.html?render=1` });
  for (let i = 0; ; i++) {
    if (await evaluate("window.PVready === true").catch(() => false)) break;
    if (i > 400) throw new Error("page did not become ready");
    await new Promise((ok) => setTimeout(ok, 50));
  }
  return { evaluate };
}

const capture = async (p, t, type, quality) => {
  const url = await p.evaluate(`PVcapture(${t}, ${samples}, "${type}", ${quality})`);
  return Buffer.from(url.slice(url.indexOf(",") + 1), "base64");
};

if (args.stills) {
  const dir = args.dir || join(root, "out", "stills");
  mkdirSync(dir, { recursive: true });
  const p = await page();
  for (const s of String(args.stills).split(",")) {
    const t = +s;
    writeFileSync(join(dir, `t${t.toFixed(2).padStart(5, "0")}.png`), await capture(p, t, "image/png", 1));
  }
  console.log("stills →", dir);
  process.exit(0);
}

// ---- video ------------------------------------------------------------------------------
const from = +(args.from || 0);
const to = +(args.to || DURATION);
const first = Math.round(from * fps);
const count = Math.round((to - from) * fps);
const outFile = args.out || join(root, "out", "topo-pv.mp4");
const audio = join(root, "out", "audio.wav");
mkdirSync(dirname(outFile), { recursive: true });

const vf = [`scale=${Math.round(1920 * scale)}:${Math.round(1080 * scale)}:flags=lanczos:in_range=pc:out_range=tv:in_color_matrix=bt601:out_color_matrix=bt709`, "format=yuv420p"];
const ff = spawn("ffmpeg", [
  "-v", "error", "-y", "-f", "image2pipe", "-framerate", String(fps), "-c:v", "mjpeg", "-i", "-",
  ...(existsSync(audio) ? ["-ss", String(from), "-t", String(to - from), "-i", audio] : []),
  "-vf", vf.join(","), "-c:v", "libx264", "-preset", args.preset || "slow", "-crf", String(args.crf || 15),
  "-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709", "-color_range", "tv",
  ...(existsSync(audio) ? ["-c:a", "aac", "-b:a", "256k"] : []),
  "-movflags", "+faststart", "-shortest", outFile,
], { stdio: ["pipe", "inherit", "inherit"] });

const pages = await Promise.all(Array.from({ length: Math.min(workers, count) }, page));
const done = new Map();
let next = 0;
let written = 0;
const started = Date.now();
const flush = async () => {
  while (done.has(written)) {
    const buf = done.get(written);
    done.delete(written);
    written++;
    if (!ff.stdin.write(buf)) await new Promise((ok) => ff.stdin.once("drain", ok));
  }
};
await Promise.all(
  pages.map(async (p) => {
    while (next < count) {
      const i = next++;
      // Keep the reorder buffer small: do not run far ahead of the encoder.
      while (i - written > workers * 4) await new Promise((ok) => setTimeout(ok, 5));
      done.set(i, await capture(p, (first + i) / fps, "image/jpeg", 0.97));
      await flush();
      if (i % 120 === 0) {
        const el = (Date.now() - started) / 1000;
        console.log(`frame ${i}/${count}  ${(i / Math.max(el, 0.001)).toFixed(1)} fps`);
      }
    }
  }),
);
await flush();
ff.stdin.end();
await new Promise((ok) => ff.on("close", ok));
console.log("wrote", outFile, `${((Date.now() - started) / 1000).toFixed(0)}s`);
process.exit(0);
