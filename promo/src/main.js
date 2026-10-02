// Composes the scenes into frames. `PV.render(t)` draws the frame at time `t`;
// the live preview and the offline renderer both call it.
(function (g) {
  const PV = g.PV;
  const E = PV.E;
  const { W, H } = E;
  const T = PV.T;
  let out = null;
  let work = null;

  function draw(c, t) {
    c.setTransform(1, 0, 0, 1, 0, 0);
    c.globalAlpha = 1;
    c.filter = "none";
    c.globalCompositeOperation = "source-over";
    c.fillStyle = E.C.bg;
    c.fillRect(0, 0, W, H);
    const S = PV.scenes;
    if (t < T.drop) S.hook(c, t);
    else if (t < T.connect) {
      S.reveal(c, t);
      if (t >= T.title) S.title(c, t);
      // The app window is already rising while the title leaves.
      if (t >= T.connect - 0.4) S.product(c, t);
    } else if (t < T.build) S.product(c, t);
    else if (t < T.climax) S.build(c, t);
    else if (t < T.logo) S.climax(c, t);
    else S.logo(c, t);
    const fade = E.prog(t, T.end - 0.7, 0.6);
    if (fade > 0) {
      c.fillStyle = `rgba(0,0,0,${fade})`;
      c.fillRect(0, 0, W, H);
    }
  }

  PV.init = async function (canvas, { base = "", preview = false } = {}) {
    canvas.width = W;
    canvas.height = H;
    out = canvas.getContext("2d");
    work = new OffscreenCanvas(W, H).getContext("2d");
    E.A.preview = preview;
    await E.loadData(base);
    if (preview) {
      for (const name of Object.keys(E.A.data.shots)) if (!name.startsWith("s_")) E.shot(name);
      await E.settle();
    }
  };

  /** Draws the frame at `t`; `samples` > 1 averages sub-frames across the shutter for motion blur. */
  PV.render = async function (t, { samples = 1, shutter = 0.6, fps = 60 } = {}) {
    E.A.tick++;
    const pass = () => {
      if (samples <= 1) {
        draw(out, t);
        return;
      }
      for (let k = 0; k < samples; k++) {
        draw(work, Math.max(0, t + ((k + 0.5) / samples - 0.5) * (shutter / fps)));
        out.setTransform(1, 0, 0, 1, 0, 0);
        out.filter = "none";
        out.globalCompositeOperation = "source-over";
        out.globalAlpha = 1 / (k + 1);
        out.drawImage(work.canvas, 0, 0);
      }
      out.globalAlpha = 1;
    };
    pass();
    // A frame can ask for images it only finds out about once others have loaded.
    while (E.A.pending.size) {
      await E.settle();
      pass();
    }
    await E.settle();
    E.post(out, t);
  };
})(typeof window !== "undefined" ? window : globalThis);
