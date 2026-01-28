use serde::{Deserialize, Serialize};
use std::fs;

/// Тип генерируемого мира
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum WorldType {
    EarthLike,
    Supercontinent,
    Archipelago,
    Mediterranean,
    IceAgeEarth,
    DesertMediterranean,
}

impl WorldType {
    /// Целевая доля суши (0.0 = всё море, 1.0 = вся суша)
    pub fn target_land_ratio(self) -> f32 {
        match self {
            WorldType::EarthLike => 0.40,
            WorldType::Supercontinent => 0.70,
            WorldType::Archipelago => 0.15,
            WorldType::Mediterranean => 0.25,
            WorldType::IceAgeEarth => 0.35, // больше льда = больше "суши", но непригодной
            WorldType::DesertMediterranean => 0.20,
        }
    }

    pub fn default_terrain(&self) -> TerrainSettings {
        match self {
            WorldType::Supercontinent | WorldType::Mediterranean => TerrainSettings {
                elevation_power: 0.65,
                smooth_radius: 2, // сильное сглаживание
            },
            WorldType::Archipelago => TerrainSettings {
                elevation_power: 1.5,
                smooth_radius: 1,
            },
            _ => TerrainSettings::default(), // EarthLike, IceAgeEarth и др.
        }
    }
}

impl Default for WorldType {
    fn default() -> Self {
        WorldType::EarthLike
    }
}

/// Глобальные климатические модификаторы
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateSettings {
    #[serde(default = "default_temperature_offset")]
    pub global_temperature_offset: f32,

    #[serde(default = "default_humidity_offset")]
    pub global_humidity_offset: f32,

    #[serde(default = "default_polar_amplification")]
    pub polar_amplification: f32,
}

fn default_temperature_offset() -> f32 {
    0.0
}
fn default_humidity_offset() -> f32 {
    0.0
}
fn default_polar_amplification() -> f32 {
    1.0
}

impl Default for ClimateSettings {
    fn default() -> Self {
        Self {
            global_temperature_offset: 0.0,
            global_humidity_offset: 0.0,
            polar_amplification: 1.0,
        }
    }
}

/// Настройки островов в океане
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandSettings {
    #[serde(default = "default_island_density")]
    pub island_density: f32,

    #[serde(default = "default_min_island_size")]
    pub min_island_size: u32,
}

fn default_island_density() -> f32 {
    0.2
}
fn default_min_island_size() -> u32 {
    200
}

impl Default for IslandSettings {
    fn default() -> Self {
        Self {
            island_density: 0.2,
            min_island_size: 200,
        }
    }
}

/// Основные параметры генерации мира
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldGenerationParams {
    pub seed: u64,

    #[serde(default = "default_width")]
    pub width: u32,

    #[serde(default = "default_height")]
    pub height: u32,

    #[serde(default)]
    pub world_type: WorldType,

    #[serde(default)]
    pub climate: ClimateSettings,

    #[serde(default)]
    pub islands: IslandSettings,

    #[serde(default = "default_num_regions")]
    pub num_regions: usize,

    #[serde(default = "default_sea_province_scale")]
    pub sea_province_scale: f32,

    #[serde(default)]
    pub terrain: TerrainSettings,
}

impl WorldGenerationParams {
    /// Загружает параметры из TOML-файла
    pub fn from_toml_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = fs::read_to_string(path)?;
        let params: Self = toml::from_str(&contents)?;
        Ok(params)
    }
}

fn default_width() -> u32 {
    2048
}
fn default_height() -> u32 {
    1024
}
fn default_num_regions() -> usize {
    100
}
fn default_sea_province_scale() -> f32 {
    2.5
}

impl Default for WorldGenerationParams {
    fn default() -> Self {
        Self {
            seed: 0,
            width: 2048,
            height: 1024,
            world_type: WorldType::EarthLike,
            climate: ClimateSettings::default(),
            islands: IslandSettings::default(),
            num_regions: 100,
            sea_province_scale: 2.5,
            terrain: TerrainSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerrainSettings {
    /// Кривая высоты:
    /// - <1.0 → сглаживает (меньше гор),
    /// - =1.0 → линейно,
    /// - >1.0 → усиливает рельеф (больше гор)
    #[serde(default = "default_elevation_power")]
    pub elevation_power: f32,

    /// Радиус сглаживания (0 = нет, 1 = 3×3, 2 = 5×5)
    #[serde(default = "default_smooth_radius")]
    pub smooth_radius: usize,
}

fn default_smooth_radius() -> usize {
    1
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            elevation_power: 1.0,
            smooth_radius: 1,
        }
    }
}

fn default_elevation_power() -> f32 {
    1.0
}
