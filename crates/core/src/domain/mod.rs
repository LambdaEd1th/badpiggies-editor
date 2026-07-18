//! Pure domain layer: data types, level (de)serialization, geometry generation.
//!
//! Must not depend on frontend frameworks or platform I/O.

pub mod level;
pub mod level_warning;
pub mod object_deserializer;
pub mod parser;
pub mod prefab_asset;
pub mod prefab_override;
pub mod terrain_gen;
pub mod terrain_prefab;
pub mod types;
