// Drawing toolkit of the promo video: easing, type, the product's look, real
// screenshots, and the post pass. Every frame is a pure function of time.
(function (g) {
  const PV = g.PV;
  const W = 1920;
  const H = 1080;

  // ---- math ---------------------------------------------------------------
  const clamp = (x, a = 0, b = 1) => Math.min(b, Math.max(a, x));
  const lerp = (a, b, t) => a + (b - a) * t;
  /** Progress 0..1 of `t` through [t0, t0 + dur]. */
  const prog = (t, t0, dur) => clamp((t - t0) / dur);
  const ease = {
    linear: (x) => x,
    inQuad: (x) => x * x,
    outQuad: (x) => 1 - (1 - x) * (1 - x),
    inCubic: (x) => x * x * x,
    outCubic: (x) => 1 - Math.pow(1 - x, 3),
    inOutCubic: (x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2),
    outQuart: (x) => 1 - Math.pow(1 - x, 4),
    inQuart: (x) => x * x * x * x,
    inOutQuart: (x) => (x < 0.5 ? 8 * x * x * x * x : 1 - Math.pow(-2 * x + 2, 4) / 2),
    outQuint: (x) => 1 - Math.pow(1 - x, 5),
    outExpo: (x) => (x >= 1 ? 1 : 1 - Math.pow(2, -10 * x)),
    inExpo: (x) => (x <= 0 ? 0 : Math.pow(2, 10 * x - 10)),
    inOutExpo: (x) =>
      x <= 0 ? 0 : x >= 1 ? 1 : x < 0.5 ? Math.pow(2, 20 * x - 10) / 2 : (2 - Math.pow(2, -20 * x + 10)) / 2,
    outBack: (x, s = 1.70158) => 1 + (s + 1) * Math.pow(x - 1, 3) + s * Math.pow(x - 1, 2),
    /** Underdamped spring settling at 1. */
    spring: (x, damping = 5.5, freq = 11) => (x >= 1 ? 1 : 1 - Math.exp(-damping * x) * Math.cos(freq * x)),
  };
  /** Eased progress. */
  const ep = (t, t0, dur, fn = ease.outExpo) => fn(prog(t, t0, dur));
  /** 1 at `t0`, decaying to 0: the shape of a hit. */
  const hit = (t, t0, decay = 0.25) => (t < t0 ? 0 : Math.exp(-(t - t0) / decay));
  /** Strength of the kick pulse at `t` for a four-on-the-floor section. */
  const beatPulse = (t, decay = 0.12) => Math.exp(-((t % PV.BEAT) + 1e-9) / decay);

  // ---- palette (the app's theme) ----------------------------------------------
  const C = {
    bg: "#0b0c10",
    canvas: "#111216",
    surface: "#17181d",
    card: "#1e1f26",
    raised: "#2a2c36",
    border: "#2b2d37",
    borderStrong: "#3b3e4a",
    text: "#e8e9ee",
    muted: "#9b9dab",
    faint: "#626574",
    accent: "#6ea8fe",
    green: "#4cc38a",
    amber: "#f5b949",
    red: "#f2555a",
    gridDot: "#24262e",
    edge: "#565a68",
  };
  const rgba = (hex, a) => {
    const n = parseInt(hex.slice(1), 16);
    return `rgba(${(n >> 16) & 255},${(n >> 8) & 255},${n & 255},${a})`;
  };
  const SANS = 'system-ui, -apple-system, "SF Pro Display", "Helvetica Neue", sans-serif';
  const MONO = '"SF Mono", ui-monospace, "UDEV Gothic", Menlo, monospace';
  const font = (weight, size, family = SANS) => `${weight} ${size}px ${family}`;

  // ---- primitives ---------------------------------------------------------------
  function rr(ctx, x, y, w, h, r) {
    ctx.beginPath();
    ctx.roundRect(x, y, w, h, Math.max(0, Math.min(r, w / 2, h / 2)));
  }

  function text(ctx, str, x, y, { f, color = C.text, align = "left", base = "alphabetic", ls = 0, alpha = 1 } = {}) {
    if (alpha <= 0) return 0;
    ctx.save();
    ctx.font = f;
    ctx.letterSpacing = `${ls}px`;
    ctx.textAlign = align;
    ctx.textBaseline = base;
    ctx.fillStyle = color;
    ctx.globalAlpha *= alpha;
    ctx.fillText(str, x, y);
    const w = ctx.measureText(str).width;
    ctx.restore();
    return w;
  }

  function measure(ctx, str, f, ls = 0) {
    ctx.save();
    ctx.font = f;
    ctx.letterSpacing = `${ls}px`;
    const w = ctx.measureText(str).width;
    ctx.restore();
    return w;
  }

  /**
   * Words rising from behind their baseline, one after another.
   * `t` is the time since the line started; `out` (optional) the time it leaves.
   */
  function riseText(ctx, str, x, y, t, o = {}) {
    const { size = 80, weight = 800, color = C.text, ls = -0.02 * (o.size || 80), stagger = 0.06, dur = 0.55, align = "left", out = Infinity, family = SANS, colors = null, times = null } = o;
    const f = font(weight, size, family);
    const words = str.split(" ");
    const space = measure(ctx, " ", f, ls);
    const widths = words.map((w) => measure(ctx, w, f, ls));
    const total = widths.reduce((a, b) => a + b, 0) + space * (words.length - 1);
    let cx = align === "center" ? x - total / 2 : align === "right" ? x - total : x;
    words.forEach((w, i) => {
      const t0 = times ? times[i] : i * stagger;
      const a = ease.outExpo(prog(t, t0, dur));
      const b = ease.inCubic(prog(t, out + i * 0.03, 0.3));
      if (t >= t0 && b < 1) {
        ctx.save();
        ctx.beginPath();
        ctx.rect(cx - size * 0.2, y - size * 1.02, widths[i] + size * 0.4, size * 1.32);
        ctx.clip();
        text(ctx, w, cx, y + (1 - a) * size * 1.15 - b * size * 1.15, { f, color: colors ? colors[i] || color : color, ls });
        ctx.restore();
      }
      cx += widths[i] + space;
    });
    return total;
  }

  /** The product's dot grid in world space, under camera (`ox`, `oy`, `z`). */
  function dotGrid(ctx, ox, oy, z, alpha = 1, color = C.gridDot) {
    if (alpha <= 0) return;
    let step = 28 * z;
    while (step < 18) step *= 2;
    ctx.save();
    ctx.globalAlpha *= alpha;
    ctx.fillStyle = color;
    const sx = ((ox % step) + step) % step;
    const sy = ((oy % step) + step) % step;
    const d = z > 1.2 ? 2.4 : 1.9;
    for (let y = sy; y < H; y += step) for (let x = sx; x < W; x += step) ctx.fillRect(x, y, d, d);
    ctx.restore();
  }

  // ---- the product's cards and edges, redrawn -------------------------------------
  const NODE_W = 232;
  const NODE_H = 66;
  const CELL_W = 280;
  const CELL_H = 92;

  function statusIcon(ctx, cx, cy, z, kind, status) {
    ctx.save();
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    if (kind === "milestone") {
      ctx.fillStyle = status === "done" ? C.green : C.amber;
      ctx.beginPath();
      ctx.moveTo(cx, cy - 6.2 * z);
      ctx.lineTo(cx + 6.2 * z, cy);
      ctx.lineTo(cx, cy + 6.2 * z);
      ctx.lineTo(cx - 6.2 * z, cy);
      ctx.closePath();
      ctx.fill();
    } else if (status === "done") {
      ctx.strokeStyle = C.green;
      ctx.lineWidth = 1.6 * z;
      ctx.beginPath();
      ctx.moveTo(cx - 4.6 * z, cy + 0.4 * z);
      ctx.lineTo(cx - 1.4 * z, cy + 3.6 * z);
      ctx.lineTo(cx + 4.8 * z, cy - 3.8 * z);
      ctx.stroke();
    } else {
      ctx.strokeStyle = status === "doing" ? C.accent : C.muted;
      ctx.lineWidth = 1.4 * z;
      ctx.beginPath();
      ctx.arc(cx, cy, 5 * z, 0, Math.PI * 2);
      ctx.stroke();
      if (status === "doing") {
        ctx.fillStyle = C.accent;
        ctx.beginPath();
        ctx.arc(cx, cy, 5 * z, Math.PI / 2, (Math.PI * 3) / 2);
        ctx.fill();
      }
    }
    ctx.restore();
  }

  /**
   * A node card as the app draws it. `n`: {title, kind, status, chip, done, total}.
   * `o`: {alpha, meta (0..1 reveal of the second row), glow (color), glowAmt, border, lift}.
   */
  function card(ctx, x, y, z, n, o = {}) {
    const { alpha = 1, meta = 1, glow = null, glowAmt = 0, border = null, borderW = 1 } = o;
    if (alpha <= 0) return;
    const ms = n.kind === "milestone";
    const closed = n.status === "done";
    const w = NODE_W * z, h = NODE_H * z;
    ctx.save();
    ctx.globalAlpha *= alpha * (closed && !o.noFade ? 0.62 : 1);
    if (glow && glowAmt > 0) {
      ctx.save();
      ctx.shadowColor = rgba(glow, 0.9 * glowAmt);
      ctx.shadowBlur = 34 * z * glowAmt;
      rr(ctx, x, y, w, h, (ms ? 14 : 8) * z);
      ctx.fillStyle = ms ? "#221f1b" : C.card;
      ctx.fill();
      ctx.restore();
    }
    rr(ctx, x, y, w, h, (ms ? 14 : 8) * z);
    ctx.fillStyle = ms ? "#221f1b" : C.card;
    ctx.fill();
    ctx.lineWidth = Math.max(1, borderW * z);
    ctx.strokeStyle = border || (ms ? rgba(C.amber, 0.45) : C.border);
    ctx.stroke();
    // With the meta row hidden the title sits in the middle, as in the app when zoomed far out.
    const rowY = y + lerp(h / 2, 22.5 * z, meta);
    statusIcon(ctx, x + 19 * z, rowY, z, n.kind, n.status);
    const tf = font(ms ? 600 : 400, 13 * z);
    const tx = x + 34 * z;
    const tw = text(ctx, n.title, tx, rowY + 0.5 * z, { f: tf, color: closed ? C.muted : C.text, base: "middle" });
    if (closed) {
      ctx.fillStyle = C.muted;
      ctx.fillRect(tx, rowY + 0.5 * z, tw, Math.max(1, z));
    }
    if (meta > 0.01) {
      ctx.globalAlpha *= meta;
      const my = y + 47 * z;
      const mf = font(400, 10.5 * z);
      if (ms) {
        const bw = w - 34 * z - 12 * z - 30 * z;
        rr(ctx, tx, my - 2 * z, bw, 4 * z, 2 * z);
        ctx.fillStyle = C.border;
        ctx.fill();
        const frac = n.total ? n.done / n.total : 0;
        if (frac > 0) {
          rr(ctx, tx, my - 2 * z, bw * frac, 4 * z, 2 * z);
          ctx.fillStyle = frac >= 1 ? C.green : C.amber;
          ctx.fill();
        }
        text(ctx, `${n.done}/${n.total}`, x + w - 12 * z, my + 0.5 * z, { f: mf, color: C.muted, align: "right", base: "middle" });
      } else if (n.chip) {
        const tint = { Ready: C.green, Done: C.green, "In progress": C.accent }[n.chip];
        if (tint) {
          const cw = measure(ctx, n.chip, mf) + 10 * z;
          rr(ctx, tx, my - 7.5 * z, cw, 15 * z, 3 * z);
          ctx.fillStyle = rgba(tint, 0.15);
          ctx.fill();
          text(ctx, n.chip, tx + 5 * z, my + 0.5 * z, { f: mf, color: tint, base: "middle" });
        } else {
          text(ctx, n.chip, tx, my + 0.5 * z, { f: mf, color: C.faint, base: "middle" });
        }
      }
    }
    ctx.restore();
  }

  /** Bezier of an edge between two card anchors, as the app routes it. */
  function edgeCurve(from, to, z) {
    const head = clamp(7 * z, 4, 10);
    const end = { x: to.x - head, y: to.y };
    const dx = Math.max((end.x - from.x) / 2, 24 * z);
    return { p0: from, p1: { x: from.x + dx, y: from.y }, p2: { x: end.x - dx, y: end.y }, p3: end, head, tip: to };
  }
  function bez(c, u) {
    const v = 1 - u;
    const a = v * v * v, b = 3 * v * v * u, d = 3 * v * u * u, e = u * u * u;
    return { x: a * c.p0.x + b * c.p1.x + d * c.p2.x + e * c.p3.x, y: a * c.p0.y + b * c.p1.y + d * c.p2.y + e * c.p3.y };
  }
  /** Strokes the curve from parameter `u0` to `u1`. */
  function strokeCurve(ctx, c, u0, u1, steps = 28) {
    if (u1 <= u0) return;
    ctx.beginPath();
    for (let i = 0; i <= steps; i++) {
      const p = bez(c, lerp(u0, u1, i / steps));
      if (i === 0) ctx.moveTo(p.x, p.y);
      else ctx.lineTo(p.x, p.y);
    }
    ctx.stroke();
  }
  function edge(ctx, c, { color = C.edge, width = 1.5, dashed = false, draw = 1, alpha = 1, arrow = true } = {}) {
    if (draw <= 0 || alpha <= 0) return;
    ctx.save();
    ctx.globalAlpha *= alpha;
    ctx.strokeStyle = color;
    ctx.fillStyle = color;
    ctx.lineWidth = width;
    ctx.lineCap = "round";
    if (dashed) ctx.setLineDash([5 * width, 4 * width]);
    strokeCurve(ctx, c, 0, draw);
    if (arrow && draw >= 0.999) {
      ctx.setLineDash([]);
      ctx.beginPath();
      ctx.moveTo(c.tip.x, c.tip.y);
      ctx.lineTo(c.tip.x - c.head, c.tip.y - c.head / 2);
      ctx.lineTo(c.tip.x - c.head, c.tip.y + c.head / 2);
      ctx.closePath();
      ctx.fill();
    }
    ctx.restore();
  }
  /** A glowing comet at parameter `u` of the curve. */
  function comet(ctx, c, u, color, size = 5, tail = 0.22) {
    if (u <= 0 || u >= 1) return;
    ctx.save();
    ctx.lineCap = "round";
    for (let i = 0; i < 10; i++) {
      const a = u - (tail * (i + 1)) / 10, b = u - (tail * i) / 10;
      if (b <= 0) break;
      ctx.strokeStyle = rgba(color, 0.75 * (1 - i / 10));
      ctx.lineWidth = size * (1 - i / 13);
      strokeCurve(ctx, c, Math.max(0, a), b, 3);
    }
    const p = bez(c, u);
    ctx.shadowColor = color;
    ctx.shadowBlur = size * 5;
    ctx.fillStyle = "#fff";
    ctx.beginPath();
    ctx.arc(p.x, p.y, size * 0.75, 0, Math.PI * 2);
    ctx.fill();
    ctx.restore();
  }

  /** Expanding ring around a rounded rect: something happened here. */
  function ringBurst(ctx, x, y, w, h, r, t, color, { dur = 0.6, grow = 34, width = 3 } = {}) {
    if (t < 0 || t > dur) return;
    const p = ease.outCubic(t / dur);
    ctx.save();
    ctx.globalAlpha *= 1 - ease.inQuad(t / dur);
    ctx.strokeStyle = color;
    ctx.lineWidth = width * (1 - p * 0.6);
    ctx.shadowColor = color;
    ctx.shadowBlur = 18;
    rr(ctx, x - grow * p, y - grow * p, w + 2 * grow * p, h + 2 * grow * p, r + grow * p);
    ctx.stroke();
    ctx.restore();
  }

  function sparks(ctx, cx, cy, t, color, seed, { n = 12, dur = 0.55, dist = 120 } = {}) {
    if (t < 0 || t > dur) return;
    const r = PV.rng(seed);
    const p = ease.outQuart(t / dur);
    ctx.save();
    ctx.fillStyle = color;
    ctx.globalAlpha *= 1 - ease.inCubic(t / dur);
    for (let i = 0; i < n; i++) {
      const a = r() * Math.PI * 2, d = dist * (0.4 + 0.6 * r()) * p, s = 2 + r() * 4;
      ctx.fillRect(cx + Math.cos(a) * d - s / 2, cy + Math.sin(a) * d - s / 2, s, s);
    }
    ctx.restore();
  }

  /** The macOS arrow pointer with its tip at (x, y). */
  function cursor(ctx, x, y, s = 1.5, press = 0) {
    ctx.save();
    ctx.translate(x, y);
    ctx.scale(s * (1 - 0.12 * press), s * (1 - 0.12 * press));
    ctx.shadowColor = "rgba(0,0,0,0.5)";
    ctx.shadowBlur = 8;
    ctx.shadowOffsetY = 3;
    ctx.beginPath();
    ctx.moveTo(0, 0);
    ctx.lineTo(0, 17);
    ctx.lineTo(4.2, 13.2);
    ctx.lineTo(7.2, 20);
    ctx.lineTo(9.8, 18.9);
    ctx.lineTo(6.9, 12.2);
    ctx.lineTo(12.2, 12.2);
    ctx.closePath();
    ctx.fillStyle = "#fff";
    ctx.fill();
    ctx.shadowColor = "transparent";
    ctx.lineWidth = 1.1;
    ctx.strokeStyle = "#111";
    ctx.stroke();
    ctx.restore();
  }

  /** A key cap popping up while its key is pressed at `t0`. */
  function keycap(ctx, label, cx, cy, t, t0, { hold = 0.55, w = 0 } = {}) {
    const dt = t - t0;
    if (dt < -0.12 || dt > hold + 0.3) return;
    const a = ease.outExpo(prog(dt, -0.12, 0.2)) * (1 - ease.inCubic(prog(dt, hold, 0.3)));
    const press = hit(t, t0, 0.09);
    const f = font(600, 40, SANS);
    const kw = w || Math.max(104, measure(ctx, label, f) + 64);
    const kh = 92;
    ctx.save();
    ctx.globalAlpha *= a;
    ctx.translate(cx, cy + (1 - a) * 30 + press * 7);
    ctx.shadowColor = "rgba(0,0,0,0.6)";
    ctx.shadowBlur = 40;
    ctx.shadowOffsetY = 14;
    rr(ctx, -kw / 2, -kh / 2 + 8, kw, kh, 18);
    ctx.fillStyle = "#0a0b0e";
    ctx.fill();
    ctx.shadowColor = "transparent";
    rr(ctx, -kw / 2, -kh / 2 - 2 + press * 6, kw, kh, 18);
    const grad = ctx.createLinearGradient(0, -kh / 2, 0, kh / 2);
    grad.addColorStop(0, "#3a3d4a");
    grad.addColorStop(1, "#262832");
    ctx.fillStyle = grad;
    ctx.fill();
    ctx.lineWidth = 2;
    ctx.strokeStyle = rgba(C.accent, 0.35 + 0.65 * press);
    ctx.stroke();
    text(ctx, label, 0, -1 + press * 6, { f, color: C.text, align: "center", base: "middle" });
    ctx.restore();
  }

  // ---- real screenshots ---------------------------------------------------------------
  const SHOT_W = 2880;
  const SHOT_H = 1684;
  const TITLEBAR = 32;
  const WIN_PT_W = 1440;
  const A = { img: new Map(), pending: new Map(), data: null, base: "", preview: false, limit: 28, tick: 0, used: new Map() };

  async function loadData(base) {
    A.base = base;
    A.data = await (await fetch(base + "assets/data.json")).json();
  }
  function request(name) {
    if (A.img.has(name) || A.pending.has(name)) return;
    const p = fetch(`${A.base}assets/shots/${name}.png`)
      .then((r) => r.blob())
      .then((b) => createImageBitmap(b, A.preview ? { resizeWidth: 1440, resizeQuality: "high" } : {}))
      .then((bmp) => {
        A.img.set(name, bmp);
        A.pending.delete(name);
      });
    A.pending.set(name, p);
  }
  /** The decoded screenshot, or null while it loads (the frame is redrawn once it has). */
  function shot(name) {
    A.used.set(name, A.tick);
    const bmp = A.img.get(name);
    if (!bmp) request(name);
    return bmp || null;
  }
  async function settle() {
    while (A.pending.size) await Promise.all([...A.pending.values()]);
    if (!A.preview && A.img.size > A.limit) {
      const old = [...A.img.keys()].sort((a, b) => (A.used.get(a) || 0) - (A.used.get(b) || 0));
      for (const name of old.slice(0, A.img.size - A.limit)) {
        A.img.get(name).close();
        A.img.delete(name);
      }
    }
  }
  const meta = (name) => A.data.shots[name];
  const nodeOf = (name, title) => meta(name).nodes.find((n) => n.title === title);

  /**
   * A real app window on screen: `x`, `y` is its top-left, `w` its width in frame pixels.
   * Returns helpers to place overlays in the window's own (content point) coordinates.
   */
  function win(x, y, w) {
    const s = w / WIN_PT_W;
    const h = (SHOT_H / SHOT_W) * w;
    return {
      x, y, w, h, s,
      /** Content point → frame pixel. */
      pt: (px, py) => ({ x: x + px * s, y: y + (TITLEBAR + py) * s }),
      draw(ctx, name, { alpha = 1, shadow = 1 } = {}) {
        const bmp = shot(name);
        ctx.save();
        ctx.globalAlpha *= alpha;
        if (shadow > 0) {
          ctx.save();
          ctx.shadowColor = `rgba(0,0,0,${0.62 * shadow})`;
          ctx.shadowBlur = 90;
          ctx.shadowOffsetY = 36;
          rr(ctx, x, y, w, h, 11 * s);
          ctx.fillStyle = C.canvas;
          ctx.fill();
          ctx.restore();
        }
        if (bmp) {
          ctx.save();
          rr(ctx, x, y, w, h, 11 * s);
          ctx.clip();
          ctx.imageSmoothingQuality = "high";
          ctx.drawImage(bmp, x, y, w, h);
          ctx.restore();
        }
        rr(ctx, x + 0.5, y + 0.5, w - 1, h - 1, 11 * s);
        ctx.strokeStyle = "rgba(255,255,255,0.13)";
        ctx.lineWidth = 1;
        ctx.stroke();
        ctx.restore();
      },
      /** Draws only the content-point rect `r` = [x, y, w, h] of another screenshot over this window. */
      patch(ctx, name, r, alpha = 1) {
        const bmp = shot(name);
        if (!bmp || alpha <= 0) return;
        const k = bmp.width / WIN_PT_W;
        ctx.save();
        ctx.globalAlpha *= alpha;
        ctx.imageSmoothingQuality = "high";
        ctx.drawImage(bmp, r[0] * k, (r[1] + TITLEBAR) * k, r[2] * k, r[3] * k, x + r[0] * s, y + (r[1] + TITLEBAR) * s, r[2] * s, r[3] * s);
        ctx.restore();
      },
      /** Frame rect of a node card of shot `name`. */
      node(name, title) {
        const m = meta(name), n = nodeOf(name, title);
        const p = this.pt(n.x, n.y);
        return { x: p.x, y: p.y, w: m.node[0] * s, h: m.node[1] * s, z: m.zoom * s, ptRect: [n.x, n.y, m.node[0], m.node[1]] };
      },
      /** The app's edge between two nodes of shot `name`, in frame coordinates. */
      edge(name, from, to) {
        const a = this.node(name, from), b = this.node(name, to);
        return edgeCurve({ x: a.x + a.w, y: a.y + a.h / 2 }, { x: b.x, y: b.y + b.h / 2 }, a.z);
      },
    };
  }

  /** A dark terminal / editor panel; returns its content origin. */
  function panel(ctx, x, y, w, h, { title = "", alpha = 1, accent = null } = {}) {
    ctx.save();
    ctx.globalAlpha *= alpha;
    ctx.shadowColor = "rgba(0,0,0,0.6)";
    ctx.shadowBlur = 70;
    ctx.shadowOffsetY = 28;
    rr(ctx, x, y, w, h, 14);
    ctx.fillStyle = "#0d0e12";
    ctx.fill();
    ctx.shadowColor = "transparent";
    ctx.lineWidth = accent ? 2 : 1;
    ctx.strokeStyle = accent || "rgba(255,255,255,0.13)";
    ctx.stroke();
    ctx.save();
    rr(ctx, x, y, w, h, 14);
    ctx.clip();
    ctx.fillStyle = "#16171c";
    ctx.fillRect(x, y, w, 40);
    ctx.fillStyle = "rgba(255,255,255,0.07)";
    ctx.fillRect(x, y + 40, w, 1);
    ctx.restore();
    ["#f2555a", "#f5b949", "#4cc38a"].forEach((col, i) => {
      ctx.fillStyle = col;
      ctx.beginPath();
      ctx.arc(x + 22 + i * 22, y + 20, 6.5, 0, Math.PI * 2);
      ctx.fill();
    });
    text(ctx, title, x + w / 2, y + 21, { f: font(500, 15, MONO), color: C.faint, align: "center", base: "middle" });
    ctx.restore();
    return { x: x + 26, y: y + 40 + 22 };
  }

  // ---- logo ------------------------------------------------------------------------------
  const LOGO = [
    new Path2D("M 246 64 C 180 74 150 98 150 139 L 150 203 L 246 159 Z"),
    new Path2D(
      "M 150 218 L 259 166 L 376 166 C 376 212 349 232 305 232 L 246 232 L 246 350 C 246 371 254 381 277 381 L 337 381 C 335 423 305 448 258 448 C 186 448 150 407 150 344 Z",
    ),
  ];
  /** The mark, `size` pixels tall around (cx, cy); `open` slides the two halves apart along the cut. */
  function logoMark(ctx, cx, cy, size, { open = 0, color = "#fff", alpha = 1 } = {}) {
    const s = size / 512;
    // The cut runs along (109, -52): slide the halves in opposite directions on it.
    const dx = 109 * open * 2.2, dy = -52 * open * 2.2;
    ctx.save();
    ctx.globalAlpha *= alpha;
    ctx.translate(cx - 263 * s, cy - 256 * s);
    ctx.scale(s, s);
    ctx.fillStyle = color;
    ctx.save();
    ctx.translate(-dx, -dy);
    ctx.fill(LOGO[0]);
    ctx.restore();
    ctx.save();
    ctx.translate(dx, dy);
    ctx.fill(LOGO[1]);
    ctx.restore();
    ctx.restore();
  }

  // ---- post ---------------------------------------------------------------------------------
  let grainTile = null;
  let bloomCanvas = null;
  function post(ctx, t, { bloom = 0.2, grain = 0.045, vignette = 0.4 } = {}) {
    if (bloom > 0) {
      if (!bloomCanvas) bloomCanvas = new OffscreenCanvas(W / 4, H / 4);
      const b = bloomCanvas.getContext("2d");
      b.globalCompositeOperation = "copy";
      b.filter = "brightness(0.8) contrast(2.2) blur(9px)";
      b.drawImage(ctx.canvas, 0, 0, W / 4, H / 4);
      b.filter = "none";
      ctx.save();
      ctx.globalCompositeOperation = "screen";
      ctx.globalAlpha = bloom;
      ctx.imageSmoothingQuality = "high";
      ctx.drawImage(bloomCanvas, 0, 0, W, H);
      ctx.restore();
    }
    if (vignette > 0) {
      const grad = ctx.createRadialGradient(W / 2, H / 2, H * 0.45, W / 2, H / 2, H * 1.05);
      grad.addColorStop(0, "rgba(0,0,0,0)");
      grad.addColorStop(1, `rgba(0,0,0,${vignette})`);
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, W, H);
    }
    if (grain > 0) {
      if (!grainTile) {
        grainTile = new OffscreenCanvas(256, 256);
        const gc = grainTile.getContext("2d");
        const id = gc.createImageData(256, 256);
        const r = PV.rng(99);
        for (let i = 0; i < id.data.length; i += 4) {
          const v = 128 + (r() - 0.5) * 255;
          id.data[i] = id.data[i + 1] = id.data[i + 2] = v;
          id.data[i + 3] = 255;
        }
        gc.putImageData(id, 0, 0);
      }
      const r = PV.rng(Math.floor(t * 60) + 1);
      const ox = Math.floor(r() * 256), oy = Math.floor(r() * 256);
      ctx.save();
      ctx.globalCompositeOperation = "overlay";
      ctx.globalAlpha = grain;
      for (let y = -oy; y < H; y += 256) for (let x = -ox; x < W; x += 256) ctx.drawImage(grainTile, x, y);
      ctx.restore();
    }
  }

  PV.E = {
    W, H, clamp, lerp, prog, ease, ep, hit, beatPulse, C, rgba, SANS, MONO, font, rr, text, measure, riseText, dotGrid,
    NODE_W, NODE_H, CELL_W, CELL_H, statusIcon, card, edgeCurve, bez, strokeCurve, edge, comet, ringBurst, sparks,
    cursor, keycap, A, loadData, shot, settle, meta, nodeOf, win, panel, logoMark, post, TITLEBAR,
  };
})(typeof window !== "undefined" ? window : globalThis);
