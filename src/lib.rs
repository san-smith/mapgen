//! Procedural world generator for Chronicles of Realms
//!
//! This library provides pure Rust implementations for generating:
//! - Heightmaps
//! - Climate and biomes
//! - Provinces and regions
//! - Rivers and strategic points
//!
//! All functions are deterministic and seed-based.
//! For CLI usage, see the `mapgen-cli` binary.
pub mod biome;
pub mod climate;
pub mod config;
pub mod heightmap;
pub mod province;
pub mod region;
pub mod rivers;
pub mod strategic;

// === Конфигурация ===
pub use config::{
    ClimateSettings, IslandSettings, TerrainSettings, WorldGenerationParams, WorldType,
};

// === Карта высот ===
pub use heightmap::{Heightmap, generate_heightmap};

// === Биомы ===
pub use biome::{Biome, BiomeMap, assign_biomes};

// === Климат ===
pub use climate::{calculate_humidity, generate_climate_maps};

// === Вода ===
pub use province::water::{WaterType, classify_water};

// === Провинции ===
pub use province::{
    Province, ProvinceType,
    generator::{generate_province_seeds, generate_provinces_from_seeds},
    graph::build_province_graph_with_map,
};

// === Регионы ===
pub use region::{Region, group_provinces_into_regions};

// === Реки ===
pub use rivers::{RiverMap, generate_rivers};

// === Стратегические точки ===
pub use strategic::{StrategicPoint, find_strategic_points};
