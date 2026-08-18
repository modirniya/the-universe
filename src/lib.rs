//! **the-universe** — executable philosophy.
//!
//! A runnable model of a simulation-hypothesis framework. Each module
//! implements one theory from `docs/philosophy.md`, and its docs say which one
//! and what would falsify it *within the model*.
//!
//! What running this proves: that the ideas are **coherent** — that they can be
//! made to work together in a system that actually executes. It does not prove
//! our universe works this way. The model makes falsifiable predictions only
//! about its own behaviour.
//!
//! # The v0.1 argument
//!
//! Theory 1 says physical limits are resource optimizations rather than
//! fundamental truths. That is testable inside a toy: build a universe, run it
//! with the limits in force and without them, and see whether the limits make
//! it cheaper without changing what an outside observer could see.
//!
//! - [`constraints`] — the four limits, as toggles
//! - [`space`] — discrete space, and two-fidelity storage
//! - [`physics`] — the laws, as pure functions
//! - [`observer`] — probes, and the render/collapse events
//! - [`experiment`] — the benchmark that compares constrained to unconstrained
//! - [`report`] — CSV, JSON and a summary that declines to overstate the result
//! - [`rng`] — the creator's input channel; the reason runs are reproducible
//!
//! # Layers
//!
//! Layer 0 is the host process. v0.1 builds a single layer and stops there;
//! nesting, pipes, and everything that follows from them are v0.2 and later.

pub mod config;
pub mod constraints;
pub mod experiment;
pub mod observer;
pub mod physics;
pub mod report;
pub mod rng;
pub mod space;
