// Soundtrack of the promo video, synthesized from the shared timeline.
// Usage: node synth.mjs <out.wav>
import { writeFileSync } from "node:fs";
import "../src/shared.js";

const PV = globalThis.PV;
const { T } = PV;
const SR = 48000;
const DUR = T.end;
const N = Math.ceil(DUR * SR);
const TAU = Math.PI * 2;

const bus = () => ({ L: new Float32Array(N), R: new Float32Array(N) });
const B = { kick: bus(), drums: bus(), bass: bus(), music: bus(), sfx: bus(), rev: bus(), dly: bus() };

const mtof = (m) => 440 * Math.pow(2, (m - 69) / 12);
const clamp = (x, a, b) => Math.min(b, Math.max(a, x));

let seed = 12345;
const rand = () => {
  seed ^= seed << 13;
  seed ^= seed >>> 17;
  seed ^= seed << 5;
  return ((seed >>> 0) / 4294967296) * 2 - 1;
};

/** Zavalishin state variable filter; returns [low, band, high]. */
function svf() {
  let ic1 = 0, ic2 = 0, g = 0, k = 1, a1 = 0, a2 = 0, a3 = 0;
  return {
    set(fc, q) {
      g = Math.tan(Math.PI * clamp(fc, 20, SR * 0.45) / SR);
      k = 1 / q;
      a1 = 1 / (1 + g * (g + k));
      a2 = g * a1;
      a3 = g * a2;
    },
    run(x) {
      const v3 = x - ic2;
      const v1 = a1 * ic1 + a2 * v3;
      const v2 = ic2 + a2 * ic1 + a3 * v3;
      ic1 = 2 * v1 - ic1;
      ic2 = 2 * v2 - ic2;
      return [v2, v1, x - k * v1 - v2];
    },
  };
}

/** Band-limited sawtooth. */
function saw(freq, phase0 = Math.random()) {
  let p = phase0 % 1;
  return (f = freq) => {
    const dt = f / SR;
    p += dt;
    if (p >= 1) p -= 1;
    let s = 2 * p - 1;
    if (p < dt) {
      const x = p / dt;
      s -= x + x - x * x - 1;
    } else if (p > 1 - dt) {
      const x = (p - 1) / dt;
      s -= x * x + x + x + 1;
    }
    return s;
  };
}

/**
 * Renders a voice: `fn(tt, i)` returns a mono sample (or [l, r]) for `len` seconds from `t0`.
 * `pan` is -1..1; `rev` and `dly` are send amounts.
 */
function voice(target, t0, len, gain, fn, { pan = 0, rev = 0, dly = 0 } = {}) {
  const start = Math.round(t0 * SR);
  const count = Math.round(len * SR);
  const gl = gain * Math.cos(((pan + 1) * Math.PI) / 4);
  const gr = gain * Math.sin(((pan + 1) * Math.PI) / 4);
  for (let i = 0; i < count; i++) {
    const j = start + i;
    if (j < 0) continue;
    if (j >= N) break;
    const s = fn(i / SR, i);
    let l, r;
    if (typeof s === "number") {
      l = s * gl * Math.SQRT2;
      r = s * gr * Math.SQRT2;
    } else {
      l = s[0] * gain;
      r = s[1] * gain;
    }
    // Short fade at the tail so no voice ends on a step.
    const tail = count - i;
    if (tail < 96) {
      l *= tail / 96;
      r *= tail / 96;
    }
    target.L[j] += l;
    target.R[j] += r;
    if (rev) {
      B.rev.L[j] += l * rev;
      B.rev.R[j] += r * rev;
    }
    if (dly) {
      B.dly.L[j] += l * dly;
      B.dly.R[j] += r * dly;
    }
  }
}

// ---- drums ------------------------------------------------------------------
function kick(t, g = 1) {
  let ph = 0;
  voice(B.kick, t, 0.5, g, (tt) => {
    const f = 47 + 105 * Math.exp(-tt / 0.05) + 380 * Math.exp(-tt / 0.005);
    ph += (TAU * f) / SR;
    const env = Math.exp(-tt / 0.19) * Math.min(1, tt / 0.0008);
    return Math.tanh(Math.sin(ph) * env * 2.2) * 0.8 + rand() * Math.exp(-tt / 0.0025) * 0.12;
  });
}

function clap(t, g = 1) {
  const f = svf();
  f.set(1500, 1.4);
  const hp = svf();
  hp.set(700, 0.7);
  voice(B.drums, t, 0.4, g, (tt) => {
    let env = 0;
    for (const b of [0, 0.011, 0.021]) if (tt >= b) env += Math.exp(-(tt - b) / 0.005);
    if (tt >= 0.03) env += Math.exp(-(tt - 0.03) / 0.085) * 0.7;
    return hp.run(f.run(rand())[1] * 2.4)[2] * env;
  }, { rev: 0.35 });
}

