// 40–58 s: the build-up, a large graph finishing itself in topological order, and the logo.
(function (g) {
  const PV = g.PV;
  const E = PV.E;
  const { W, H, C, clamp, lerp, prog, ease, ep, hit, rgba, font, text, rr } = E;
  const T = PV.T;

  // ---- 40–44: build ------------------------------------------------------------------
  const RAIL = "topo ready  ·  topo add  ·  topo link  ·  topo status  ·  topo milestones  ·  topo graph  ·  topo apply  ·  topo organize  ·  topo tui  ·  ";

  function build(ctx, t) {
    const x = prog(t, T.build, 3.5);
    // The product stays on screen as texture, pushing in as the tension rises.
    const bmp = E.shot("a_after");
    if (bmp && t < PV.buildGap) {
      const sc = lerp(1.25, 2.1, ease.inQuad(x));
      ctx.save();
      ctx.globalAlpha = 0.2 * ep(t, T.build, 0.3) * (1 - ep(t, PV.buildGap - 0.1, 0.1));
      ctx.translate(W / 2 - 250, H / 2 + 30);
      ctx.scale(sc, sc);
      ctx.drawImage(bmp, -W / 2, -(W * bmp.height) / bmp.width / 2, W, (W * bmp.height) / bmp.width);
      ctx.restore();
    }

    let i = -1;
    for (let k = 0; k < PV.flashes.length; k++) if (PV.flashes[k] <= t) i = k;
    if (i >= 0 && t < PV.buildGap + 0.14) {
      // Words too big for the frame: each one is cut off by the edges and keeps
      // sliding through, alternating direction, so the type never stands still.
      const t0 = PV.flashes[i];
      const t1 = PV.flashes[i + 1] ?? PV.buildGap;
      const word = PV.BUILD_WORDS[i % PV.BUILD_WORDS.length];
      const w100 = E.measure(ctx, word, font(900, 100), -4);
      const size = Math.max(620, Math.min(1180, ((W * 1.42) / w100) * 100));
      const f = font(900, size);
      const ls = -0.04 * size;
      const collapse = ease.inExpo(prog(t, PV.buildGap, 0.13));
      const dir = i % 2 ? 1 : -1;
      const slide = dir * (lerp(260, 60, ep(t, t0, Math.max(0.3, t1 - t0))) - 190 * prog(t, t0, 0.6));
      const style = i % 4;
      ctx.save();
      ctx.translate(W / 2 + slide * (1 - collapse), H / 2 + (i % 3 - 1) * 46);
      ctx.scale(1 - collapse, 1 - collapse);
      const tw = E.measure(ctx, word, f, ls);
      if (style === 3) {
        ctx.fillStyle = C.green;
        ctx.fillRect(-tw / 2 - 400, -size * 0.5, tw + 800, size * 1.0);
        text(ctx, word, 0, size * 0.355, { f, color: "#08090c", align: "center", ls });
      } else if (style === 1) {
        ctx.font = f;
        ctx.letterSpacing = `${ls}px`;
        ctx.textAlign = "center";
        ctx.lineWidth = 5;
        ctx.strokeStyle = C.text;
        ctx.lineJoin = "round";
        ctx.strokeText(word, 0, size * 0.355);
      } else {
        text(ctx, word, 0, size * 0.355, { f, color: style === 2 ? C.green : C.text, align: "center", ls });
      }
      ctx.restore();
    }

    // Rails of commands, sliding in opposite directions, faster and faster.
    if (t < PV.buildGap) {
      const f = font(500, 26, E.MONO);
      const rw = E.measure(ctx, RAIL, f, 1);
      const shift = (ease.inQuad(x) * 2600 + x * 300) % rw;
      const a = ep(t, T.build, 0.4) * (1 - ep(t, PV.buildGap - 0.1, 0.1));
      for (let k = -1; k < 3; k++) {
        text(ctx, RAIL, k * rw - shift, H - 70, { f, color: C.faint, ls: 1, alpha: a });
        text(ctx, RAIL, k * rw - rw + shift, 172, { f, color: C.faint, ls: 1, alpha: a * 0.7 });
      }
      ctx.fillStyle = C.green;
      ctx.fillRect(0, H - 8, W * ease.inQuad(x), 8);
    }

    // The gap before the drop: one point again.
    if (t >= PV.buildGap) {
      ctx.save();
      ctx.shadowColor = C.green;
      ctx.shadowBlur = 44;
      ctx.fillStyle = "#fff";
      ctx.beginPath();
      ctx.arc(W / 2, H / 2, 7, 0, Math.PI * 2);
      ctx.fill();
      ctx.restore();
    }
  }

  // ---- 44–52: the wave ---------------------------------------------------------------------
  const VERBS = ["Build", "Design", "Write", "Test", "Ship", "Review", "Fix", "Deploy", "Plan", "Draft", "Wire", "Tune"];
  const NOUNS = ["API", "schema", "docs", "UI", "tests", "auth", "pipeline", "dashboard", "billing", "search", "onboarding", "cache", "sync", "export"];
  const MS_NAMES = ["Alpha", "Beta", "v1.0", "Launch"];
  const CW = 210, CH = 70, MW = 156, MH = 46;
  let megaReady = false;
  function mega() {
    const M = PV.mega;
    if (megaReady) return M;
    const r = PV.rng(31);
    const abc = "0123456789abcdefghijklmnopqrstuvwxyz";
    for (const n of M.nodes) {
      n.x = n.col * CW;
      n.y = n.row * CH;
      n.title = n.milestone ? MS_NAMES[Math.floor(n.col / 4)] : `${VERBS[Math.floor(r() * VERBS.length)]} ${NOUNS[Math.floor(r() * NOUNS.length)]}`;
      n.id = Array.from({ length: 6 }, () => abc[Math.floor(r() * 36)]).join("");
      n.readyAt = n.deps.length ? Math.max(...n.deps.map((d) => M.nodes[d].t)) : T.climax - 1;
    }
    M.order = M.nodes.slice().sort((a, b) => a.t - b.t || a.row - b.row);
    M.total = M.nodes.length;
    megaReady = true;
    return M;
  }

  function climax(ctx, t) {
    const M = mega();
    const zKeys = [[44, 1.75], [46, 1.5], [48, 1.3], [50, 1.1], [50.8, 1.05], [51.7, 0.56]];
    let z = zKeys[0][1];
    for (let i = 1; i < zKeys.length; i++) z = lerp(z, zKeys[i][1], ease.inOutCubic(prog(t, zKeys[i - 1][0], zKeys[i][0] - zKeys[i - 1][0])));
    // Burst out of the point the build collapsed into.
    const open = ep(t, T.climax, 1.5);
    const front = (t - T.climax) * 2 * CW;
    const cxW = lerp(front + 150, (15 * CW + MW) / 2, ease.inOutCubic(prog(t, 50.6, 1.1)));
    const cyW = Math.sin(t * 0.9) * 18;
    const implode = ease.inExpo(prog(t, 51.72, 0.28));
    const sx = (wx) => lerp(W / 2, W / 2 + (wx - cxW) * z, open) * (1 - implode) + (W / 2) * implode;
    const sy = (wy) => lerp(H / 2, H / 2 + 10 + (wy - cyW) * z, open) * (1 - implode) + (H / 2) * implode;
    const zz = z * lerp(0.3, 1, open) * (1 - implode * 0.85);

    E.dotGrid(ctx, W / 2 - cxW * z, H / 2 - cyW * z, z, 0.9 * open * (1 - implode), "#22252e");

    const fl = hit(t, T.climax, 0.07);
    if (fl > 0.01) {
      ctx.fillStyle = rgba("#ffffff", fl * 0.4);
      ctx.fillRect(0, 0, W, H);
    }

    const sizeOf = (n) => (n.milestone ? [MW + 34, MH + 12] : [MW, MH]);
    const visible = (n) => {
      const x = sx(n.x);
      return x > -400 && x < W + 400;
    };

    // Edges, then the comets that carry each completion forward.
    ctx.save();
    ctx.globalAlpha = open * (1 - implode);
    for (const n of M.nodes) {
      if (!visible(n)) continue;
      const [nw, nh] = sizeOf(n);
      for (const di of n.deps) {
        const d = M.nodes[di];
        const [dw, dh] = sizeOf(d);
        const curve = E.edgeCurve({ x: sx(d.x) + dw * zz, y: sy(d.y) + (dh * zz) / 2 }, { x: sx(n.x), y: sy(n.y) + (nh * zz) / 2 }, zz);
        const done = t >= d.t;
        const col = n.milestone ? C.amber : done ? C.green : C.edge;
        E.edge(ctx, curve, { color: col, width: (done ? 1.9 : 1.3) * zz, dashed: n.milestone, alpha: done ? 0.75 : 0.55, arrow: zz > 0.7 });
        if (done) E.comet(ctx, curve, prog(t, d.t, Math.max(0.14, Math.min(0.4, n.t - d.t))), n.milestone ? C.amber : C.green, 4.5 * zz, 0.3);
      }
    }
    for (const n of M.nodes) {
      if (!visible(n)) continue;
      const [nw, nh] = sizeOf(n);
      const done = t >= n.t;
      const ready = !done && t >= n.readyAt;
      const pop = done ? hit(t, n.t, 0.16) : 0;
      const x = sx(n.x), y = sy(n.y);
      const w = nw * zz, h = nh * zz;
      const rad = (n.milestone ? 12 : 7) * zz;
      ctx.save();
      ctx.translate(x + w / 2, y + h / 2);
      ctx.scale(1 + pop * 0.05, 1 + pop * 0.05);
      ctx.translate(-w / 2, -h / 2);
      const tint = n.milestone ? C.amber : C.green;
      if (done || ready) {
        ctx.shadowColor = tint;
        ctx.shadowBlur = (done ? 10 + 40 * pop : 20) * zz;
      }
      rr(ctx, 0, 0, w, h, rad);
      ctx.fillStyle = n.milestone ? "#221f1b" : C.card;
      ctx.fill();
      ctx.shadowColor = "transparent";
      if (done) {
        ctx.fillStyle = rgba(tint, 0.14 + 0.5 * pop);
        ctx.fill();
      }
      ctx.lineWidth = Math.max(1, (done || ready ? 1.6 : 1) * zz);
      ctx.strokeStyle = done ? rgba(tint, 0.75) : ready ? rgba(C.green, 0.8) : n.milestone ? rgba(C.amber, 0.45) : C.border;
      ctx.stroke();
      const dim = done || ready ? 1 : 0.5;
      ctx.globalAlpha *= dim;
      E.statusIcon(ctx, 15 * zz, h / 2, zz * 0.95, n.milestone ? "milestone" : "task", done ? "done" : "todo");
      if (zz > 0.75) {
        const f = font(n.milestone ? 700 : 500, 12.5 * zz);
        const tw = text(ctx, n.title, 28 * zz, h / 2 + 0.5 * zz, { f, color: done && !n.milestone ? C.muted : C.text, base: "middle" });
        if (done && !n.milestone) {
          ctx.fillStyle = C.muted;
          ctx.fillRect(28 * zz, h / 2 + 0.5 * zz, tw, Math.max(1, zz));
        }
      }
      ctx.restore();
      if (done) {
        E.ringBurst(ctx, x, y, w, h, rad, t - n.t, tint, n.milestone ? { grow: 150 * zz, width: 7, dur: 0.9 } : { grow: 20 * zz, width: 2.5, dur: 0.4 });
        if (n.milestone) {
          E.ringBurst(ctx, x, y, w, h, rad, t - n.t - 0.07, tint, { grow: 80 * zz, width: 4, dur: 0.8 });
          E.sparks(ctx, x + w / 2, y + h / 2, t - n.t, tint, 70 + n.col, { n: 40, dist: 420 * zz, dur: 0.9 });
        }
      }
    }
    ctx.restore();

    // Words on the bar lines: solid for an instant, then an outline falling back.
    for (const cw of PV.climaxWords) {
      const dt = t - cw.t;
      if (dt < 0 || dt > 1.9) continue;
      const w100 = E.measure(ctx, cw.w, font(900, 100), -4);
      const size = Math.min(560, (1700 / w100) * 100);
      const f = font(900, size);
      const ls = -0.04 * size;
      const sc = lerp(1.4, 1, ep(dt, 0, 1.3)) * lerp(1, 1.08, prog(dt, 0, 1.9));
      const solid = 1 - ep(dt, 0.08, 0.5, ease.outCubic);
      ctx.save();
      ctx.translate(W / 2, H / 2);
      ctx.scale(sc, sc);
      ctx.font = f;
      ctx.letterSpacing = `${ls}px`;
      ctx.textAlign = "center";
      if (solid > 0.01) {
        ctx.globalAlpha = solid;
        ctx.fillStyle = "#fff";
        ctx.fillText(cw.w, 0, size * 0.36);
      }
      ctx.globalAlpha = 0.85 * (1 - ep(dt, 1.0, 0.9, ease.inOutCubic));
      ctx.lineWidth = 3.5;
      ctx.lineJoin = "round";
      ctx.strokeStyle = "#fff";
      ctx.strokeText(cw.w, 0, size * 0.36);
      ctx.restore();
    }

    // HUD: the numbers behind the wave.
    const hud = ep(t, T.climax + 0.1, 1.0) * (1 - ep(t, 51.5, 0.35, ease.inCubic));
    if (hud > 0) {
      const doneN = M.order.filter((n) => n.t <= t).length;
      const readyN = M.nodes.filter((n) => t < n.t && t >= n.readyAt).length;
      ctx.save();
      ctx.globalAlpha = hud;
      const top = ctx.createLinearGradient(0, 0, 0, 190);
      top.addColorStop(0, rgba(C.bg, 0.9));
      top.addColorStop(1, rgba(C.bg, 0));
      ctx.fillStyle = top;
      ctx.fillRect(0, 0, W, 190);
      const bot = ctx.createLinearGradient(0, H - 330, 0, H);
      bot.addColorStop(0, rgba(C.bg, 0));
      bot.addColorStop(1, rgba(C.bg, 0.92));
      ctx.fillStyle = bot;
      ctx.fillRect(0, H - 330, W, 330);

      text(ctx, "$ topo ready", 96, 96, { f: font(600, 30, E.MONO), color: C.muted });
      ctx.fillStyle = C.green;
      ctx.beginPath();
      ctx.arc(104, 138, 8, 0, Math.PI * 2);
      ctx.fill();
      text(ctx, `${readyN} ready`, 126, 149, { f: font(700, 34), color: C.green });

      const numF = font(800, 84, E.MONO);
      const totW = text(ctx, `/${String(M.total).padStart(3, "0")} done`, W - 96, 150, { f: font(600, 34, E.MONO), color: C.faint, align: "right" });
      text(ctx, String(doneN).padStart(3, "0"), W - 96 - totW - 10, 150, { f: numF, color: C.text, align: "right", ls: -2 });

      // A log of what just finished, as the CLI would list it.
      const recent = M.order.filter((n) => n.t <= t).slice(-7);
      recent.forEach((n, k) => {
        const age = recent.length - 1 - k;
        const a = (1 - age / 7.5) * ep(t, n.t, 0.25);
        const y = H - 84 - age * 34 + (1 - ep(t, n.t, 0.5)) * 26;
        const w1 = text(ctx, "[x] ", 96, y, { f: font(500, 22, E.MONO), color: n.milestone ? C.amber : C.green, alpha: a });
        text(ctx, `${n.id}  ${n.milestone ? "◆ " : ""}${n.title}`, 96 + w1, y, { f: font(500, 22, E.MONO), color: age === 0 ? C.text : C.muted, alpha: a });
      });

      const frac = doneN / M.total;
      ctx.fillStyle = "rgba(255,255,255,0.08)";
      ctx.fillRect(0, H - 8, W, 8);
      ctx.fillStyle = C.green;
      ctx.fillRect(0, H - 8, W * frac, 8);
      ctx.restore();
    }
  }

  // ---- 52–58: logo ---------------------------------------------------------------------------
  function logo(ctx, t) {
    const L = PV.logo;
    const lt = t - L.hit;
    const drift = lerp(1, 1.04, prog(t, L.hit, 6));
    ctx.save();
    ctx.translate(W / 2, 540);
    ctx.scale(drift, drift);
    ctx.translate(-W / 2, -540);
    const glow = ctx.createRadialGradient(W / 2, 430, 0, W / 2, 430, 900);
    glow.addColorStop(0, rgba(C.accent, 0.13 * ep(t, L.hit, 0.8)));
    glow.addColorStop(0.5, rgba(C.green, 0.045 * ep(t, L.hit, 0.8)));
    glow.addColorStop(1, rgba(C.bg, 0));
    ctx.fillStyle = glow;
    ctx.fillRect(0, 0, W, H);
    E.dotGrid(ctx, 6, 14, 1.5, 0.7 * ep(t, L.hit, 0.6), "#1d2028");

    // The wave's last ring.
    const sw = prog(t, L.hit, 1.1);
    if (sw < 1) {
      ctx.save();
      ctx.strokeStyle = rgba("#ffffff", (1 - sw) * 0.55);
      ctx.lineWidth = 7 * (1 - sw) + 1;
      ctx.beginPath();
      ctx.arc(W / 2, 430, ease.outCubic(sw) * 1500, 0, Math.PI * 2);
      ctx.stroke();
      ctx.restore();
    }
    const fl = hit(t, L.hit, 0.09);
    if (fl > 0.01) {
      ctx.fillStyle = rgba("#ffffff", fl * 0.5);
      ctx.fillRect(0, 0, W, H);
    }

    const size = 340;
    const f = font(800, size);
    const ls = -13;
    const wordW = E.measure(ctx, "topo", f, ls);
    const markW = 258;
    const x0 = W / 2 - (markW + 44 + wordW) / 2;
    const baseY = 500;
    const pop = lerp(1.3, 1, ep(t, L.hit, 1.6));
    ctx.save();
    ctx.translate(x0 + markW / 2, baseY - 122);
    ctx.scale(pop, pop);
    E.logoMark(ctx, 0, 0, 454, { open: (1 - ep(t, L.hit, 1.5)) * 0.6, alpha: ep(t, L.hit, 0.08) });
    ctx.restore();
    let lx = x0 + markW + 44;
    "topo".split("").forEach((ch, i) => {
      const w = E.measure(ctx, ch, f, ls);
      const a = ep(lt, 0.05 + i * 0.07, 1.2);
      ctx.save();
      ctx.beginPath();
      ctx.rect(lx - 20, baseY - size, w + 40, size * 1.3);
      ctx.clip();
      text(ctx, ch, lx, baseY + (1 - a) * size * 1.1, { f, ls });
      ctx.restore();
      lx += w;
    });

    E.riseText(ctx, "Know what's next.", W / 2, 668, t - L.tagline, {
      size: 98, weight: 800, align: "center", dur: 1.1, stagger: 0.1, colors: [C.text, C.text, C.green],
    });

    // The last word goes to the product: the only thing ready is trying it.
    if (t >= L.type[0] - 0.45) {
      const a = ep(t, L.type[0] - 0.45, 0.9);
      const fs = 38;
      const fm = font(500, fs, E.MONO);
      const typed = L.type.filter((x) => x <= t).length;
      const cmd = PV.END_CMD.slice(0, typed);
      const full = E.measure(ctx, "$ " + PV.END_CMD, fm);
      const answer = "[ ] t0p0go  Try topo";
      const blockW = Math.max(full, E.measure(ctx, answer, fm));
      const bx = W / 2 - blockW / 2;
      ctx.save();
      ctx.globalAlpha = a;
      rr(ctx, bx - 44, 762, blockW + 88, 150, 18);
      ctx.fillStyle = "#0d0e12";
      ctx.fill();
      ctx.strokeStyle = rgba(C.green, 0.25 + 0.75 * hit(t, L.answer, 0.4) * (t >= L.answer ? 1 : 0));
      ctx.lineWidth = 1.5;
      ctx.stroke();
      const w1 = text(ctx, "$ ", bx, 818, { f: fm, color: C.green });
      const w2 = text(ctx, cmd, bx + w1, 818, { f: fm, color: C.text });
      if (t < L.answer && Math.floor(t * 3.2) % 2 === 0) {
        ctx.fillStyle = C.text;
        ctx.fillRect(bx + w1 + w2 + 4, 786, 20, 40);
      }
      if (t >= L.answer) {
        const k = ep(t, L.answer, 0.9);
        const w3 = text(ctx, "[ ] t0p0go  ", bx, 876 + (1 - k) * 14, { f: fm, color: C.muted, alpha: k });
        text(ctx, "Try topo", bx + w3, 876 + (1 - k) * 14, { f: font(700, fs, E.MONO), color: C.green, alpha: k });
      }
      ctx.restore();
      E.ringBurst(ctx, bx - 44, 762, blockW + 88, 150, 18, t - L.answer, C.green, { grow: 36, width: 3, dur: 0.7 });
    }
    const ua = ep(t, L.url - 0.1, 1.2);
    text(ctx, "github.com/r4ai/topo", W / 2, 990 + (1 - ua) * 24, { f: font(500, 30, E.MONO), color: C.muted, align: "center", ls: 1, alpha: ua });
    ctx.restore();
  }

  PV.scenes = PV.scenes || {};
  Object.assign(PV.scenes, { build, climax, logo });
})(typeof window !== "undefined" ? window : globalThis);
