// the-universe — viewer.
//
// This file draws and wires controls. It contains no physics: every rule comes
// from the Rust core compiled to WebAssembly, which is the same code the CLI
// runs and the same code the cross-target fingerprint check compares. A viewer
// that reimplemented a cheap version of the laws for the browser would be
// showing a different universe from the one the findings describe.

import init, { Sim, golden_fingerprint, golden_expected } from './pkg/universe_web.js';

// Default world. Deliberately smaller than the 128x128 the CLI experiments use:
// this one has to stay smooth in a tab while a person fiddles with it, and the
// reference size is a setting rather than the default.
const BASE = 96;
const BLOCK = 16;

const el = (id) => document.getElementById(id);

const canvas = el('world');
const ctx = canvas.getContext('2d', { alpha: false });

// Off-screen buffer at world resolution, scaled up with smoothing off so a cell
// is a crisp square rather than a blur.
const off = document.createElement('canvas');
const offCtx = off.getContext('2d', { willReadFrequently: true });
let img = null;

let sim = null;
let playing = false;
let rate = 20;
let acc = 0;
let last = 0;

// Blocks that changed fidelity recently, so the change can be seen. Value is
// the number of frames of fade remaining.
const flashes = new Map();
const FLASH_FRAMES = 26;

// --- palette, read from the stylesheet so the two cannot drift -----------