function snare(t, g = 1) {
  const f = svf();
  f.set(2100, 0.8);
  let ph = 0;
  voice(B.drums, t, 0.22, g, (tt) => {
    ph += (TAU * (165 + 60 * Math.exp(-tt / 0.02))) / SR;
    return f.run(rand())[1] * Math.exp(-tt / 0.06) * 1.5 + Math.sin(ph) * Math.exp(-tt / 0.045) * 0.5;
  }, { rev: 0.2 });
}

function hat(t, g = 1, open = false, pan = 0.15) {
  const f = svf();
  f.set(open ? 6500 : 8500, 0.9);
  const dec = open ? 0.11 : 0.02;
  voice(B.drums, t, open ? 0.45 : 0.1, g, (tt) => f.run(rand())[2] * Math.exp(-tt / dec), { pan });
}

function crash(t, g = 1) {
  const fl = svf(), fr = svf();
  fl.set(4200, 0.7);
  fr.set(4700, 0.7);
  voice(B.drums, t, 2.6, g, (tt) => {
    const env = Math.exp(-tt / 0.75) * Math.min(1, tt / 0.002);
    return [fl.run(rand())[2] * env, fr.run(rand())[2] * env];
  }, { rev: 0.4 });
}

// ---- effects -----------------------------------------------------------------
function tick(t, g = 1, midi = 96, pan = 0) {
  let ph = 0;
  const f0 = mtof(midi);
  const hp = svf();
  hp.set(5000, 0.8);
  voice(B.sfx, t, 0.09, g, (tt) => {
    ph += (TAU * f0 * (1 + 1.5 * Math.exp(-tt / 0.0015))) / SR;
    return Math.sin(ph) * Math.exp(-tt / 0.014) * 0.6 + hp.run(rand())[2] * Math.exp(-tt / 0.003) * 0.7;
  }, { pan, rev: 0.12 });
}

/** A key press: a short woody thock. */
function key(t, g = 1, pan = 0) {
  const f = svf();
  f.set(1250 + rand() * 250, 2.2);
  let ph = 0;
  voice(B.sfx, t, 0.09, g, (tt) => {
    ph += (TAU * (190 + 120 * Math.exp(-tt / 0.004))) / SR;
    return f.run(rand())[1] * Math.exp(-tt / 0.009) * 1.6 + Math.sin(ph) * Math.exp(-tt / 0.02) * 0.6;
  }, { pan, rev: 0.08 });
}

function thud(t, g = 1) {
  let ph = 0;
  voice(B.sfx, t, 0.5, g, (tt) => {
    ph += (TAU * (46 + 60 * Math.exp(-tt / 0.03))) / SR;
    return Math.sin(ph) * Math.exp(-tt / 0.16) * Math.min(1, tt / 0.001) + rand() * Math.exp(-tt / 0.002) * 0.15;
  }, { rev: 0.2 });
}

function impact(t, g = 1) {
  let ph = 0;
  const lp = svf();
  lp.set(320, 0.7);
  const hp = svf();
  hp.set(2500, 0.7);
  voice(B.sfx, t, 3.2, g, (tt) => {
    ph += (TAU * (33 + 85 * Math.exp(-tt / 0.11))) / SR;
    const n = rand();
    const body = Math.tanh(Math.sin(ph) * 1.6) * Math.exp(-tt / 0.75);
    return (body + lp.run(n)[0] * Math.exp(-tt / 0.4) * 1.8 + hp.run(n)[2] * Math.exp(-tt / 0.16) * 0.3) * Math.min(1, tt / 0.001);
  }, { rev: 0.5 });
}

function whoosh(t0, dur, g = 1, dir = 1) {
  const f = svf();
  voice(B.sfx, t0, dur, g, (tt, i) => {
    const x = tt / dur;
    if (i % 16 === 0) f.set(350 * Math.pow(18, Math.sin(Math.PI * x)), 1.6);
    const s = f.run(rand())[1] * Math.pow(Math.sin(Math.PI * x), 2) * 1.6;
    const p = ((dir * (x * 2 - 1) * 0.8 + 1) * Math.PI) / 4;
    return [s * Math.cos(p) * Math.SQRT2, s * Math.sin(p) * Math.SQRT2];
  }, { rev: 0.25 });
}

