//! The browser bridge: one universe, running in a tab, exposed for drawing.
//!
//! This crate is deliberately thin. It owns no physics and decides nothing
//! about how a universe behaves — every rule it runs comes from `universe-core`
//! unchanged, which is why the fingerprint check in [`golden_fingerprint`] is
//! worth anything. A bridge that reimplemented the laws for speed would be
//! comparing two different universes and calling their agreement determinism.
//!
//! What it adds is the small amount of bookkeeping a viewer needs and a
//! simulation does not: which blocks changed fidelity this tick, so that
//! rendering and collapse can be *seen* rather than inferred.
//!
//! # Why changing a limit restarts the universe
//!
//! Relaxing `discrete_space` subdivides the grid, which changes the world's
//! dimensions — there is no continuous edit that turns one into the other. So
//! toggling any limit rebuilds the world from the same seed. That is not a
//! limitation worked around; it is the honest behaviour. `World::seed` draws the
//! pattern at base resolution and upsamples it, so the coarse universe and the
//! subdivided one begin from the same macro configuration, exactly as the v0.1
//! experiment requires. A visitor toggling a limit sees the same universe under
//! different physics, not a different universe.

use universe_core::constraints::{Constraints, Params, Resolved};
use universe_core::observer::{Probe, observe};
use universe_core::physics::{Rules, Work, tick};
use universe_core::space::{Geometry, World};
use wasm_bindgen::prelude::*;

/// The cross-target determinism check, callable from the page.
///
/// Returns the reference universe's fingerprint as a string, because a `u64`
/// crosses into JavaScript as a `BigInt` and this number exists to be displayed
/// and compared, not computed with.
#[wasm_bindgen]
pub fn golden_fingerprint() -> String {
    universe_core::golden::golden_fingerprint().to_string()
}

/// What the native build produced, compiled in so the page can show both.
#[wasm_bindgen]
pub fn golden_expected() -> String {
    universe_core::golden::GOLDEN_FINGERPRINT.to_string()
}

/// A universe, plus the bookkeeping a viewer needs.
#[wasm_bindgen]
pub struct Sim {
    base_w: usize,
    base_h: usize,
    seed: u64,
    density: f64,

    constraints: Constraints,
    params: Params,
    rules: Rules,
    probe: Probe,

    res: Resolved,
    world: World,
    tick_no: u64,

    /// Blocks that gained cells this tick, and blocks that lost them. Cleared
    /// and refilled on every step so the page can flash exactly what changed.
    rendered: Vec<u32>,
    collapsed: Vec<u32>,
    work: Work,
}

#[wasm_bindgen]
impl Sim {
    /// Build a universe. `base_w`/`base_h` are in base cells; relaxing
    /// `discrete_space` subdivides from there.
    #[wasm_bindgen(constructor)]
    pub fn new(base_w: usize, base_h: usize, seed: f64, density: f64, block_size: usize) -> Sim {
        let params = Params {
            subdivision: 2,
            substeps: 2,
            capped_radius: 1,
            uncapped_radius: 3,
            block_size: block_size.max(2),
        };
        let constraints = Constraints::ALL_ON;
        let probe = Probe {
            x: base_w / 4,
            y: base_h / 4,
            width: base_w / 2,
            height: base_h / 2,
        };
        let res = Resolved::new(&constraints, &params);
        let geom = Geometry::new(base_w, base_h, res.subdivision, res.block_size);
        let world = World::seed(geom, seed as u64, density);

        Sim {
            base_w,
            base_h,
            seed: seed as u64,
            density,
            constraints,
            params,
            rules: Rules::default(),
            probe,
            res,
            world,
            tick_no: 0,
            rendered: Vec::new(),
            collapsed: Vec::new(),
            work: Work::default(),
        }
    }

    /// Rebuild from the current settings, back at tick zero.
    pub fn reset(&mut self) {
        self.res = Resolved::new(&self.constraints, &self.params);
        let geom = Geometry::new(
            self.base_w,
            self.base_h,
            self.res.subdivision,
            self.res.block_size,
        );
        self.world = World::seed(geom, self.seed, self.density);
        self.tick_no = 0;
        self.rendered.clear();
        self.collapsed.clear();
        self.work = Work::default();
    }

    /// Which limits are in force. Rebuilds, for the reason in the module docs.
    pub fn set_limits(&mut self, space: bool, time: bool, speed: bool, lazy: bool) {
        self.constraints = Constraints {
            discrete_space: space,
            discrete_time: time,
            speed_cap: speed,
            lazy_rendering: lazy,
        };
        self.reset();
    }

    /// The dials behind the limits. `substeps` and `uncapped_radius` are the
    /// two the page exposes: they multiply into one another, and a visitor
    /// moving either sees the same influence figure move.
    pub fn set_dials(&mut self, substeps: usize, uncapped_radius: usize, subdivision: usize) {
        self.params.substeps = substeps.clamp(1, 8);
        self.params.uncapped_radius = uncapped_radius.clamp(1, 6);
        self.params.subdivision = subdivision.clamp(1, 4);
        self.reset();
    }

