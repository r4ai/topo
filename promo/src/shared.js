// Timeline shared by the picture (browser) and the soundtrack (node): every
// hit the audio plays is an event the picture animates, so the two cannot drift.
// 120 BPM: a beat is 0.5 s (30 frames at 60 fps), a bar is 2 s.
(function (g) {
  const BEAT = 0.5;
  const BAR = 2;
  const S16 = 0.125;

  /** Scene starts, in seconds. */
  const T = {
    hook: 0,
    stop: 4,
    tense: 6,
    drop: 8,
    graph: 10,
    title: 12,
    connect: 16,
    crit: 20,
    ready: 24,
    unlock: 28,
    sync: 32,
    files: 36,
    agents: 38,
    build: 40,
    climax: 44,
    logo: 52,
    end: 58,
  };

  function rng(seed) {
    let a = seed >>> 0;
    return function () {
      a = (a + 0x6d2b79f5) >>> 0;
      let t = a;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  // ---- hook: a flat list piling up ---------------------------------------
  const LIST = [
    "Ship v1.0",
    "Build API",
    "Write docs",
    "Design UI",
    "Set up CI",
    "Integration tests",
    "Design DB schema",
    "Build frontend",
    "Write auth spec",
    "Define requirements",
    "Fix flaky test",
    "Reply to Sam",
  ];
  const FLOOD = [
    "Update changelog", "Review PR #42", "Rotate API keys", "Plan sprint", "Refactor auth", "Call vendor",
    "Draft pricing page", "Set up analytics", "Record demo", "Write onboarding emails", "Publish blog post",
    "Renew domain", "Load testing", "Migrate database", "Add SSO login", "Billing & plans", "Fix nav bug",
    "Triage inbox", "Book venue", "Update roadmap", "Security review", "Backup strategy", "Dark mode",
    "Hire designer", "Write tests",
  ];
  const rowTimes = [0, 0.5, 1, 1.5, 2, 2.25, 2.5, 2.75, 3, 3.125, 3.25, 3.375];
  const floodTimes = [];
  for (let i = 0; i < FLOOD.length; i++) floodTimes.push(3.5 + (i * 0.5) / FLOOD.length);
  const words = [
    { t: 5.0, w: "Where" },
    { t: 5.25, w: "do" },
    { t: 5.5, w: "you" },
    { t: 5.75, w: "start?" },
  ];
  const heart = [6, 6.5, 7, 7.25, 7.5, 7.625];

  // ---- product scenes -----------------------------------------------------
  const TYPE_TITLE = "Write docs";
  const connect = {
    in: 16,
    click: 16.5,
    tab: 17,
    type: Array.from({ length: TYPE_TITLE.length }, (_, i) => 17.25 + i * S16),
    enter: 18.5,
    drag: 19,
    drop: 19.5,
  };
  const crit = { pull: 20, land: 20.75, steps: [21, 21.5, 22], goal: 22.5 };
  const ready = {
    in: 24,
    pings: [25, 25.25, 25.5, 25.75],
    zoom: 26,
    zoomDur: 0.75,
    // Finishing `node` at `click` reaches `to` at `arrive`, which then shows `state`.
    done: [
      { node: "db", click: 27, to: "api", arrive: 27.25, shot: "r_1", pop: false },
      { node: "auth", click: 27.5, to: "api", arrive: 28, shot: "r_2", pop: true },
      { node: "ui", click: 29, to: "fe", arrive: 29.5, shot: "r_3", pop: true },
    ],
    space: 30,
    out: 31,
  };
  const SYNC_CMD = "topo status api done";
  const sync = {
    panels: [32, 32.5, 33],
    type: Array.from({ length: SYNC_CMD.length }, (_, i) => 33 + i * 0.045),
    enter: 34,
    flash: 34,
    view: 35,
  };
  const files = { in: 36, minus: 36.5, plus: 36.75, git: 37 };
  const agents = { in: 38, enter: 38.5, land: 39 };

  // ---- build: words flashing faster and faster ----------------------------
  const BUILD_WORDS = [
    "READY", "BLOCKED", "DONE", "CRITICAL PATH", "MILESTONES", "DEPENDS ON", "MARKDOWN", "GIT",
    "CLI", "TUI", "GUI", "AGENTS", "LOCAL-FIRST", "DAG", "TAB", "DRAG", "LINK", "APPLY", "UNDO", "SEARCH",
    "RUST", "GPUI", "JSON", "NO CYCLES", "ONE GRAPH", "TOPO",
  ];
  const flashes = [];
  for (let t = 40; t < 42 - 1e-6; t += 0.25) flashes.push(t);
  for (let t = 42; t < 43 - 1e-6; t += 0.125) flashes.push(t);
  for (let t = 43; t < 43.5 - 1e-6; t += 0.0625) flashes.push(t);
  const buildGap = 43.5;

  // ---- climax: a big graph completing in topological order ----------------
  const MEGA_COLS = 16;
  const mega = (function () {
    const r = rng(7);
    const nodes = [];
    const byCol = [];
    for (let c = 0; c < MEGA_COLS; c++) {
      const milestone = c % 4 === 3;
      const count = milestone ? 1 : 4 + Math.floor(r() * 4);
      const col = [];
      for (let k = 0; k < count; k++) {
        const row = milestone ? 0 : k - (count - 1) / 2 + (r() - 0.5) * 0.3;
        const n = { i: nodes.length, col: c, row, milestone, deps: [], t: 0 };
        nodes.push(n);
        col.push(n);
      }
      byCol.push(col);
    }
    for (let c = 1; c < MEGA_COLS; c++) {
      for (const n of byCol[c]) {
        const prev = byCol[c - 1];
        if (n.milestone) {
          n.deps = prev.map((p) => p.i);
          continue;
        }
        // Nearest rows of the previous column, so edges stay short.
        const sorted = prev.slice().sort((a, b) => Math.abs(a.row - n.row) - Math.abs(b.row - n.row));
        const k = prev.length === 1 ? 1 : 1 + (r() < 0.55 ? 1 : 0);
        n.deps = sorted.slice(0, k).map((p) => p.i);
        if (c >= 2 && r() < 0.18 && !byCol[c - 2][0].milestone) {
          const far = byCol[c - 2];
          n.deps.push(far[Math.floor(r() * far.length)].i);
        }
      }
    }
    // Each column completes inside its own beat; rows ripple on sixteenths.
    for (const n of nodes) {
      const bar = Math.floor(n.col / 4);
      const base = T.climax + bar * BAR + (n.col % 4) * BEAT;
      if (n.milestone) {
        n.t = T.climax + (bar + 1) * BAR;
      } else {
        const order = byCol[n.col].slice().sort((a, b) => a.row - b.row).indexOf(n);
        n.t = base + (order % 4) * S16;
      }
    }
    return { nodes, byCol };
  })();
  const climaxWords = [
    { t: 44, w: "PLAN" },
    { t: 46, w: "SEE" },
    { t: 48, w: "UNBLOCK" },
    { t: 50, w: "SHIP" },
  ];

  // ---- outro ---------------------------------------------------------------
  const END_CMD = "topo ready";
  const logo = {
    hit: 52,
    tagline: 53,
    type: Array.from({ length: END_CMD.length }, (_, i) => 54 + i * 0.07),
    answer: 55,
    url: 55.5,
  };

  g.PV = {
    BEAT, BAR, S16, T, rng,
    LIST, FLOOD, rowTimes, floodTimes, words, heart,
    TYPE_TITLE, connect, crit, ready, SYNC_CMD, sync, files, agents,
    BUILD_WORDS, flashes, buildGap, MEGA_COLS, mega, climaxWords, END_CMD, logo,
  };
})(typeof window !== "undefined" ? window : globalThis);