function riser(t0, dur, g = 1, midi = 54) {
  const fl = svf(), fr = svf();
  const voices = [saw(1), saw(1), saw(1)];
  const lp = svf();
  voice(B.sfx, t0, dur, g, (tt, i) => {
    const x = tt / dur;
    if (i % 16 === 0) {
      const fc = 500 * Math.pow(22, x);
      fl.set(fc, 1.1);
      fr.set(fc * 1.07, 1.1);
      lp.set(600 + 6000 * x * x, 1.2);
    }
    const env = Math.pow(x, 2.2);
    const f0 = mtof(midi) * Math.pow(2, x * x);
    const tone = lp.run((voices[0](f0) + voices[1](f0 * 1.007) + voices[2](f0 * 0.993)) / 3)[0];
    const trem = 0.6 + 0.4 * Math.sin(TAU * (4 + 12 * x) * tt);
    const t2 = tone * env * 0.35 * trem;
    return [fl.run(rand())[2] * env * 0.9 + t2, fr.run(rand())[2] * env * 0.9 + t2];
  }, { rev: 0.3 });
}

/** Noise swelling up to an abrupt stop at `tEnd`. */
function revcym(tEnd, dur, g = 1) {
  const fl = svf(), fr = svf();
  fl.set(3800, 0.7);
  fr.set(4300, 0.7);
  voice(B.sfx, tEnd - dur, dur, g, (tt) => {
    const env = Math.exp((tt - dur) / (dur / 3.5));
    return [fl.run(rand())[2] * env, fr.run(rand())[2] * env];
  }, { rev: 0.15 });
}

function ding(t, midi, g = 1, pan = 0) {
  const f0 = mtof(midi);
  const parts = [[1, 1, 0.55], [2.0, 0.28, 0.3], [3.01, 0.16, 0.12], [5.4, 0.07, 0.05]];
  voice(B.sfx, t, 1.6, g, (tt) => {
    let s = 0;
    for (const [ratio, amp, dec] of parts) s += Math.sin(TAU * f0 * ratio * tt) * amp * Math.exp(-tt / dec);
    return s * Math.min(1, tt / 0.0015) * 0.7;
  }, { pan, rev: 0.4, dly: 0.3 });
}

/** A rising blip: something just unlocked. */
function pop(t, g = 1, pan = 0) {
  let ph = 0;
  voice(B.sfx, t, 0.3, g, (tt) => {
    ph += (TAU * (330 * Math.pow(3.4, Math.min(1, tt / 0.06)))) / SR;
    return Math.sin(ph) * Math.min(1, tt / 0.003) * Math.exp(-tt / 0.07);
  }, { pan, rev: 0.3, dly: 0.2 });
}

// ---- pitched instruments ------------------------------------------------------
function pluck(target, t, midi, len, g = 1, { pan = 0, bright = 1, rev = 0.18, dly = 0.3 } = {}) {
  const f0 = mtof(midi);
  const o1 = saw(f0), o2 = saw(f0 * 1.004);
  const lp = svf();
  voice(target, t, len + 0.25, g, (tt, i) => {
    if (i % 8 === 0) lp.set(f0 * (1.5 + 9 * bright * Math.exp(-tt / 0.07)), 1.3);
    const env = Math.min(1, tt / 0.002) * Math.exp(-tt / (len * 0.45));
    return lp.run((o1() + o2()) * 0.5)[0] * env;
  }, { pan, rev, dly });
}

function bass(t, midi, len, g = 1) {
  const f0 = mtof(midi);
  const o = saw(f0);
  const lp = svf();
  let ph = 0;
  voice(B.bass, t, len + 0.05, g, (tt, i) => {
    if (i % 8 === 0) lp.set(150 + 950 * Math.exp(-tt / 0.1), 1.6);
    ph += (TAU * f0) / SR;
    const env = Math.min(1, tt / 0.004) * Math.min(1, Math.max(0, (len + 0.04 - tt) / 0.04));
    return (lp.run(o())[0] * 0.75 + Math.sin(ph) * 0.8) * env;
  });
}

/** Detuned saws across the stereo field; `shape(tt)` returns [amp, cutoff]. */
function supersaw(target, t, notes, len, g, shape, { rev = 0.3, dly = 0 } = {}) {
  const det = [-0.11, -0.04, 0.03, 0.1];
  const oscs = [];
  for (const m of notes) {
    det.forEach((d, k) => {
      const f = mtof(m + d);
      oscs.push({ o: saw(f), pan: (k / (det.length - 1)) * 2 - 1 });
    });
  }
  const lpL = svf(), lpR = svf();
  const norm = 1 / Math.sqrt(oscs.length);
  voice(target, t, len, g, (tt, i) => {
    const [amp, fc] = shape(tt);
    if (i % 16 === 0) {
      lpL.set(fc, 0.9);
      lpR.set(fc, 0.9);
    }
    let l = 0, r = 0;
    for (const { o, pan } of oscs) {
      const s = o();
      l += s * (1 - pan * 0.6);
      r += s * (1 + pan * 0.6);
    }
    return [lpL.run(l * norm)[0] * amp, lpR.run(r * norm)[0] * amp];
  }, { rev, dly });
}