    pub fn set_seed(&mut self, seed: f64) {
        self.seed = seed as u64;
        self.reset();
    }

    pub fn set_density(&mut self, density: f64) {
        self.density = density.clamp(0.0, 1.0);
        self.reset();
    }

    /// Move the probe, in base cells. Does not rebuild: where something looks
    /// is not a physical constant.
    pub fn set_probe(&mut self, x: usize, y: usize, w: usize, h: usize) {
        self.probe = Probe {
            x: x.min(self.base_w.saturating_sub(1)),
            y: y.min(self.base_h.saturating_sub(1)),
            width: w.clamp(1, self.base_w),
            height: h.clamp(1, self.base_h),
        };
    }

    /// One tick: observe, then apply the laws.
    pub fn step(&mut self) {
        let before: Vec<bool> = self.world.resolved.clone();

        let (observed, render_work) = observe(
            &self.world,
            &self.probe,
            self.tick_no,
            self.seed,
            self.res.lazy,
        );
        let (advanced, physics_work) = tick(&observed, &self.rules, &self.res);

        self.rendered.clear();
        self.collapsed.clear();
        for (b, was) in before.iter().enumerate() {
            match (*was, observed.resolved[b]) {
                (false, true) => self.rendered.push(b as u32),
                (true, false) => self.collapsed.push(b as u32),
                _ => {}
            }
        }

        self.work = Work::default();
        self.work.add(render_work);
        self.work.add(physics_work);
        self.world = advanced;
        self.tick_no += 1;
    }

    // ---- geometry ------------------------------------------------------

    pub fn width(&self) -> usize {
        self.world.geom.w
    }
    pub fn height(&self) -> usize {
        self.world.geom.h
    }
    pub fn blocks_w(&self) -> usize {
        self.world.geom.bw
    }
    pub fn blocks_h(&self) -> usize {
        self.world.geom.bh
    }
    pub fn block_edge(&self) -> usize {
        self.world.geom.block
    }
    pub fn scale(&self) -> usize {
        self.world.geom.scale
    }

    // ---- state for drawing ---------------------------------------------

    /// Per-cell occupancy. Meaningful only inside resolved blocks; the page
    /// draws the others from `coarse` instead, which is the whole point.
    pub fn cells(&self) -> Vec<u8> {
        self.world.cells.clone()
    }

    /// Which blocks are computed per-cell right now.
    pub fn resolved(&self) -> Vec<u8> {
        self.world.resolved.iter().map(|r| u8::from(*r)).collect()
    }

    /// Per-block density. What an unresolved block has instead of cells.
    pub fn coarse(&self) -> Vec<f64> {
        self.world.coarse.clone()
    }

    /// Blocks forced into existence this tick.
    pub fn rendered_blocks(&self) -> Vec<u32> {
        self.rendered.clone()
    }

    /// Blocks that stopped being computed this tick.
    pub fn collapsed_blocks(&self) -> Vec<u32> {
        self.collapsed.clone()
    }

    /// The probe, in fine-grid cells: `[x, y, width, height]`.
    pub fn probe_rect(&self) -> Vec<u32> {
        let s = self.world.geom.scale as u32;
        vec![
            self.probe.x as u32 * s,
            self.probe.y as u32 * s,
            self.probe.width as u32 * s,
            self.probe.height as u32 * s,
        ]
    }

    // ---- readouts -------------------------------------------------------

    pub fn tick_count(&self) -> u64 {
        self.tick_no
    }
    pub fn live_fraction(&self) -> f64 {
        self.world.live_fraction()
    }
    /// Neighbour visits spent on the last tick. The reproducible cost counter.
    pub fn neighbor_visits(&self) -> u64 {
        self.work.neighbor_visits
    }
    pub fn cells_rendered(&self) -> u64 {
        self.work.cells_rendered
    }
    /// Base cell lengths influence covers per tick: `radius * substeps /
    /// subdivision`. The number two separate dials both move.
    pub fn influence_speed(&self) -> f64 {
        self.res.influence_speed()
    }
    pub fn radius(&self) -> usize {
        self.res.radius
    }
    pub fn substeps(&self) -> usize {
        self.res.substeps
    }
    pub fn resolved_blocks(&self) -> usize {
        self.world.resolved.iter().filter(|r| **r).count()
    }
    pub fn total_blocks(&self) -> usize {
        self.world.geom.blocks()
    }
    /// What a resource-honest implementation would be holding.
    pub fn live_state_bytes(&self) -> usize {
        self.world.live_state_bytes()
    }
    /// What this one actually allocates. Both are reported, as on the CLI.
    pub fn allocated_bytes(&self) -> usize {
        self.world.allocated_bytes()
    }
    pub fn fingerprint(&self) -> String {
        self.world.fingerprint().to_string()
    }
}
