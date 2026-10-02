// 0–16 s: the hook (a list that cannot tell you where to start), the reveal
// (it was a graph all along) and the title.
(function (g) {
  const PV = g.PV;
  const E = PV.E;
  const { W, H, C, clamp, lerp, prog, ease, ep, hit, rgba, font, text, rr } = E;
  const T = PV.T;

  let layer = null;
  const getLayer = () => {
    if (!layer) layer = new OffscreenCanvas(W, H);
    const c = layer.getContext("2d");
    c.setTransform(1, 0, 0, 1, 0, 0);
    c.clearRect(0, 0, W, H);
    return c;
  };

  // ---- hook -------------------------------------------------------------------
  const RH = 150;
  // Row index → rows it must wait for, by index in PV.LIST: the order hidden in the list.
  const ARCS = [[9, 3], [9, 6], [9, 8], [3, 7], [6, 1], [8, 1], [1, 2], [1, 5], [7, 5], [4, 5], [2, 0], [5, 0]];

  function listRow(c, x, y, s, title, a, pop, dim, mark) {
    if (a <= 0) return;
    c.save();
    c.globalAlpha *= a * dim;
    c.translate(x + (1 - a) * 140 * s, y);
    c.scale(1 + pop * 0.1, 1 + pop * 0.1);
    const b = 62 * s;
    rr(c, 0, -b / 2, b, b, 14 * s);
    c.lineWidth = 5 * s;
    c.strokeStyle = mark > 0 ? rgba(C.accent, 0.5 + 0.5 * mark) : rgba(C.muted, 0.9);
    c.stroke();
    if (pop > 0.02) {
      c.fillStyle = rgba("#ffffff", pop * 0.5);
      c.fill();
    }
    text(c, title, b + 34 * s, 2 * s, { f: font(700, 92 * s), color: C.text, base: "middle", ls: -2.2 * s });
    c.restore();
  }

  function hook(ctx, t) {
    const n = PV.rowTimes.filter((rt) => rt <= t).length;
    let nS = 0;
    for (const rt of PV.rowTimes) nS += ep(t, rt, 0.8);
    nS = Math.max(1, nS);
    const flood = PV.floodTimes.filter((ft) => ft <= t).length;
    const frozen = Math.min(t, T.stop);

    // Everything shrinks as the pile grows; the last half second it pulls back further.
    // After the stop the pile keeps drifting back slowly, so nothing ever sits still.
    const s = clamp(760 / (nS * RH), 0, 1) * lerp(1, 0.94, ep(frozen, 3.4, 0.6, ease.outCubic)) * lerp(1, 0.93, ep(t, T.stop, 3.2, ease.outCubic));
    const dim = t < T.stop ? 1 : lerp(lerp(1, 0.2, ep(t, T.stop, 0.5)), 0.95, ep(t, T.tense - 0.15, 0.9));
    const blur = t < T.stop ? 0 : lerp(lerp(0, 7, ep(t, T.stop, 0.6)), 0, ep(t, T.tense - 0.15, 0.8));
    const suck = ease.inExpo(prog(t, 7.0, 0.82));
    const shake = Math.sin(t * 95) * 16 * hit(t, T.stop, 0.1);

    // The count of tasks, huge, behind everything.
    const count = n + flood;
    const cPop = t < T.stop ? Math.max(...PV.rowTimes.concat(PV.floodTimes).map((rt) => hit(t, rt, 0.07))) : 0;
    ctx.save();
    ctx.globalAlpha = (0.05 + 0.04 * cPop) * (1 - ep(t, T.stop, 0.2)) + 0;
    text(ctx, String(count).padStart(2, "0"), W - 50 + shake, H - 30, { f: font(900, 760), color: "#fff", align: "right", ls: -40 });
    ctx.restore();

    const c = getLayer();
    c.save();
    c.translate(W / 2 + shake, H / 2);
    c.scale(1 - suck, 1 - suck);
    c.rotate(suck * 0.5);
    c.translate(-W / 2, -H / 2);
    const x0 = W / 2 - 440 * s;
    const yOf = (i) => H / 2 + (i - (nS - 1) / 2) * RH * s;

    // The pile overflowing on both sides.
    const floodA = 1 - ep(t, T.tense, 0.5, ease.outCubic);
    PV.FLOOD.forEach((title, i) => {
      const ft = PV.floodTimes[i];
      if (t < ft || floodA <= 0) return;
      const left = i % 2 === 0;
      const row = Math.floor(i / 2);
      const x = left ? x0 - 610 : x0 + 1010 * s + 250;
      const y = H / 2 + (row - 6) * RH * s;
      listRow(c, x, y, s, title, ep(frozen, ft, 0.4) * floodA, hit(frozen, ft, 0.05), 0.42, 0);
    });

    // Hidden order: arcs flicker between the rows that depend on each other.
    const arcOn = (k) => PV.heart[Math.floor(k / 2)];
    const marks = new Array(PV.LIST.length).fill(0);
    ARCS.forEach(([a, b], k) => {
      const t0 = arcOn(k);
      if (t < t0) return;
      const d = ep(t, t0, 0.7);
      marks[a] = Math.max(marks[a], hit(t, t0, 0.3));
      marks[b] = Math.max(marks[b], hit(t, t0 + 0.1, 0.3));
      const ya = yOf(a), yb = yOf(b);
      const bulge = (70 + Math.abs(a - b) * 34) * s;
      const xa = x0 - 16 * s;
      c.save();
      c.strokeStyle = rgba(C.accent, 0.45 + 0.55 * hit(t, t0, 0.25));
      c.lineWidth = 4 * s + 3 * hit(t, t0, 0.15);
      c.lineCap = "round";
      c.shadowColor = C.accent;
      c.shadowBlur = 24 * hit(t, t0, 0.3);
      const curve = { p0: { x: xa, y: ya }, p1: { x: xa - bulge, y: ya }, p2: { x: xa - bulge, y: yb }, p3: { x: xa, y: yb } };
      E.strokeCurve(c, curve, 0, d);
      c.restore();
    });

    PV.LIST.forEach((title, i) => {
      const rt = PV.rowTimes[i];
      if (t < rt) return;
      const jitter = t >= T.tense ? Math.sin(t * 60 + i * 2.1) * 9 * Math.max(...PV.heart.map((h) => hit(t, h, 0.09))) : 0;
      listRow(c, x0 + jitter, yOf(i), s, title, ep(frozen, rt, 0.6), hit(frozen, rt, 0.06), 1, marks[i]);
    });
    c.restore();

    ctx.save();
    if (blur > 0.1) ctx.filter = `blur(${blur.toFixed(2)}px)`;
    ctx.globalAlpha = dim * (1 - ease.inQuad(prog(t, 7.6, 0.25)));
    ctx.drawImage(layer, 0, 0);
    ctx.restore();

    // The stop: one number, one question.
    if (t >= T.stop && t < T.tense + 0.4) {
      const lt = t - T.stop;
      const out = T.tense - T.stop;
      const up = ep(t, 4.72, 1.5);
      const a = ep(t, T.stop, 0.08) * (1 - ep(t, T.tense - 0.12, 0.5, ease.inCubic));
      const sc = lerp(1.3, 1, ep(t, T.stop, 1.6)) * lerp(1, 0.62, up) * lerp(1, 1.05, prog(t, T.stop, 2.4));
      ctx.save();
      ctx.globalAlpha = a;
      ctx.translate(W / 2 + shake, lerp(540, 395, up) - ep(t, T.tense - 0.12, 0.5, ease.inCubic) * 90);
      ctx.scale(sc, sc);
      text(ctx, `${PV.LIST.length + PV.FLOOD.length} tasks.`, 0, 0, { f: font(800, 250), align: "center", base: "middle", ls: -9 });
      ctx.restore();
      E.riseText(ctx, "Where do you start?", W / 2, 690, lt, {
        size: 150, weight: 800, align: "center", out: out - 0.14, dur: 1.0,
        times: PV.words.map((w) => w.t - T.stop),
        colors: [C.text, C.text, C.text, C.green],
      });
    }

    // What is left: a single point.
    if (t > 7.4) {
      const a = ep(t, 7.4, 0.3);
      ctx.save();
      ctx.shadowColor = C.green;
      ctx.shadowBlur = 40;
      ctx.fillStyle = "#fff";
      ctx.globalAlpha = a;
      ctx.beginPath();
      ctx.arc(W / 2, H / 2, 7 * a * (1 - ep(t, 7.93, 0.07, ease.inCubic) * 0.6), 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();
    }
  }

  // ---- the graph ------------------------------------------------------------------
  const DEPS = {
    "Design UI": ["Define requirements"],
    "Design DB schema": ["Define requirements"],
    "Write auth spec": ["Define requirements"],
    "Build frontend": ["Design UI"],
    "Build API": ["Design DB schema", "Write auth spec"],
    "Write docs": ["Build API"],
    "Integration tests": ["Build API", "Build frontend", "Set up CI"],
  };
  const MEMBERS = ["Write docs", "Integration tests"];
  let hero = null;
  function heroGraph() {
    if (hero) return hero;
    const nodes = E.meta("e_init").nodes.map((n) => ({
      title: n.title,
      kind: n.kind.toLowerCase(),
      status: n.status.toLowerCase(),
      col: n.col,
      row: n.row,
      x: n.col * E.CELL_W,
      y: n.row * E.CELL_H,
    }));
    const by = Object.fromEntries(nodes.map((n) => [n.title, n]));
    for (const n of nodes) {
      const open = (DEPS[n.title] || []).filter((d) => by[d].status !== "done").length;
      if (n.kind === "milestone") {
        n.done = 0;
        n.total = MEMBERS.length;
      } else {
        n.chip = n.status === "done" ? "Done" : open === 0 ? "Ready" : `Blocked by ${open}`;
      }
    }
    const edges = [];
    for (const [to, froms] of Object.entries(DEPS)) for (const from of froms) edges.push({ from: by[from], to: by[to], member: false });
    for (const m of MEMBERS) edges.push({ from: by[m], to: by["v1.0 Release"], member: true });
    edges.sort((a, b) => a.from.col - b.from.col || a.to.row - b.to.row || a.from.row - b.from.row);
    edges.forEach((e, k) => (e.t0 = T.graph + k * PV.S16));
    hero = { nodes, edges, cx: (4 * E.CELL_W + E.NODE_W) / 2, cy: (2 * E.CELL_H + E.NODE_H) / 2 };
    return hero;
  }

  function graphLayer(c, t) {
    const G = heroGraph();
    const z = lerp(1.2, 1.3, prog(t, T.drop, 8)) * lerp(1, 1.12, ep(t, T.title - 0.1, 1.8));
    const fy = lerp(700, 560, ep(t, T.title - 0.1, 1.8));
    const ox = W / 2 - G.cx * z, oy = fy - G.cy * z;

    E.dotGrid(c, ox, oy, z, ep(t, T.drop, 1.4), "#2a2d37");

    // Shockwave from the point the list collapsed into.
    const sw = prog(t, T.drop, 0.9);
    if (sw > 0 && sw < 1) {
      c.save();
      c.strokeStyle = rgba("#ffffff", (1 - sw) * 0.5);
      c.lineWidth = 6 * (1 - sw) + 1;
      c.beginPath();
      c.arc(W / 2, H / 2, ease.outCubic(sw) * 1300, 0, Math.PI * 2);
      c.stroke();
      c.restore();
    }

    const at = (n) => {
      const t0 = T.drop + n.col * 0.04 + n.row * 0.025;
      const k = ease.spring(prog(t, t0, 1.25), 5.2, 8.5);
      return { x: lerp(W / 2 - (E.NODE_W * z) / 2, ox + n.x * z, k), y: lerp(H / 2 - (E.NODE_H * z) / 2, oy + n.y * z, k), a: ep(t, t0, 0.12) };
    };
    const pos = new Map(G.nodes.map((n) => [n, at(n)]));

    for (const e of G.edges) {
      if (t < e.t0) continue;
      const a = pos.get(e.from), b = pos.get(e.to);
      const curve = E.edgeCurve({ x: a.x + E.NODE_W * z, y: a.y + (E.NODE_H * z) / 2 }, { x: b.x, y: b.y + (E.NODE_H * z) / 2 }, z);
      const d = ep(t, e.t0, 0.75);
      const fresh = 1 - ep(t, e.t0 + 0.2, 1.3, ease.outCubic);
      const base = e.member ? C.amber : C.edge;
      E.edge(c, curve, { color: base, width: 1.6 * z, dashed: e.member, draw: d, alpha: e.member ? 0.6 : 1 });
      if (fresh > 0) E.edge(c, curve, { color: e.member ? C.amber : C.accent, width: 2.6 * z, dashed: e.member, draw: d, alpha: fresh });
      if (d < 1) E.comet(c, curve, d, e.member ? C.amber : C.accent, 5 * z);
    }

    for (const n of G.nodes) {
      const p = pos.get(n);
      const metaT = 11.5 + n.col * 0.06;
      const m = ep(t, metaT - 0.15, 0.9);
      const ready = n.chip === "Ready";
      const arrive = Math.max(0, ...G.edges.filter((e) => e.to === n).map((e) => hit(t, e.t0 + 0.2, 0.4) * (t >= e.t0 + 0.2 ? 1 : 0)));
      const glow = ready ? m * 0.5 : 0;
      E.card(c, p.x, p.y, z, n, {
        alpha: p.a,
        meta: m,
        glow: ready ? C.green : C.accent,
        glowAmt: Math.max(glow, arrive * 0.8),
        border: arrive > 0.05 ? rgba(C.accent, arrive) : ready && m > 0 ? rgba(C.green, 0.55 * m) : null,
        noFade: m < 0.5,
      });
    }
  }

  function reveal(ctx, t) {
    const titleP = ep(t, T.title - 0.12, 1.3);
    const c = getLayer();
    graphLayer(c, t);
    ctx.save();
    const blur = titleP * 6;
    if (blur > 0.1) ctx.filter = `blur(${blur.toFixed(2)}px)`;
    ctx.globalAlpha = lerp(1, 0.3, titleP) * (1 - ep(t, 15.25, 0.6, ease.inCubic));
    ctx.drawImage(layer, 0, 0);
    ctx.restore();

    // Flash of the drop.
    const fl = hit(t, T.drop, 0.07);
    if (fl > 0.01) {
      ctx.fillStyle = rgba("#ffffff", fl * 0.4);
      ctx.fillRect(0, 0, W, H);
    }

    if (t < T.title + 0.3) {
      E.riseText(ctx, "Not a list.", W / 2, 370, t - T.drop, { size: 230, weight: 800, align: "center", out: T.graph - T.drop - 0.32, dur: 1.0, stagger: 0.07 });
      E.riseText(ctx, "It's a graph.", W / 2, 370, t - T.graph, {
        size: 230, weight: 800, align: "center", out: T.title - T.graph - 0.36, dur: 1.0, stagger: 0.07,
        colors: [C.text, C.text, C.accent],
      });
    }
  }

  function title(ctx, t) {
    const lt = t - T.title;
    const out = ep(t, 15.25, 0.6, ease.inCubic);
    const fl = hit(t, T.title, 0.08);
    if (fl > 0.01) {
      ctx.fillStyle = rgba("#ffffff", fl * 0.28);
      ctx.fillRect(0, 0, W, H);
    }
    ctx.save();
    ctx.globalAlpha = 1 - out;
    const drift = lerp(1, 1.045, prog(t, T.title, 4));
    ctx.translate(W / 2, 540 - out * 90);
    ctx.scale(drift, drift);
    ctx.translate(-W / 2, -540);

    const size = 330;
    const f = font(800, size);
    const ls = -12;
    const wordW = E.measure(ctx, "topo", f, ls);
    const markW = 250;
    const total = markW + 40 + wordW;
    const x0 = W / 2 - total / 2;
    const baseY = 560;
    const open = 1 - ep(t, T.title, 1.3);
    const pop = lerp(1.2, 1, ep(t, T.title, 1.5));
    ctx.save();
    ctx.translate(x0 + markW / 2, baseY - 118);
    ctx.scale(pop, pop);
    E.logoMark(ctx, 0, 0, 440, { open: open * 0.5, alpha: ep(t, T.title, 0.1) });
    ctx.restore();
    // Letters rise one by one.
    let lx = x0 + markW + 40;
    "topo".split("").forEach((ch, i) => {
      const w = E.measure(ctx, ch, f, ls);
      const a = ep(lt, 0.05 + i * 0.07, 1.1);
      ctx.save();
      ctx.beginPath();
      ctx.rect(lx - 20, baseY - size, w + 40, size * 1.3);
      ctx.clip();
      text(ctx, ch, lx, baseY + (1 - a) * size * 1.1, { f, ls });
      ctx.restore();
      lx += w;
    });

    // Kicker types on.
    const ka = ep(t, 12.9, 1.0);
    text(ctx, "topo", W / 2, 664 + (1 - ka) * 22, { f: font(400, 36), color: C.muted, align: "center", ls: 0.5, alpha: ka });

    E.riseText(ctx, "Do things in the right order.", W / 2, 830, t - 14, {
      size: 84, weight: 700, align: "center", dur: 1.0, stagger: 0.08,
      colors: [C.text, C.text, C.text, C.text, C.green, C.green],
    });
    ctx.restore();
  }

  PV.scenes = PV.scenes || {};
  Object.assign(PV.scenes, { hook, reveal, title, heroGraph });
})(typeof window !== "undefined" ? window : globalThis);