function stab(t, notes, g = 1, len = 0.22, bright = 1) {
  supersaw(B.music, t, notes, len + 0.2, g, (tt) => [
    Math.min(1, tt / 0.003) * Math.exp(-tt / (len * 0.7)),
    500 + 5200 * bright * Math.exp(-tt / 0.11),
  ], { rev: 0.3, dly: 0.25 });
}

function pad(t, notes, len, g = 1, fc0 = 700, fc1 = 1100) {
  supersaw(B.music, t, notes, len + 0.6, g, (tt) => [
    Math.min(1, tt / 0.35) * Math.min(1, Math.max(0, (len + 0.6 - tt) / 0.6)),
    fc0 + (fc1 - fc0) * (tt / len),
  ], { rev: 0.35 });
}

function lead(t, midi, len, g = 1) {
  const f0 = mtof(midi);
  const o1 = saw(f0), o2 = saw(f0), o3 = saw(f0 * 2);
  const lp = svf();
  voice(B.music, t, len + 0.2, g, (tt, i) => {
    const vib = 1 + 0.004 * Math.sin(TAU * 5.5 * tt) * Math.min(1, tt / 0.25);
    if (i % 8 === 0) lp.set(1400 + 3800 * Math.exp(-tt / 0.25), 1.1);
    const env = Math.min(1, tt / 0.006) * Math.min(1, Math.max(0, (len + 0.18 - tt) / 0.18));
    return lp.run((o1(f0 * vib) + o2(f0 * vib * 1.006) + o3(f0 * 2 * vib) * 0.4) / 2.4)[0] * env;
  }, { rev: 0.3, dly: 0.4 });
}

function drone(t, len, midi, g = 1) {
  const f0 = mtof(midi);
  voice(B.bass, t, len, g, (tt) => {
    const env = Math.min(1, tt / (len * 0.7)) * Math.min(1, Math.max(0, (len - tt) / 0.02));
    return (Math.sin(TAU * f0 * tt) + 0.5 * Math.sin(TAU * f0 * 2 * tt) + 0.2 * Math.sin(TAU * f0 * 3.01 * tt)) * env * 0.6;
  });
}

// ---- harmony ---------------------------------------------------------------------
// vi – IV – I – V in A major, one chord per bar.
const CHORDS = [
  { root: 30, notes: [54, 57, 61, 64] }, // F#m7
  { root: 38, notes: [54, 57, 62, 66] }, // D
  { root: 33, notes: [52, 57, 61, 64] }, // A
  { root: 40, notes: [52, 56, 59, 64] }, // E
];
const chordAt = (t) => CHORDS[(((Math.floor((t - T.drop) / 2 + 1e-6)) % 4) + 4) % 4];
const PENTA = [69, 71, 73, 76, 78, 81, 83, 85, 88]; // A major pentatonic from A4

const kicks = [];
const K = (t, g = 1) => {
  kick(t, g);
  kicks.push(t);
};

// ---- A. hook: the list piles up (0–4) ------------------------------------------
drone(0, 4, 30, 0.4);
drone(0, 4, 42, 0.18);
thud(0, 0.9);
PV.rowTimes.forEach((t, i) => {
  key(t, 0.55, (i % 2 ? 0.2 : -0.2));
  tick(t, 0.22, 84 + (i % 5) * 2, 0);
});
PV.floodTimes.forEach((t, i) => key(t, 0.3 + (0.3 * i) / PV.floodTimes.length, rand() * 0.8));
riser(2, 2, 0.5, 42);
revcym(4, 1.2, 0.3);

// ---- B. stop (4–6) ---------------------------------------------------------------
impact(4, 1.0);
drone(4.05, 3.7, 30, 0.22);
[66, 69, 71, 73].forEach((m, i) => {
  const t = PV.words[i].t;
  thud(t, i === 3 ? 0.75 : 0.5);
  pluck(B.sfx, t, m, 0.5, i === 3 ? 0.3 : 0.2, { bright: 0.5, rev: 0.4, dly: 0.35 });
});

// ---- C. tension (6–8) ------------------------------------------------------------
PV.heart.forEach((t, i) => K(t, 0.75 + i * 0.04));
for (let t = 7; t < 7.75 - 1e-6; t += 0.125) snare(t, 0.2 + ((t - 7) / 0.75) * 0.45);
riser(6, 1.75, 0.6, 42);
revcym(7.75, 1.5, 0.45);

