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
            WorldType::EarthLike => 0.30,
            WorldType::Supercontinent => 0.70,
            WorldType::Archipelago => 0.15,
            WorldType::Mediterranean => 0.25,
            WorldType::IceAgeEarth => 0.35, // больше льда = больше "суши", но непригодной
            WorldType::DesertMediterranean => 0.20,
        }
    }

    pub fn default_climate(&self) -> ClimateSettings {
        match self {
            WorldType::IceAgeEarth => ClimateSettings {
                global_temperature_offset: -0.7,
                global_humidity_offset: 0.0,
                polar_amplification: 1.8,
                climate_latitude_exponent: 1.2, // расширенные полюсы
            },
            _ => ClimateSettings {
                global_temperature_offset: 0.0,
                global_humidity_offset: 0.0,
                polar_amplification: 1.0,
                climate_latitude_exponent: 0.65, // сжатые полюсы → больше играбельной зоны
            },
        }
    }

    pub fn default_terrain(&self) -> TerrainSettings {
        match self {
            WorldType::Supercontinent | WorldType::Mediterranean => TerrainSettings {
                elevation_power: 0.65,
                smooth_radius: 2,
                mountain_compression: 0.8,
                total_provinces: 80,
            },
            WorldType::Archipelago => TerrainSettings {
                elevation_power: 0.75,
                smooth_radius: 1,
                mountain_compression: 0.5,
                total_provinces: 120,
            },
            _ => TerrainSettings::default(),
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

    /// Экспонента для сжатия полярных зон по широте:
    /// - <1.0 → сжимает полюсы (больше умеренных зон),
    /// - =1.0 → линейно,
    /// - >1.0 → расширяет полюсы.
    #[serde(default = "default_climate_latitude_exponent")]
    pub climate_latitude_exponent: f32,
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
fn default_climate_latitude_exponent() -> f32 {
    0.65
}

impl Default for ClimateSettings {
    fn default() -> Self {
        Self {
            global_temperature_offset: 0.0,
            global_humidity_offset: 0.0,
            polar_amplification: 1.0,
            climate_latitude_exponent: 0.65,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerrainSettings {
    /// Степень нелинейности высоты:
    /// - <1.0 → сглаживает рельеф (меньше гор),
    /// - =1.0 → линейно,
    /// - >1.0 → усиливает рельеф.
    #[serde(default = "default_elevation_power")]
    pub elevation_power: f32,

    #[serde(default = "default_smooth_radius")]
    pub smooth_radius: usize,

    /// Сжатие горных зон при назначении биомов:
    /// - 0.0 = линейно,
    /// - 1.0 = сильное сжатие (горы только на пиках).
    #[serde(default = "default_mountain_compression")]
    pub mountain_compression: f32,

    /// Общее количество провинций (суша + море)
    #[serde(default = "default_total_provinces")]
    pub total_provinces: usize,
}

fn default_elevation_power() -> f32 {
    0.8
}
fn default_smooth_radius() -> usize {
    1
}
fn default_mountain_compression() -> f32 {
    0.7
}
fn default_total_provinces() -> usize {
    120
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            elevation_power: 0.8,
            smooth_radius: 1,
            mountain_compression: 0.7,
            total_provinces: 120,
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

    /// Количество регионов (групп провинций)
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
    12
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
            num_regions: 12,
            sea_province_scale: 2.5,
            terrain: TerrainSettings::default(),
        }
    }
}