function cssRGB(name, fallback) {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
  const m = v.match(/^#?([0-9a-f]{6})$/i);
  if (!m) return [0, 0, 0];
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

let DEAD, LIVE, COARSE, ACCENT, CONFIRMED, CORRECTED;

function readPalette() {
  DEAD = cssRGB('--c-dead', '#ffffff');
  LIVE = cssRGB('--c-live', '#14181d');
  COARSE = cssRGB('--c-coarse', '#a8620a');
  ACCENT = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() || '#a8620a';
  CONFIRMED = getComputedStyle(document.documentElement).getPropertyValue('--confirmed').trim() || '#0e6b62';
  CORRECTED = getComputedStyle(document.documentElement).getPropertyValue('--corrected').trim() || '#9c3524';
}

// --- drawing --------------------------------------------------------------

function draw() {
  const w = sim.width();
  const h = sim.height();

  if (off.width !== w || off.height !== h) {
    off.width = w;
    off.height = h;
    img = offCtx.createImageData(w, h);
  }

  const cells = sim.cells();
  const resolved = sim.resolved();
  const coarse = sim.coarse();
  const bw = sim.blocks_w();
  const be = sim.block_edge();
  const data = img.data;

  for (let y = 0; y < h; y++) {
    const by = (y / be) | 0;
    const row = y * w;
    for (let x = 0; x < w; x++) {
      const b = by * bw + ((x / be) | 0);
      const i = (row + x) * 4;

      if (resolved[b]) {
        // Being computed: draw the cell itself.
        const c = cells[row + x] === 1 ? LIVE : DEAD;
        data[i] = c[0];
        data[i + 1] = c[1];
        data[i + 2] = c[2];
      } else {
        // Not being computed: one density standing in for the whole region.
        // A faint floor tint marks the region as coarse even where it is nearly
        // empty -- an empty coarse block and empty computed ground really are
        // indistinguishable in the model, so the viewer has to say which is
        // which itself.
        const a = 0.12 + 0.78 * coarse[b];
        data[i] = DEAD[0] + (COARSE[0] - DEAD[0]) * a;
        data[i + 1] = DEAD[1] + (COARSE[1] - DEAD[1]) * a;
        data[i + 2] = DEAD[2] + (COARSE[2] - DEAD[2]) * a;
      }
      data[i + 3] = 255;
    }
  }

  offCtx.putImageData(img, 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(off, 0, 0, canvas.width, canvas.height);

  const px = canvas.width / w;

  // Fidelity changes, fading out.
  for (const [b, left] of flashes) {
    const bx = b % bw;
    const by = (b / bw) | 0;
    ctx.globalAlpha = left / FLASH_FRAMES;
    ctx.strokeStyle = flashKind.get(b) === 'r' ? CONFIRMED : CORRECTED;
    ctx.lineWidth = 2;
    ctx.strokeRect(bx * be * px + 1, by * be * px + 1, be * px - 2, be * px - 2);
  }
  ctx.globalAlpha = 1;

  // The probe. Drawn last so it is never obscured.
  const [px0, py0, pw, ph] = sim.probe_rect();
  ctx.strokeStyle = ACCENT;
  ctx.lineWidth = 2;
  ctx.setLineDash([6, 4]);
  ctx.strokeRect(px0 * px, py0 * px, pw * px, ph * px);
  ctx.setLineDash([]);
}

const flashKind = new Map();

function noteFidelityChanges() {
  for (const b of sim.rendered_blocks()) {
    flashes.set(b, FLASH_FRAMES);
    flashKind.set(b, 'r');
  }
  for (const b of sim.collapsed_blocks()) {
    flashes.set(b, FLASH_FRAMES);
    flashKind.set(b, 'c');
  }
}

function fadeFlashes() {
  for (const [b, left] of flashes) {
    if (left <= 1) {
      flashes.delete(b);
      flashKind.delete(b);
    } else {
      flashes.set(b, left - 1);
    }
  }
}

// --- readouts -------------------------------------------------------------

function bytes(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}

function updateReadouts() {
  el('r-tick').textContent = sim.tick_count();
  el('r-live').textContent = sim.live_fraction().toFixed(3);
  el('r-work').textContent = sim.neighbor_visits().toLocaleString();
  el('r-blocks').textContent = `${sim.resolved_blocks()} / ${sim.total_blocks()}`;
  el('r-live-bytes').textContent = bytes(sim.live_state_bytes());
  el('r-alloc-bytes').textContent = bytes(sim.allocated_bytes());

  el('coupling').textContent = sim.influence_speed().toFixed(2);
  const [, timeOn, speedOn] = limits();
  const pinned = [speedOn ? 'the speed cap' : null, timeOn ? 'discrete time' : null].filter(Boolean);
  el('couplingWhy').textContent =
    `radius ${sim.radius()} × substeps ${sim.substeps()}` +
    (pinned.length ? ` — held there by ${pinned.join(' and ')}` : ' — both dials free');
}

// --- loop -----------------------------------------------------------------

function frame(now) {
  if (last === 0) last = now;
  const dt = (now - last) / 1000;
  last = now;

  if (playing) {
    acc += dt * rate;
    let steps = 0;
    while (acc >= 1 && steps < 8) {
      sim.step();
      noteFidelityChanges();
      acc -= 1;
      steps += 1;
    }
    if (steps > 0) updateReadouts();
  }

  fadeFlashes();
  draw();
  requestAnimationFrame(frame);
}

// --- controls -------------------------------------------------------------

function limits() {
  return [el('lim-space').checked, el('lim-time').checked, el('lim-speed').checked, el('lim-lazy').checked];
}

function applyLimits() {
  const [s, t, sp, lz] = limits();
  sim.set_limits(s, t, sp, lz);
  restart();
}

// Everything that changes state ends here: clear the fidelity flashes, refresh
// the numbers, and repaint now. Waiting for the next animation frame would mean
// a paused visitor clicking Step saw nothing happen.
function restart() {
  flashes.clear();
  flashKind.clear();
  updateReadouts();
  syncDialState();
  draw();
}

// A dial whose limit is in force does nothing, and silently doing nothing is
// the worst way to teach that. Say which limit is pinning it.
function syncDialState() {
  const [, timeOn, speedOn] = limits();
  el('substepsWrap').classList.toggle('inactive', timeOn);
  el('radiusWrap').classList.toggle('inactive', speedOn);
}

function applyDials() {
  const substeps = Number(el('substeps').value);
  const radius = Number(el('radius').value);
  el('substepsOut').textContent = substeps;
  el('radiusOut').textContent = radius;
  sim.set_dials(substeps, radius, 2);
  restart();
}

function wire() {
  el('play').addEventListener('click', () => {
    playing = !playing;
    el('play').textContent = playing ? 'Pause' : 'Play';
  });

  el('stepBtn').addEventListener('click', () => {
    sim.step();
    noteFidelityChanges();
    updateReadouts();
    draw();
  });

  el('resetBtn').addEventListener('click', () => {
    sim.reset();
    restart();
  });

  el('rate').addEventListener('input', (e) => {
    rate = Number(e.target.value);
    el('rateOut').textContent = rate;
  });

  for (const id of ['lim-space', 'lim-time', 'lim-speed', 'lim-lazy']) {
    el(id).addEventListener('change', applyLimits);
  }
  el('substeps').addEventListener('input', applyDials);
  el('radius').addEventListener('input', applyDials);

  el('seed').addEventListener('change', (e) => {
    const v = Number(e.target.value);
    sim.set_seed(Number.isFinite(v) && v >= 0 ? v : 42);
    restart();
  });

  el('density').addEventListener('input', (e) => {
    const d = Number(e.target.value) / 100;
    el('densityOut').textContent = d.toFixed(2);
    sim.set_density(d);
    restart();
  });

  // Click the world to move the probe. Where something looks is not a physical
  // constant, so this does not restart the universe.
  canvas.addEventListener('click', (e) => {
    const r = canvas.getBoundingClientRect();
    const fx = ((e.clientX - r.left) / r.width) * sim.width();
    const fy = ((e.clientY - r.top) / r.height) * sim.height();
    const s = sim.scale();
    const [, , pw, ph] = sim.probe_rect();
    const bw = Math.max(1, Math.round(pw / s));
    const bh = Math.max(1, Math.round(ph / s));
    sim.set_probe(
      Math.max(0, Math.round(fx / s - bw / 2)),
      Math.max(0, Math.round(fy / s - bh / 2)),
      bw,
      bh,
    );
    draw();
  });

  const mq = window.matchMedia('(prefers-color-scheme: dark)');
  mq.addEventListener('change', readPalette);
}

// --- the cross-target check ----------------------------------------------

function showGolden() {
  const here = golden_fingerprint();
  const native = golden_expected();
  el('g-here').textContent = here;
  el('g-native').textContent = native;
  const v = el('g-verdict');
  if (here === native) {
    v.textContent = 'match — the value stream survived the change of platform';
    v.className = 'verdict match';
  } else {
    // Recorded, not hidden. A disagreement here is a finding about the
    // project's first rule.
    v.textContent = 'DIFFERENT — determinism did not survive this platform';
    v.className = 'verdict differ';
  }
}

// --- start ----------------------------------------------------------------

async function main() {
  try {
    await init();
  } catch (err) {
    document.querySelector('.viewport').innerHTML =
      '<p class="shading-note">The WebAssembly module did not load. Build it with ' +
      '<code>wasm-pack build --target web --out-dir web/pkg crates/universe-web</code>.</p>';
    console.error(err);
    return;
  }

  readPalette();
  sim = new Sim(BASE, BASE, 42, 0.3, BLOCK);
  wire();
  showGolden();
  updateReadouts();
  syncDialState();
  requestAnimationFrame(frame);
}

main();