// ---- D. drop: it is a graph (8–16) -----------------------------------------------
function grooveBar(t0, { full = true, arp = false, hatG = 1, stabG = 1, bassG = 1 } = {}) {
  const ch = chordAt(t0);
  for (let b = 0; b < 4; b++) {
    K(t0 + b * 0.5);
    hat(t0 + b * 0.5 + 0.25, 0.3 * hatG, full && b === 3, 0.2);
    hat(t0 + b * 0.5 + 0.125, 0.09 * hatG, false, -0.25);
    hat(t0 + b * 0.5 + 0.375, 0.11 * hatG, false, -0.25);
  }
  clap(t0 + 0.5, 0.55);
  clap(t0 + 1.5, 0.55);
  for (let e = 0; e < 8; e++) bass(t0 + e * 0.25, ch.root + (e === 7 ? 12 : 0), 0.2, 0.5 * bassG);
  for (const s of [0, 3, 6, 8, 11, 14]) stab(t0 + s * 0.125, ch.notes, 0.2 * stabG, 0.2, full ? 1 : 0.6);
  if (arp) {
    const seq = [0, 1, 2, 3, 2, 1, 0, 1, 2, 3, 2, 1, 0, 2, 1, 3];
    seq.forEach((k, s) => {
      pluck(B.music, t0 + s * 0.125, ch.notes[k] + 12, 0.16, 0.13, { pan: s % 2 ? 0.4 : -0.4, bright: 0.9 });
    });
  }
}
impact(8, 1.0);
crash(8, 0.35);
grooveBar(8, { arp: false });
grooveBar(10, { arp: true });
crash(12, 0.3);
grooveBar(12, { arp: true });
grooveBar(14, { arp: true });
// Lead motif over the title.
const MOTIF = [
  // [beat offset, midi, beats]
  [[0, 76, 1], [1, 81, 0.5], [1.5, 85, 0.5], [2, 83, 1], [3, 81, 1]],
  [[0, 83, 1.5], [1.5, 80, 0.5], [2, 76, 1], [3, 83, 1]],
  [[0, 78, 1], [1, 81, 0.5], [1.5, 85, 0.5], [2, 83, 1], [3, 81, 1]],
  [[0, 78, 1], [1, 81, 0.5], [1.5, 83, 0.5], [2, 81, 1], [3, 78, 1]],
];
const motif = (t0, k, g) => MOTIF[k].forEach(([b, m, d]) => lead(t0 + b * 0.5, m, d * 0.5 - 0.04, g));
motif(12, 0, 0.2);
motif(14, 1, 0.2);
for (let t = 15.5; t < 16 - 1e-6; t += 0.125) snare(t, 0.25 + (t - 15.5) * 0.5);
revcym(16, 1.0, 0.3);

// ---- E. product scenes (16–40): a lighter groove that leaves room for the UI --------
function lightBar(t0, { hats = false, arp = false, stabs = false, g = 1 } = {}) {
  const ch = chordAt(t0);
  K(t0, 0.85);
  K(t0 + 1, 0.85);
  K(t0 + 1.75, 0.6);
  clap(t0 + 0.5, 0.32);
  clap(t0 + 1.5, 0.32);
  bass(t0, ch.root, 0.7, 0.45);
  bass(t0 + 0.75, ch.root, 0.2, 0.35);
  bass(t0 + 1, ch.root, 0.6, 0.45);
  bass(t0 + 1.75, ch.root + 12, 0.2, 0.35);
  pad(t0, ch.notes, 2, 0.16 * g, 600, 1000);
  if (hats) for (let e = 0; e < 8; e++) hat(t0 + e * 0.25, e % 2 ? 0.2 : 0.09, false, 0.2);
  if (arp) {
    [0, 2, 1, 3, 2, 3, 1, 2].forEach((k, e) => {
      pluck(B.music, t0 + e * 0.25, ch.notes[k] + 12, 0.2, 0.085, { pan: e % 2 ? 0.45 : -0.45, bright: 0.6 });
    });
  }
  if (stabs) for (const s of [0, 6, 11]) stab(t0 + s * 0.125, ch.notes, 0.1, 0.2, 0.55);
}
for (let t = 16; t < 40; t += 2) {
  lightBar(t, { hats: t >= 20, arp: t >= 24 && t < 36, stabs: t >= 28 });
}

// 16–20 connect: click, Tab, typing, Enter, drag, drop.
whoosh(15.85, 0.5, 0.35);
const c = PV.connect;
tick(c.click, 0.4, 88);
key(c.tab, 0.9);
ding(c.tab, 76, 0.1);
c.type.forEach((t, i) => key(t, 0.5, (i % 3 - 1) * 0.25));
key(c.enter, 1.0);
pop(c.enter, 0.3);
ding(c.enter, 81, 0.16);
whoosh(c.drag, 0.5, 0.25);
ding(c.drop, 85, 0.22);
tick(c.drop, 0.5, 92);

