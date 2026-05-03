//! Pure domain layer: data types, level (de)serialization, geometry generation.
//!
//! Must not depend on `egui`, `wgpu`, or any I/O.

pub mod level;
pub mod parser;
pub mod terrain_gen;
pub mod types;
