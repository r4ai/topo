// 16–40 s: the product itself. Every window here is a frame the real app wrote
// of its own window; the overlays (pointer, key caps, rings, comets) only point at it.
(function (g) {
  const PV = g.PV;
  const E = PV.E;
  const { W, H, C, clamp, lerp, prog, ease, ep, hit, rgba, font, text, rr } = E;
  const T = PV.T;
  const smooth = (x) => x * x * (3 - 2 * x);
  const pad2 = (n) => String(n).padStart(2, "0");

  const BASE = { x: 160, y: 236, w: 1600 };
  const AGENT = { x: 700, y: 262, w: 1700 };
  const CANVAS_PT = [0, 46, 1080, 764];

  // The critical path of the captured workspace, as its inspector lists it.
  const CRIT_PATH = ["Write auth spec", "Build API", "Write docs"];
  const GOAL = "v1.0 Release";
  // Rows of the inspector's "Ready now" list, top to bottom.
  const READY_ROWS = ["Design UI", "Write auth spec", "Design DB schema", "Set up CI"];
  const TITLE = { db: "Design DB schema", auth: "Write auth spec", ui: "Design UI", api: "Build API", fe: "Build frontend" };

  // ---- captions --------------------------------------------------------------------
  const CAPTIONS = [
    { t0: 16, t1: 20, head: "Sketch the plan.", sub: "Tab adds what comes next. Drag to connect." },
    { t0: 20, t1: 24, head: "See the critical path.", sub: "The longest chain to your milestone, found for you.", colors: [0, 0, C.amber, C.amber] },
    { t0: 24, t1: 28, head: "Know what's ready.", sub: "Only the tasks with nothing in their way.", colors: [0, 0, C.green] },
    { t0: 28, t1: 32, head: "Finish one. Unlock the next.", sub: "Blocked work frees itself.", colors: [0, 0, C.green, C.green, C.green] },
    { t0: 32, t1: 36, head: "GUI. TUI. CLI.", sub: "One graph. Every edit shows up live.", times: [0, 0.5, 1] },
    { t0: 36, t1: 38, head: "Plain Markdown. Git-native.", sub: "One node, one file. Diff, branch, merge.", times: [0, 0.07, 1, 1], colors: [0, 0, C.green] },
    { t0: 38, t1: 40, head: "Agent-ready.", sub: "A whole plan lands as one atomic batch.", colors: [C.accent] },
  ];

  // Headlines roll into each other: the old one is still leaving upwards
  // through its line while the next one rises into the same place.
  function captions(ctx, t) {
    for (const cap of CAPTIONS) {
      const lt = t - cap.t0;
      const len = cap.t1 - cap.t0;
      if (lt < 0 || lt > len + 0.7) continue;
      const last = cap.t1 >= 40;
      // The old line is on its way out just as the next one starts to rise.
      const out = len - (last ? 0.5 : 0.3);
      E.riseText(ctx, cap.head, 158, 172, lt, {
        size: 92, weight: 800, out, dur: 1.0, stagger: 0.08, times: cap.times,
        colors: cap.colors ? cap.colors.map((c) => c || C.text) : null,
      });
      const sa = ep(lt, 0.2, 1.1) * (1 - ep(lt, out - 0.05, 0.3, ease.inCubic));
      const sy = (1 - ep(lt, 0.2, 1.1)) * 26 - ep(lt, out - 0.05, 0.3, ease.inCubic) * 26;
      text(ctx, cap.sub, 1760, 166 + sy, { f: font(400, 30), color: C.muted, align: "right", alpha: sa });
    }
  }

  // ---- helpers ------------------------------------------------------------------------
  /** Position along `keys` = [{t, x, y}], easing between neighbours. */
  function path(keys, t) {
    if (t <= keys[0].t) return keys[0];
    for (let i = 1; i < keys.length; i++) {
      if (t <= keys[i].t) {
        const a = keys[i - 1], b = keys[i];
        const k = ease.inOutCubic(prog(t, a.hold ?? a.t, b.t - (a.hold ?? a.t)));
        return { x: lerp(a.x, b.x, k), y: lerp(a.y, b.y, k) };
      }
    }
    return keys[keys.length - 1];
  }

  function clickRipple(ctx, p, t, t0, color = "#ffffff") {
    const dt = t - t0;
    if (dt < 0 || dt > 0.45) return;
    const k = ease.outCubic(dt / 0.45);
    ctx.save();
    ctx.strokeStyle = rgba(color, (1 - k) * 0.9);
    ctx.lineWidth = 3;
    ctx.beginPath();
    ctx.arc(p.x, p.y, 8 + k * 38, 0, Math.PI * 2);
    ctx.stroke();
    ctx.restore();
  }

  function badge(ctx, x, y, label, t, t0, color) {
    if (t < t0) return;
    const k = ease.outBack(prog(t, t0, 0.3), 2.6);
    ctx.save();
    ctx.translate(x, y);
    ctx.scale(k, k);
    ctx.shadowColor = color;
    ctx.shadowBlur = 22 * (0.4 + hit(t, t0, 0.3));
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(0, 0, 19, 0, Math.PI * 2);
    ctx.fill();
    ctx.shadowColor = "transparent";
    text(ctx, label, 0, 1, { f: font(800, 22), color: "#16120a", align: "center", base: "middle" });
    ctx.restore();
  }

  function rowGlow(ctx, w, yPt, t, t0, color) {
    if (t < t0) return;
    const p = w.pt(1086, yPt - 15);
    const a = 0.1 + 0.4 * hit(t, t0, 0.3);
    ctx.save();
    rr(ctx, p.x, p.y, 344 * w.s, 30 * w.s, 6 * w.s);
    ctx.fillStyle = rgba(color, a);
    ctx.fill();
    ctx.strokeStyle = rgba(color, 0.35 + 0.65 * hit(t, t0, 0.3));
    ctx.lineWidth = 1.5;
    ctx.stroke();
    ctx.restore();
  }

  function chip(ctx, label, x, y, a, color = C.accent) {
    if (a <= 0) return;
    const f = font(700, 20, E.MONO);
    const tw = E.measure(ctx, label, f, 2);
    ctx.save();
    ctx.globalAlpha *= a;
    rr(ctx, x, y, tw + 26, 34, 8);
    ctx.fillStyle = "#0d0e12";
    ctx.fill();
    ctx.strokeStyle = rgba(color, 0.7);
    ctx.lineWidth = 1.5;
    ctx.stroke();
    text(ctx, label, x + 13, y + 18, { f, color, base: "middle", ls: 2 });
    ctx.restore();
  }

  // ---- 16–20: build the graph by hand --------------------------------------------------
  function connectShot(t) {
    const c = PV.connect;
    if (t < c.click) return "c_init";
    if (t < c.tab) return "c_sel";
    if (t < c.enter) {
      const typed = c.type.filter((x) => x <= t).length;
      return `c_type_${pad2(typed)}`;
    }
    if (t < c.drag) return "c_created";
    if (t < c.drop) return `c_drag_${pad2(Math.min(23, Math.floor(((t - c.drag) / (c.drop - c.drag)) * 24)))}`;
    return "c_linked_sel";
  }

  function connectOverlay(ctx, t, w) {
    const c = PV.connect;
    const api = E.nodeOf("c_init", "Build API"), m0 = E.meta("c_init");
    const docs = E.nodeOf("c_created", "Write docs"), ms = E.nodeOf("c_created", GOAL), m1 = E.meta("c_created");
    const a = { x: docs.x + m1.node[0], y: docs.y + m1.node[1] / 2 };
    const b = { x: ms.x + m1.node[0] / 2, y: ms.y + m1.node[1] / 2 };
    const dragAt = (u) => ({ x: lerp(a.x, b.x, u), y: lerp(a.y, b.y, u) - 46 * m1.zoom * Math.sin(Math.PI * u) });
    let cur;
    if (t >= c.drag && t < c.drop) {
      const k = Math.min(23, Math.floor(((t - c.drag) / (c.drop - c.drag)) * 24));
      cur = dragAt(smooth((k + 1) / 24));
    } else if (t >= c.drop) {
      cur = dragAt(1);
    } else {
      cur = path([
        { t: 16.05, x: 700, y: 560 },
        { t: c.click - 0.03, x: api.x + m0.node[0] * 0.62, y: api.y + m0.node[1] * 0.62, hold: 18.56 },
        { t: c.drag - 0.04, x: a.x, y: a.y },
      ], t);
    }
    const p = w.pt(cur.x, cur.y);

    // The new card and the new membership announce themselves.
    const d = w.node("c_created", "Write docs");
    E.ringBurst(ctx, d.x, d.y, d.w, d.h, 8 * d.z, t - c.enter, C.accent, { grow: 46, width: 4 });
    E.sparks(ctx, d.x + d.w / 2, d.y + d.h / 2, t - c.enter, C.accent, 3, { n: 14, dist: 210 });
    const mm = w.node("c_linked_sel", GOAL);
    E.ringBurst(ctx, mm.x, mm.y, mm.w, mm.h, 14 * mm.z, t - c.drop, C.amber, { grow: 50, width: 4 });
    E.sparks(ctx, mm.x + mm.w / 2, mm.y + mm.h / 2, t - c.drop, C.amber, 5, { n: 16, dist: 230 });

    clickRipple(ctx, p, t, c.click);
    clickRipple(ctx, p, t, c.drag, C.accent);
    clickRipple(ctx, p, t, c.drop, C.amber);
    const press = Math.max(hit(t, c.click, 0.1), t >= c.drag && t < c.drop ? 1 : 0);
    ctx.save();
    ctx.globalAlpha = ep(t, 16.1, 0.3) * (1 - ep(t, PV.seq.y.t0 - 0.12, 0.14, ease.inCubic));
    E.cursor(ctx, p.x, p.y, 1.7, press);
    ctx.restore();

    E.keycap(ctx, "Tab", 700, 968, t, c.tab, { hold: 0.5 });
    E.keycap(ctx, "↵", 700, 968, t, c.enter, { hold: 0.4 });
  }

  // ---- 20–24: the critical path ---------------------------------------------------------
  /**
   * The real frame of the app's window showing at `t` between 16 and 36 s. Camera moves
   * come from sequences captured one app frame per video frame; between them a still holds.
   * Null while the close-up composites its own states.
   */
  function guiShot(t) {
    const q = PV.seq;
    const end = (k) => q[k].t0 + q[k].d;
    if (t < q.y.t0) return connectShot(t);
    if (t < end("y")) return PV.seqFrame("y", t);
    if (t < q.x.t0) return "e_path";
    if (t < end("x")) return PV.seqFrame("x", t);
    if (t < q.z.t0) return "e_init";
    if (t < end("z")) return PV.seqFrame("z", t);
    if (t < q.w.t0) return null;
    if (t < end("w")) return PV.seqFrame("w", t);
    return t < PV.sync.flash ? "e_after" : "e_sync";
  }

  /** Overlays of the critical path, placed by where the nodes are in the frame `CR` showing now. */
  function critOverlay(ctx, t, w, CR) {
    const c = PV.crit;
    ctx.save();
    ctx.globalAlpha = 1 - ep(t, PV.seq.x.t0 - 0.3, 0.3, ease.inCubic);
    const names = [...CRIT_PATH, GOAL];
    const times = [...c.steps, c.goal];
    for (let i = 0; i < 3; i++) {
      const curve = w.edge(CR, names[i], names[i + 1]);
      E.comet(ctx, curve, prog(t, times[i], times[i + 1] - times[i]), C.amber, 6, 0.3);
    }
    names.forEach((name, i) => {
      const n = w.node(CR, name);
      const goal = i === 3;
      E.ringBurst(ctx, n.x, n.y, n.w, n.h, (goal ? 14 : 8) * n.z, t - times[i], C.amber, { grow: goal ? 60 : 34, width: goal ? 5 : 3, dur: goal ? 0.8 : 0.6 });
      if (goal) E.sparks(ctx, n.x + n.w / 2, n.y + n.h / 2, t - times[i], C.amber, 11, { n: 20, dist: 240 });
      else {
        badge(ctx, n.x + 6, n.y - 4, String(i + 1), t, times[i], C.amber);
        rowGlow(ctx, w, 391.5 + 30 * i, t, times[i], C.amber);
      }
    });
    // "3 steps" called out where the milestone sits.
    const m = w.node(CR, GOAL);
    const a = ep(t, c.goal, 0.6);
    if (a > 0) {
      ctx.save();
      ctx.globalAlpha = a;
      const bx = m.x + m.w / 2, by = m.y - 54 + (1 - ep(t, c.goal, 1.0)) * 26;
      const f = font(700, 26);
      const label = "3 steps left";
      const tw = E.measure(ctx, label, f);
      rr(ctx, bx - tw / 2 - 18, by - 24, tw + 36, 46, 23);
      ctx.fillStyle = C.amber;
      ctx.fill();
      text(ctx, label, bx, by, { f, color: "#16120a", align: "center", base: "middle" });
      ctx.restore();
    }
    ctx.restore();
  }

  // ---- 24–32: ready, and what finishing unlocks ------------------------------------------
  /** The close-up between the zoom in and the zoom out: a base still plus cards already switched. */
  function readyState(t) {
    const r = PV.ready;
    if (t >= r.space) return { base: "r_4", patches: [] };
    let base = "r_0";
    const patches = [];
    for (const d of r.done) {
      if (t >= d.arrive) base = d.shot;
      else if (t >= d.click) patches.push({ shot: d.shot, title: TITLE[d.node] });
    }
    return { base, patches };
  }

  /** `geo` is the frame showing now: overlays follow the nodes while the app's camera settles. */
  function readyOverlay(ctx, t, w, geo) {
    const r = PV.ready;
    // Spotlight: the canvas dims and each ready card lights up on its beat.
    const dimA = ep(t, 24.55, 0.9) * (1 - ep(t, PV.seq.z.t0 - 0.3, 0.3, ease.inCubic));
    if (dimA > 0) {
      const a = w.pt(CANVAS_PT[0], CANVAS_PT[1]);
      ctx.save();
      ctx.beginPath();
      ctx.rect(a.x, a.y, CANVAS_PT[2] * w.s, Math.min(CANVAS_PT[3] * w.s, H - a.y));
      READY_ROWS.forEach((name, i) => {
        if (t < r.pings[i]) return;
        const n = w.node(geo, name);
        ctx.roundRect(n.x - 5, n.y - 5, n.w + 10, n.h + 10, 8 * n.z + 4);
      });
      ctx.fillStyle = rgba("#08090c", 0.72 * dimA);
      ctx.fill("evenodd");
      ctx.restore();
      READY_ROWS.forEach((name, i) => {
        const n = w.node(geo, name);
        const dt = t - r.pings[i];
        if (dt < 0) return;
        ctx.save();
        ctx.globalAlpha = dimA;
        ctx.strokeStyle = rgba(C.green, 0.55 + 0.45 * hit(t, r.pings[i], 0.3));
        ctx.lineWidth = 2.5;
        ctx.shadowColor = C.green;
        ctx.shadowBlur = 26 * (0.3 + hit(t, r.pings[i], 0.3));
        rr(ctx, n.x - 5, n.y - 5, n.w + 10, n.h + 10, 8 * n.z + 4);
        ctx.stroke();
        ctx.restore();
        E.ringBurst(ctx, n.x, n.y, n.w, n.h, 8 * n.z, dt, C.green, { grow: 30, width: 3 });
        ctx.save();
        ctx.globalAlpha = dimA;
        rowGlow(ctx, w, 177 + 30 * i, t, r.pings[i], C.green);
        ctx.restore();
      });
    }
    if (t < PV.seq.z.t0 + 0.7 || t >= PV.seq.w.t0) return;

    // Close-up: clicks finish tasks; a comet carries the news along the real edge.
    const checkOf = (title) => {
      const m = E.meta(geo), n = E.nodeOf(geo, title);
      return { x: n.x + 19 * m.zoom, y: n.y + 22.5 * m.zoom };
    };
    const [d0, d1, d2] = r.done;
    const apiN = E.nodeOf(geo, TITLE.api), m = E.meta(geo);
    const keys = [
      { t: 26.6, x: 760, y: 640, hold: 26.62 },
      { t: d0.click - 0.03, ...checkOf(TITLE.db), hold: d0.click + 0.12 },
      { t: d1.click - 0.03, ...checkOf(TITLE.auth), hold: 28.45 },
      { t: d2.click - 0.03, ...checkOf(TITLE.ui), hold: 29.52 },
      { t: 29.84, x: apiN.x + m.node[0] * 0.7, y: apiN.y + m.node[1] * 0.64 },
    ];
    const cur = path(keys, t);
    const p = w.pt(cur.x, cur.y);

    for (const d of r.done) {
      const curve = w.edge(geo, TITLE[d.node], TITLE[d.to]);
      E.comet(ctx, curve, prog(t, d.click, d.arrive - d.click), C.green, 7, 0.3);
      const n = w.node(geo, TITLE[d.node]);
      E.ringBurst(ctx, n.x, n.y, n.w, n.h, 8 * n.z, t - d.click, C.green, { grow: 22, width: 3, dur: 0.45 });
      const to = w.node(geo, TITLE[d.to]);
      if (d.pop) {
        E.ringBurst(ctx, to.x, to.y, to.w, to.h, 8 * to.z, t - d.arrive, C.green, { grow: 70, width: 6, dur: 0.8 });
        E.ringBurst(ctx, to.x, to.y, to.w, to.h, 8 * to.z, t - d.arrive - 0.08, C.green, { grow: 40, width: 3, dur: 0.7 });
        E.sparks(ctx, to.x + to.w / 2, to.y + to.h / 2, t - d.arrive, C.green, 21 + d.arrive, { n: 26, dist: 300, dur: 0.7 });
        const glow = hit(t, d.arrive, 0.5);
        if (t >= d.arrive && glow > 0.02) {
          ctx.save();
          ctx.strokeStyle = rgba(C.green, glow);
          ctx.lineWidth = 3;
          ctx.shadowColor = C.green;
          ctx.shadowBlur = 40 * glow;
          rr(ctx, to.x, to.y, to.w, to.h, 8 * to.z);
          ctx.stroke();
          ctx.restore();
        }
      } else {
        E.ringBurst(ctx, to.x, to.y, to.w, to.h, 8 * to.z, t - d.arrive, C.muted, { grow: 18, width: 2, dur: 0.4 });
      }
      clickRipple(ctx, w.pt(checkOf(TITLE[d.node]).x, checkOf(TITLE[d.node]).y), t, d.click, C.green);
    }

    // Selecting the freed task and pressing Space starts it.
    const api = w.node(geo, TITLE.api);
    const selA = ep(t, 29.86, 0.1) * (1 - ep(t, PV.seq.w.t0 - 0.05, 0.05));
    if (selA > 0) {
      ctx.save();
      ctx.globalAlpha = selA;
      ctx.strokeStyle = C.accent;
      ctx.lineWidth = 2 * w.s * 1.2;
      rr(ctx, api.x, api.y, api.w, api.h, 8 * api.z);
      ctx.stroke();
      ctx.restore();
    }
    clickRipple(ctx, p, t, 29.86);
    E.ringBurst(ctx, api.x, api.y, api.w, api.h, 8 * api.z, t - r.space, C.accent, { grow: 40, width: 4 });
    E.keycap(ctx, "Space", 700, 968, t, r.space, { hold: 0.55, w: 230 });

    const press = Math.max(...r.done.map((d) => hit(t, d.click, 0.1)), hit(t, 29.86, 0.1));
    ctx.save();
    ctx.globalAlpha = ep(t, 26.62, 0.4) * (1 - ep(t, PV.seq.w.t0 - 0.25, 0.25));
    E.cursor(ctx, p.x, p.y, 1.7, press);
    ctx.restore();
  }

  // ---- 32–36: one graph, three surfaces ----------------------------------------------------
  function tuiScreen(ctx, x, y, screen, fs) {
    const f = font(500, fs, E.MONO);
    const cw = E.measure(ctx, "M", f);
    const lh = fs * 1.28;
    screen.forEach((row, ry) => {
      let cx = x;
      for (const run of row) {
        const wRun = cw * run.t.length;
        const reverse = run.s[3];
        if (reverse) {
          ctx.fillStyle = "#d6d8e2";
          ctx.fillRect(cx, y + ry * lh - lh * 0.5, wRun, lh);
        }
        // Monospace cells: draw per character so box lines stay on the grid.
        for (let i = 0; i < run.t.length; i++) {
          const ch = run.t[i];
          if (ch !== " ") text(ctx, ch, cx + i * cw, y + ry * lh, { f, color: reverse ? "#111216" : "─│┌┐└┘".includes(ch) ? C.faint : "#c9cbd6", base: "middle" });
        }
        cx += wRun;
      }
    });
  }

  function termLine(ctx, x, y, str, fs, color = "#c9cbd6") {
    return text(ctx, str, x, y, { f: font(500, fs, E.MONO), color, base: "middle" });
  }

  /** Middle of the two cards the CLI edit changes, in content points. */
  function syncFocus() {
    const m = E.meta("e_sync"), a = E.nodeOf("e_sync", "Build API"), b = E.nodeOf("e_sync", "Write docs");
    return { x: (a.x + b.x + m.node[0]) / 2, y: (a.y + b.y + m.node[1]) / 2 };
  }

  function syncPanels(ctx, t) {
    const s = PV.sync;
    const data = E.A.data;
    const flash = hit(t, s.flash, 0.35) * (t >= s.flash ? 1 : 0);
    const outA = 1 - ep(t, 35.7, 0.5, ease.inCubic);
    const outY = ep(t, 35.7, 0.5, ease.inCubic) * 90;

    // TUI: the real ratatui screen, as text.
    const aT = ep(t, s.panels[1] - 0.05, 0.5) * outA;
    if (aT > 0) {
      const px = 1000 + (1 - ep(t, s.panels[1] - 0.05, 1.3)) * 320, py = 262 + outY;
      const pw = 874, ph = 356;
      const o = E.panel(ctx, px, py, pw, ph, { title: "topo tui", alpha: aT, accent: flash > 0.02 ? rgba(C.green, flash) : null });
      ctx.save();
      ctx.globalAlpha = aT;
      // The empty middle of the screen is left out; the key hints stay at the bottom.
      const scr = data.tui.screens[t < s.flash ? "before" : "after"];
      tuiScreen(ctx, o.x, o.y + 6, [...scr.slice(0, 8), scr[scr.length - 2], scr[scr.length - 1]], 18.5);
      ctx.restore();
      chip(ctx, "TUI", px + pw - 78, py - 17, aT);
    }

    // CLI: the command that makes the edit.
    const aC = ep(t, s.panels[2] - 0.05, 0.5) * outA;
    if (aC > 0) {
      const px = 1090, py = 668 + (1 - ep(t, s.panels[2] - 0.05, 1.3)) * 300 + outY;
      const pw = 784, ph = 330;
      const o = E.panel(ctx, px, py, pw, ph, { title: "zsh", alpha: aC, accent: flash > 0.02 ? rgba(C.green, flash) : null });
      ctx.save();
      ctx.globalAlpha = aC;
      const cmd = `topo status ${data.ids.api} done`;
      const typed = Math.floor(prog(t, s.type[0], s.enter - 0.1 - s.type[0]) * cmd.length);
      const fs = 27, lh = 44;
      let wx = termLine(ctx, o.x, o.y + 6, "$ ", fs, C.green);
      wx += termLine(ctx, o.x + wx, o.y + 6, cmd.slice(0, typed), fs);
      if (t < s.enter && Math.floor(t * 3) % 2 === 0) {
        ctx.fillStyle = "#c9cbd6";
        ctx.fillRect(o.x + wx + 2, o.y - 10, 15, 32);
      }
      if (t >= s.enter + 0.5) {
        const w2 = termLine(ctx, o.x, o.y + 6 + lh, "$ ", fs, C.green);
        termLine(ctx, o.x + w2, o.y + 6 + lh, "topo ready", fs);
        // What `topo ready` prints now: the rows the TUI lists after the edit.
        const rows = data.tui.screens.after.slice(2, 5).map((row) => row.map((r) => r.t).join("").split("│")[1].trimEnd());
        rows.forEach((line, i) => {
          if (t >= s.enter + 0.75 + i * 0.06) termLine(ctx, o.x, o.y + 6 + lh * (2 + i), line, fs, line.includes("Write docs") ? C.green : "#c9cbd6");
        });
      }
      ctx.restore();
      chip(ctx, "CLI", px + pw - 78, py - 17, aC);
    }
  }

  // ---- 36–38: files ---------------------------------------------------------------------------
  function filesScene(ctx, t) {
    const f = PV.files;
    const data = E.A.data;
    const inA = ep(t, f.in, 0.45);
    const outK = ep(t, 37.7, 0.5, ease.inCubic);
    const a = (1 - outK);
    if (t < f.in || a <= 0) return;

    // A card unfolds into the file it is stored as.
    const cardZ = 2.6;
    const c0 = { x: W / 2 - (E.NODE_W * cardZ) / 2, y: 560 - (E.NODE_H * cardZ) / 2, w: E.NODE_W * cardZ, h: E.NODE_H * cardZ };
    const p1 = { x: 160, y: 262 - outK * 60, w: 800, h: 700 };
    const k = ease.gwan(prog(t, f.in + 0.02, 1.2));
    const R = { x: lerp(c0.x, p1.x, k), y: lerp(c0.y, p1.y, k), w: lerp(c0.w, p1.w, k), h: lerp(c0.h, p1.h, k) };
    if (k < 0.5) {
      const pop = lerp(0.7, 1, ep(t, f.in, 0.3, (x) => ease.outBack(x, 2)));
      ctx.save();
      ctx.translate(W / 2, 560);
      ctx.scale(pop * lerp(1, R.w / c0.w, k), pop * lerp(1, R.h / c0.h, k));
      ctx.translate(-W / 2, -560);
      E.card(ctx, c0.x, c0.y, cardZ, { title: "Build API", kind: "task", status: "todo", chip: "Blocked by 2" }, { alpha: inA * (1 - k * 2) });
      ctx.restore();
    }
    if (k > 0.3) {
      const pa = ep(k, 0.3, 0.4, ease.linear) * a;
      const o = E.panel(ctx, R.x, R.y, R.w, R.h, { title: `.topo/nodes/${data.ids.api}.md`, alpha: pa });
      ctx.save();
      ctx.globalAlpha = pa;
      ctx.beginPath();
      ctx.rect(R.x, R.y, R.w, R.h);
      ctx.clip();
      const fs = 34, lh = 56;
      const lines = data.cli.file_api.trimEnd().split("\n").concat(["", "Notes go here, in plain **Markdown**."]);
      lines.forEach((line, i) => {
        const la = ep(t, f.in + 0.22 + i * 0.045, 0.9);
        const y = o.y + 22 + i * lh + (1 - la) * 16;
        ctx.globalAlpha = pa * la;
        text(ctx, String(i + 1).padStart(2, " "), o.x, y, { f: font(400, fs * 0.7, E.MONO), color: "#3b3e4a", base: "middle" });
        const x = o.x + 62;
        const m = line.match(/^(\w+):(.*)$/);
        if (line === "---") termLine(ctx, x, y, line, fs, C.faint);
        else if (m) {
          const w1 = termLine(ctx, x, y, m[1] + ":", fs, C.accent);
          termLine(ctx, x + w1, y, m[2], fs, m[1] === "status" ? C.amber : C.text);
        } else if (i >= 9) termLine(ctx, x, y, line, fs * 0.82, C.muted);
        else termLine(ctx, x, y, line, fs, C.muted);
      });
      ctx.restore();
    }

    // The same change, as git sees it.
    const dA = ep(t, f.in + 0.2, 0.5) * a;
    if (dA > 0) {
      const px = 1010 + (1 - ep(t, f.in + 0.2, 1.3)) * 320, py = 330 - outK * 60, pw = 760, ph = 560;
      const o = E.panel(ctx, px, py, pw, ph, { title: "git diff", alpha: dA });
      const hunk = data.cli.diff.split("\n").slice(0, 13);
      const keep = [0, 4, 5, 6, 7, 8, 9, 10];
      ctx.save();
      ctx.globalAlpha = dA;
      ctx.beginPath();
      ctx.rect(px, py + 41, pw, ph - 42);
      ctx.clip();
      const fs = 27, lh = 52;
      keep.forEach((li, i) => {
        const line = hunk[li];
        const y = o.y + 18 + i * lh;
        const minus = line.startsWith("-") && !line.startsWith("---");
        const plus = line.startsWith("+") && !line.startsWith("+++");
        const at = minus ? f.minus : plus ? f.plus : 0;
        if (minus || plus) {
          if (t < at) return;
          const col = minus ? C.red : C.green;
          const sweep = ep(t, at, 0.6);
          ctx.fillStyle = rgba(col, 0.16 + 0.3 * hit(t, at, 0.25));
          ctx.fillRect(px + 1, y - lh / 2 + 4, (pw - 2) * sweep, lh - 8);
          ctx.fillStyle = col;
          ctx.fillRect(px + 1, y - lh / 2 + 4, 5, lh - 8);
          termLine(ctx, o.x + 6, y, line, fs, col);
        } else {
          termLine(ctx, o.x + 6, y, line.length > 40 ? line.slice(0, 39) + "…" : line, fs, li === 0 ? C.faint : li === 4 ? C.accent : C.muted);
        }
      });
      ctx.restore();
    }
  }

  // ---- 38–40: agents -----------------------------------------------------------------------------
  function agentScene(ctx, t) {
    const ag = PV.agents;
    const data = E.A.data;
    if (t < ag.in) return;
    const outA = 1 - ep(t, 39.6, 0.4, ease.inCubic);
    const inK = ep(t, ag.in - 0.1, 1.4);
    const w = E.win(AGENT.x + (1 - inK) * 500, AGENT.y, AGENT.w);
    ctx.save();
    ctx.globalAlpha = ep(t, ag.in - 0.1, 0.5) * outA;
    w.draw(ctx, t < ag.land ? "a_before" : "a_toast");
    // New nodes land together: the batch is atomic.
    const before = new Set(E.meta("a_before").nodes.map((n) => n.title));
    const fresh = E.meta("a_toast").nodes.filter((n) => !before.has(n.title)).sort((p, q) => p.x - q.x || p.y - q.y);
    fresh.forEach((n, i) => {
      const r = w.node("a_toast", n.title);
      const dt = t - ag.land - i * 0.06;
      const col = n.kind === "Milestone" ? C.amber : C.green;
      E.ringBurst(ctx, r.x, r.y, r.w, r.h, 8 * r.z, dt, col, { grow: 34, width: 3, dur: 0.6 });
      E.sparks(ctx, r.x + r.w / 2, r.y + r.h / 2, dt, col, 40 + i, { n: 8, dist: 110, dur: 0.5 });
      const gl = dt >= 0 ? hit(dt, 0, 0.6) : 0;
      if (gl > 0.02) {
        ctx.save();
        ctx.strokeStyle = rgba(col, gl);
        ctx.lineWidth = 2.5;
        ctx.shadowColor = col;
        ctx.shadowBlur = 24 * gl;
        rr(ctx, r.x, r.y, r.w, r.h, 8 * r.z);
        ctx.stroke();
        ctx.restore();
      }
    });
    ctx.restore();

    const px = 70 - (1 - inK) * 300, py = 330, pw = 700, ph = 640;
    const a = ep(t, ag.in - 0.1, 0.5) * outA;
    const o = E.panel(ctx, px, py, pw, ph, { title: "agent", alpha: a });
    ctx.save();
    ctx.globalAlpha = a;
    ctx.beginPath();
    ctx.rect(px, py + 41, pw, ph - 42);
    ctx.clip();
    // The plan the agent wrote, then the one command that applies it.
    const plan = JSON.parse(data.cli.agent_batch).map((op) => {
      const dep = op.depends_on ? `, "depends_on": [${op.depends_on.map((d) => `"${d}"`).join(", ")}]` : "";
      return `{"op": "add", "title": "${op.title}"${op.kind ? `, "kind": "${op.kind}"` : ""}${dep}}`;
    });
    termLine(ctx, o.x, o.y + 2, "plan.json", 19, C.faint);
    plan.forEach((line, i) => {
      const max = 53;
      termLine(ctx, o.x, o.y + 40 + i * 34, line.length > max ? line.slice(0, max - 1) + "…" : line, 19.5, i === 0 ? rgba(C.amber, 0.85) : C.muted);
    });
    const y0 = o.y + 40 + plan.length * 34 + 34;
    ctx.fillStyle = "rgba(255,255,255,0.08)";
    ctx.fillRect(px, y0 - 30, pw, 1);
    const cmd = "topo apply plan.json";
    const typed = Math.floor(prog(t, ag.in + 0.06, 0.4) * cmd.length);
    const wx = termLine(ctx, o.x, y0 + 8, "$ ", 26, C.green);
    termLine(ctx, o.x + wx, y0 + 8, cmd.slice(0, typed), 26);
    data.cli.apply.trim().split("\n").forEach((line, i) => {
      if (t >= ag.enter + 0.08 + i * 0.07) termLine(ctx, o.x, y0 + 54 + i * 36, line, 23, C.green);
    });
    ctx.restore();
  }

  // ---- composition --------------------------------------------------------------------------------
  function product(ctx, t) {
    // A faint grid behind everything keeps the product's texture on screen.
    E.dotGrid(ctx, 0, 0, 1.6, 0.5 * ep(t, T.connect - 0.4, 1.0), "#1b1d24");

    // The main window. The video's own camera hardly moves: it goes in once when the app
    // pulls back to the path and slides aside for the terminals. Every other move is the
    // app's own camera, captured frame by frame. `x`, `y` is where the middle of the app's
    // canvas (content point 540, 428) sits, at `s` frame pixels per point.
    if (t < 36.35) {
      const q = PV.seq;
      const inK = ep(t, T.connect - 0.4, 2.0);
      const u1 = PV.swoop(prog(t, q.y.t0, q.y.d));
      const u2 = PV.swoop(prog(t, q.w.t0, q.w.d));
      const f = syncFocus();
      // A slow push for as long as a framing is held.
      const s0 = 1.2 * lerp(1, 1.03, prog(t, T.connect, 4));
      const s1 = 1.5 * lerp(1, 1.04, prog(t, q.y.t0, 16));
      const sc = s0 * Math.pow(s1 / s0, u1);
      const cam = {
        s: sc,
        x: lerp(lerp(744, 860, u1), 560 + (540 - f.x) * sc, u2),
        y: lerp(lerp(788, 668, u1), 650 + (428 - f.y) * sc, u2),
      };
      const r = { x: cam.x - 540 * cam.s, y: cam.y - 460 * cam.s + (1 - inK) * 420, w: 1440 * cam.s };
      const outK = ep(t, T.files - 0.2, 0.5, ease.inCubic);
      const w = E.win(r.x - outK * 420, r.y, r.w);
      ctx.save();
      ctx.globalAlpha = ep(t, T.connect - 0.4, 0.7) * (1 - outK);

      const flash = hit(t, PV.sync.flash, 0.35) * (t >= PV.sync.flash ? 1 : 0);
      const name = guiShot(t);
      if (name) {
        w.draw(ctx, name);
      } else {
        const st = readyState(t);
        w.draw(ctx, st.base);
        for (const p of st.patches) {
          const n = E.nodeOf(p.shot, p.title), m = E.meta(p.shot);
          w.patch(ctx, p.shot, [n.x - 3, n.y - 3, m.node[0] + 6, m.node[1] + 6]);
        }
      }
      if (t < q.y.t0 + 0.3) connectOverlay(ctx, t, w);
      if (t >= q.y.t0 + 0.9 && t < q.x.t0) critOverlay(ctx, t, w, name);
      if (t >= T.ready && t < q.w.t0) readyOverlay(ctx, t, w, name || "r_0");
      if (t >= T.sync) {
        if (flash > 0.02) {
          ctx.save();
          ctx.strokeStyle = rgba(C.green, flash);
          ctx.lineWidth = 3;
          ctx.shadowColor = C.green;
          ctx.shadowBlur = 30 * flash;
          rr(ctx, w.x, w.y, w.w, w.h, 11 * w.s);
          ctx.stroke();
          ctx.restore();
          // The task the CLI just finished, and what it freed.
          for (const title of ["Build API", "Write docs"]) {
            const n = w.node("e_sync", title);
            E.ringBurst(ctx, n.x, n.y, n.w, n.h, 8 * n.z, t - PV.sync.flash, C.green, { grow: 30, width: 3 });
          }
        }
        chip(ctx, "GUI", 162, 262, ep(t, T.sync + 0.1, 0.6) * (1 - outK));
        // The edit travels from the command line to the other two surfaces.
        const n = w.node("e_sync", "Build API");
        const from = { x: 1090, y: 760 };
        const u = prog(t, PV.sync.flash - 0.14, 0.14);
        E.comet(ctx, { p0: from, p1: { x: 900, y: 760 }, p2: { x: n.x + n.w / 2, y: n.y + n.h + 160 }, p3: { x: n.x + n.w / 2, y: n.y + n.h } }, u, C.green, 7, 0.5);
        E.comet(ctx, { p0: { x: 1480, y: 668 }, p1: { x: 1480, y: 650 }, p2: { x: 1440, y: 640 }, p3: { x: 1440, y: 618 } }, u, C.green, 7, 0.5);
      }
      ctx.restore();
    }

    if (t >= PV.seq.w.t0 && t < T.files + 0.3) {
      // Darken the right side so the terminal panels sit on a calm ground.
      const side = ctx.createLinearGradient(800, 0, 1150, 0);
      side.addColorStop(0, rgba(C.bg, 0));
      side.addColorStop(1, rgba(C.bg, 0.94));
      ctx.save();
      ctx.globalAlpha = ep(t, T.sync - 0.4, 0.5) * (1 - ep(t, T.files, 0.3));
      ctx.fillStyle = side;
      ctx.fillRect(800, 0, W - 800, H);
      ctx.restore();
      syncPanels(ctx, t);
    }
    if (t >= T.files && t < T.agents + 0.1) filesScene(ctx, t);
    if (t >= T.agents) agentScene(ctx, t);

    // Fade the top into the background so the headline always reads over the window.
    const scrim = ctx.createLinearGradient(0, 0, 0, 330);
    scrim.addColorStop(0, C.bg);
    scrim.addColorStop(0.64, rgba(C.bg, 0.96));
    scrim.addColorStop(1, rgba(C.bg, 0));
    const scrimA = PV.swoop(prog(t, PV.seq.y.t0, PV.seq.y.d * 0.6)) * (1 - ep(t, T.files + 0.3, 0.3));
    if (scrimA > 0) {
      ctx.save();
      ctx.globalAlpha = scrimA;
      ctx.fillStyle = scrim;
      ctx.fillRect(0, 0, W, 330);
      ctx.restore();
    }
    captions(ctx, t);
  }

  PV.scenes = PV.scenes || {};
  PV.scenes.product = product;
})(typeof window !== "undefined" ? window : globalThis);