// 20–24 critical path: pull back, then the steps light one by one.
const cr = PV.crit;
whoosh(cr.pull - 0.1, 0.8, 0.35, -1);
cr.steps.forEach((t, i) => {
  pluck(B.sfx, t, [73, 76, 81][i], 0.5, 0.3, { bright: 1.1, rev: 0.4, dly: 0.35 });
  tick(t, 0.3, 90 + i * 2);
});
ding(cr.goal, 85, 0.28);
ding(cr.goal + 0.06, 88, 0.16, 0.3);

// 24–32 ready: pings, zoom, finish → unlock.
const rd = PV.ready;
rd.pings.forEach((t, i) => {
  pop(t, 0.22, i % 2 ? 0.3 : -0.3);
  pluck(B.sfx, t, PENTA[i + 1], 0.3, 0.16, { bright: 0.8 });
});
whoosh(rd.zoom - 0.05, 0.8, 0.4);
rd.done.forEach((d, i) => {
  tick(d.click, 0.5, 91);
  ding(d.click, [76, 78, 81][i], 0.2, -0.2);
  if (d.pop) {
    pop(d.arrive, 0.5, 0.2);
    [0, 0.045, 0.09].forEach((o, k) => ding(d.arrive + o, [81, 85, 88][k], 0.17, 0.3));
  } else {
    tick(d.arrive, 0.35, 86);
  }
});
crash(PV.T.unlock, 0.16);
key(rd.space, 0.9);
ding(rd.space, 83, 0.14);
whoosh(rd.out - 0.15, 0.6, 0.3, -1);

// 32–36 everywhere: three surfaces land, a command is typed, all of them update.
const sy = PV.sync;
sy.panels.forEach((t, i) => {
  thud(t, 0.55);
  whoosh(t - 0.18, 0.3, 0.2, i % 2 ? 1 : -1);
});
sy.type.forEach((t, i) => key(t, 0.4, (i % 3 - 1) * 0.2));
key(sy.enter, 1.0);
[0, 0.05, 0.1].forEach((o, k) => ding(sy.flash + o, [76, 81, 85][k], 0.2, [-0.5, 0, 0.5][k]));
pop(sy.flash, 0.35);

// 36–40 files and agents.
const fi = PV.files;
whoosh(fi.in - 0.15, 0.5, 0.3);
tick(fi.minus, 0.4, 80);
tick(fi.plus, 0.45, 87);
ding(fi.plus, 81, 0.14);
thud(fi.git, 0.5);
const ag = PV.agents;
whoosh(ag.in - 0.15, 0.5, 0.3, -1);
for (let i = 0; i < 8; i++) key(ag.in + 0.06 + i * 0.05, 0.4);
key(ag.enter, 1.0);
[0, 0.06, 0.12, 0.18, 0.24, 0.3].forEach((o, k) => {
  pop(ag.land + o, 0.22, (k % 3 - 1) * 0.5);
  ding(ag.land + o, PENTA[k + 2], 0.11, (k % 3 - 1) * 0.5);
});

// ---- F. build (40–44) ----------------------------------------------------------------
for (let t = 40; t < 43 - 1e-6; t += 0.5) K(t, 0.95);
for (let t = 40; t < 42 - 1e-6; t += 0.25) snare(t, 0.16 + (t - 40) * 0.06);
for (let t = 42; t < 43 - 1e-6; t += 0.125) snare(t, 0.3 + (t - 42) * 0.2);
for (let t = 43; t < 43.5 - 1e-6; t += 0.0625) snare(t, 0.5 + (t - 43) * 0.5);
PV.flashes.forEach((t, i) => tick(t, 0.2, 84 + (i % 7), (i % 2 ? 0.5 : -0.5)));
for (let t = 40; t < 43.5; t += 2) {
  const ch = chordAt(t);
  const len = Math.min(2, 43.5 - t);
  const x0 = (t - 40) / 3.5, x1 = (t + len - 40) / 3.5;
  pad(t, ch.notes.map((m) => m + 12), len - 0.3, 0.2, 500 + 5500 * x0 * x0, 500 + 5500 * x1 * x1);
  if (t < 42) for (let e = 0; e < 8; e++) bass(t + e * 0.25, ch.root, 0.2, 0.45);
}
riser(40, 3.5, 0.75, 45);
revcym(43.5, 2.0, 0.5);
ding(43.5, 88, 0.3);

