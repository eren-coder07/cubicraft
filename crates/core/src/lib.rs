//! # cubicraft-core
//!
//! Clean-room Live2D-style 2D character animation engine in pure Rust.
//!
//! Pipeline: layered PSD -> texture parts -> deformable mesh tree -> spring-damper
//! physics -> software rasterizer. Zero rendering dependencies; everything from
//! PSD parsing to PNG encoding is implemented in this crate.

pub mod anim;
pub mod app;
pub mod math;
pub mod model;
pub mod physics;
pub mod png;
pub mod psd;
pub mod render;
pub mod rig;