// ---- G. climax (44–52) ---------------------------------------------------------------
for (let t = 44; t < 52; t += 2) {
  grooveBar(t, { arp: true, hatG: 1.25, stabG: 1.3, bassG: 1.1 });
  crash(t, t % 4 === 0 ? 0.36 : 0.22);
  motif(t, ((t - 44) / 2) % 4, 0.24);
  // Octave doubling of the arp for extra sparkle.
  const ch = chordAt(t);
  [0, 2, 1, 3, 0, 2, 1, 3].forEach((k, e) => {
    pluck(B.music, t + e * 0.25 + 0.125, ch.notes[k] + 24, 0.14, 0.06, { pan: e % 2 ? -0.6 : 0.6, bright: 1 });
  });
}
impact(44, 1.0);
impact(48, 0.7);
for (const n of PV.mega.nodes) {
  if (n.milestone) continue;
  tick(n.t, 0.13, PENTA[(n.col + Math.round(n.row + 4)) % PENTA.length] + 12, clamp(n.row / 4, -0.8, 0.8));
}
for (const t of [46, 48, 50]) {
  [0, 0.04, 0.08].forEach((o, k) => ding(t + o, [81, 85, 88][k], 0.2, [-0.4, 0, 0.4][k]));
}
for (let t = 51.5; t < 52 - 1e-6; t += 0.0625) snare(t, 0.3 + (t - 51.5) * 0.7);
revcym(52, 1.6, 0.45);

// ---- H. logo (52–58) -----------------------------------------------------------------
const lg = PV.logo;
impact(lg.hit, 1.1);
crash(lg.hit, 0.4);
K(lg.hit);
bass(lg.hit, 33, 2.4, 0.5);
supersaw(B.music, lg.hit, [45, 57, 61, 64, 71], 4.5, 0.42, (tt) => [
  Math.min(1, tt / 0.004) * Math.exp(-tt / 1.5),
  600 + 6000 * Math.exp(-tt / 0.7),
], { rev: 0.5, dly: 0.2 });
[0, 0.05, 0.1, 0.15].forEach((o, k) => ding(lg.hit + o, [81, 85, 88, 93][k], 0.2, [-0.5, -0.2, 0.2, 0.5][k]));
pluck(B.sfx, lg.tagline, 73, 0.9, 0.3, { bright: 0.8, rev: 0.5, dly: 0.4 });
pluck(B.sfx, lg.tagline + 0.25, 76, 0.9, 0.3, { bright: 0.8, rev: 0.5, dly: 0.4 });
lg.type.forEach((t, i) => key(t, 0.5, (i % 3 - 1) * 0.2));
key(lg.type[lg.type.length - 1] + 0.2, 0.9);
pop(lg.answer, 0.4);
ding(lg.answer, 81, 0.3, -0.2);
ding(lg.answer + 0.07, 88, 0.22, 0.2);
tick(lg.url, 0.35, 88);
pad(lg.answer, [57, 61, 64, 69], 2.2, 0.1, 500, 900);

// ---- mix ---------------------------------------------------------------------------------
// Sidechain: everything musical ducks under the kick.
const duck = new Float32Array(N).fill(1);
for (const tk of kicks) {
  const s = Math.round(tk * SR);
  const len = Math.round(0.26 * SR);
  for (let i = 0; i < len && s + i < N; i++) {
    const x = i / len;
    const d = 1 - 0.72 * Math.pow(1 - x, 2) * Math.min(1, i / 96);
    if (d < duck[s + i]) duck[s + i] = d;
  }
}

/** Ping-pong delay, a dotted eighth and a quarter. */
function pingPong(send) {
  const out = bus();
  const dl = Math.round(0.375 * SR), dr = Math.round(0.25 * SR);
  const bl = new Float32Array(dl), br = new Float32Array(dr);
  let il = 0, ir = 0, zl = 0, zr = 0;
  for (let i = 0; i < N; i++) {
    const yl = bl[il], yr = br[ir];
    zl += (yl - zl) * 0.45;
    zr += (yr - zr) * 0.45;
    bl[il] = (send.L[i] + send.R[i]) * 0.5 + zr * 0.42;
    br[ir] = zl * 0.6;
    out.L[i] = yl;
    out.R[i] = yr;
    il = (il + 1) % dl;
    ir = (ir + 1) % dr;
  }
  return out;
}

/** Freeverb. */
function reverb(send) {
  const out = bus();
  const scale = SR / 44100;
  const combT = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
  const apT = [556, 441, 341, 225];
  const fb = 0.87, damp = 0.32;
  for (const [ch, spread] of [["L", 0], ["R", 23]]) {
    const combs = combT.map((n) => ({ b: new Float32Array(Math.round((n + spread) * scale)), i: 0, z: 0 }));
    const aps = apT.map((n) => ({ b: new Float32Array(Math.round((n + spread) * scale)), i: 0 }));
    const hp = svf();
    hp.set(260, 0.7);
    const src = send[ch], dst = out[ch];
    for (let i = 0; i < N; i++) {
      const x = hp.run(src[i])[2] * 0.02;
      let y = 0;
      for (const cb of combs) {
        const o = cb.b[cb.i];
        cb.z = o * (1 - damp) + cb.z * damp;
        cb.b[cb.i] = x + cb.z * fb;
        cb.i = (cb.i + 1) % cb.b.length;
        y += o;
      }
      for (const ap of aps) {
        const o = ap.b[ap.i];
        ap.b[ap.i] = y + o * 0.5;
        ap.i = (ap.i + 1) % ap.b.length;
        y = o - y;
      }
      dst[i] = y;
    }
  }
  return out;
}

const dly = pingPong(B.dly);
// The delay tail feeds the reverb a little, so echoes sit in the same room.
for (let i = 0; i < N; i++) {
  B.rev.L[i] += dly.L[i] * 0.25;
  B.rev.R[i] += dly.R[i] * 0.25;
}
const rev = reverb(B.rev);

const stats = (name, b) => {
  let peak = 0, sum = 0;
  for (let i = 0; i < N; i++) {
    const a = Math.max(Math.abs(b.L[i]), Math.abs(b.R[i]));
    if (a > peak) peak = a;
    sum += b.L[i] * b.L[i] + b.R[i] * b.R[i];
  }
  console.log(name.padEnd(8), "peak", peak.toFixed(3), "rms", Math.sqrt(sum / (2 * N)).toFixed(4));
};
for (const [name, b] of Object.entries(B)) stats(name, b);
stats("dlyOut", dly);
stats("revOut", rev);

const G = { kick: 0.55, drums: 1.25, bass: 0.27, music: 1.7, sfx: 0.62, dly: 0.9, rev: 1.5, drive: 1.0 };
const mix = bus();
const hpL = svf(), hpR = svf();
hpL.set(28, 0.7);
hpR.set(28, 0.7);
const fadeIn = 0.004 * SR, fadeOut0 = (DUR - 1.6) * SR;
let peak = 0;
for (let i = 0; i < N; i++) {
  const d = duck[i];
  const dm = 0.35 + 0.65 * d;
  let l = B.kick.L[i] * G.kick + B.drums.L[i] * G.drums + B.bass.L[i] * d * G.bass + B.music.L[i] * d * G.music + B.sfx.L[i] * G.sfx + dly.L[i] * dm * G.dly + rev.L[i] * dm * G.rev;
  let r = B.kick.R[i] * G.kick + B.drums.R[i] * G.drums + B.bass.R[i] * d * G.bass + B.music.R[i] * d * G.music + B.sfx.R[i] * G.sfx + dly.R[i] * dm * G.dly + rev.R[i] * dm * G.rev;
  l = Math.tanh(hpL.run(l)[2] * G.drive);
  r = Math.tanh(hpR.run(r)[2] * G.drive);
  const fade = Math.min(1, i / fadeIn) * (i > fadeOut0 ? Math.pow(Math.max(0, 1 - (i - fadeOut0) / (N - fadeOut0)), 2) : 1);
  mix.L[i] = l * fade;
  mix.R[i] = r * fade;
  peak = Math.max(peak, Math.abs(mix.L[i]), Math.abs(mix.R[i]));
}
stats("mix", mix);
const norm = 0.9 / peak;
// Loudness per bar, to check the arc of the track at a glance.
let line = "";
for (let bar = 0; bar < DUR / 2; bar++) {
  let sum = 0;
  for (let i = bar * 2 * SR; i < (bar + 1) * 2 * SR && i < N; i++) sum += mix.L[i] ** 2 + mix.R[i] ** 2;
  line += `${bar * 2}s:${(10 * Math.log10(sum / (4 * SR) + 1e-12) + 20 * Math.log10(norm)).toFixed(1)} `;
}
console.log(line);

const pcm = Buffer.alloc(44 + N * 4);
pcm.write("RIFF", 0);
pcm.writeUInt32LE(36 + N * 4, 4);
pcm.write("WAVEfmt ", 8);
pcm.writeUInt32LE(16, 16);
pcm.writeUInt16LE(1, 20);
pcm.writeUInt16LE(2, 22);
pcm.writeUInt32LE(SR, 24);
pcm.writeUInt32LE(SR * 4, 28);
pcm.writeUInt16LE(4, 32);
pcm.writeUInt16LE(16, 34);
pcm.write("data", 36);
pcm.writeUInt32LE(N * 4, 40);
for (let i = 0; i < N; i++) {
  pcm.writeInt16LE(Math.round(clamp(mix.L[i] * norm, -1, 1) * 32767), 44 + i * 4);
  pcm.writeInt16LE(Math.round(clamp(mix.R[i] * norm, -1, 1) * 32767), 46 + i * 4);
}
writeFileSync(process.argv[2], pcm);
console.log("wrote", process.argv[2]);
