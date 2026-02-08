// src/config.rs
//! Конфигурация генерации мира
//!
//! Этот модуль определяет все параметры, управляющие процедурной генерацией мира:
//! - Типы миров (землеподобный, архипелаг и т.д.)
//! - Климатические настройки
//! - Параметры рельефа и островов
//! - Настройки провинций и регионов
//!
//! Все структуры поддерживают сериализацию в TOML/JSON для удобной настройки через конфигурационные файлы.

use serde::{Deserialize, Serialize};
use std::fs;

/// Тип генерируемого мира
///
/// Определяет глобальную структуру карты: распределение суши/моря, форму континентов и климатические особенности.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum WorldType {
    /// Землеподобный мир с несколькими континентами и океанами (≈30% суши)
    #[default]
    EarthLike,
    /// Один крупный суперконтинент с небольшими островами (≈70% суши)
    Supercontinent,
    /// Многочисленные острова и архипелаги, мало крупных континентов (≈15% суши)
    Archipelago,
    /// Большое внутреннее море, окружённое континентами (≈25% суши)
    Mediterranean,
    /// Ледниковый период: расширенные полярные шапки, больше льда и тундры (≈35% "суши", но большая часть непригодна)
    IceAgeEarth,
    /// Средиземноморье с преобладанием пустынь и засушливых биомов (≈20% суши)
    DesertMediterranean,
}

impl WorldType {
    /// Возвращает целевую долю суши для данного типа мира.
    ///
    /// # Возвращает
    /// - `0.0` — полностью водный мир
    /// - `1.0` — полностью сухопутный мир
    ///
    /// # Примеры
    /// ```
    /// use mapgen::config::WorldType;
    /// assert_eq!(WorldType::EarthLike.target_land_ratio(), 0.30);
    /// assert_eq!(WorldType::Archipelago.target_land_ratio(), 0.15);
    /// ```
    #[must_use]
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

    /// Возвращает настройки климата по умолчанию для данного типа мира.
    ///
    /// # Особенности
    /// - `IceAgeEarth` имеет пониженную глобальную температуру и расширенные полярные зоны
    /// - Остальные типы используют умеренный климат с сжатыми полюсами для увеличения играбельной зоны
    #[must_use]
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

    /// Возвращает настройки рельефа по умолчанию для данного типа мира.
    ///
    /// # Особенности
    /// - `Supercontinent` и `Mediterranean` имеют более сглаженный рельеф (меньше гор)
    /// - `Archipelago` имеет более резкий рельеф для создания драматичных островов
    #[must_use]
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

/// Глобальные климатические модификаторы
///
/// Управляет распределением температуры и влажности по широте и высоте.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClimateSettings {
    /// Глобальный сдвиг температуры (-1.0 = очень холодно, +1.0 = очень жарко)
    #[serde(default = "default_temperature_offset")]
    pub global_temperature_offset: f32,

    /// Глобальный сдвиг влажности (-1.0 = очень сухо, +1.0 = очень влажно)
    #[serde(default = "default_humidity_offset")]
    pub global_humidity_offset: f32,

    /// Усиление полярного эффекта:
    /// - `1.0` — стандартное охлаждение к полюсам
    /// - `>1.0` — более резкое охлаждение (широкие тундры/льды)
    /// - `<1.0` — более мягкое охлаждение (узкие полярные зоны)
    #[serde(default = "default_polar_amplification")]
    pub polar_amplification: f32,

    /// Экспонента для сжатия полярных зон по широте:
    /// - `<1.0` → сжимает полюсы (больше умеренных зон),
    /// - `=1.0` → линейно,
    /// - `>1.0` → расширяет полюсы.
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
///
/// Управляет генерацией мелких островов в открытых океанах.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IslandSettings {
    /// Плотность островов (0.0 = нет островов, 1.0 = очень много островов)
    #[serde(default = "default_island_density")]
    pub island_density: f32,

    /// Минимальный размер острова в пикселях (острова меньше этого размера не генерируются)
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

/// Настройки рельефа и провинций
///
/// Управляет формой ландшафта и количеством административных единиц.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerrainSettings {
    /// Степень нелинейности высоты:
    /// - `<1.0` → сглаживает рельеф (меньше гор, больше равнин),
    /// - `=1.0` → линейно,
    /// - `>1.0` → усиливает рельеф (более резкие горы и долины).
    #[serde(default = "default_elevation_power")]
    pub elevation_power: f32,

    /// Радиус сглаживания в пикселях (0 = без сглаживания)
    #[serde(default = "default_smooth_radius")]
    pub smooth_radius: usize,

    /// Сжатие горных зон при назначении биомов:
    /// - `0.0` = линейно,
    /// - `1.0` = сильное сжатие (горы только на самых высоких пиках).
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
///
/// Полная конфигурация для генерации одного мира. Поддерживает загрузку из TOML-файлов.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldGenerationParams {
    /// Сид генератора случайных чисел (детерминированная генерация)
    pub seed: u64,

    /// Ширина карты в пикселях (по умолчанию 2048)
    #[serde(default = "default_width")]
    pub width: u32,

    /// Высота карты в пикселях (по умолчанию 1024)
    #[serde(default = "default_height")]
    pub height: u32,

    /// Тип генерируемого мира (по умолчанию `EarthLike`)
    #[serde(default)]
    pub world_type: WorldType,

    /// Климатические настройки (по умолчанию умеренный климат)
    #[serde(default)]
    pub climate: ClimateSettings,

    /// Настройки островов в океане (по умолчанию умеренная плотность)
    #[serde(default)]
    pub islands: IslandSettings,

    /// Количество регионов (групп провинций) для генерации
    #[serde(default = "default_num_regions")]
    pub num_regions: usize,

    /// Масштаб морских провинций относительно сухопутных (по умолчанию 2.5 = морские провинции крупнее)
    #[serde(default = "default_sea_province_scale")]
    pub sea_province_scale: f32,

    /// Настройки рельефа и провинций
    #[serde(default)]
    pub terrain: TerrainSettings,
}

impl WorldGenerationParams {
    /// Загружает параметры из TOML-файла
    ///
    /// # Аргументы
    /// * `path` - путь к файлу конфигурации в формате TOML
    ///
    /// # Ошибки
    /// Возвращает ошибку, если файл не найден или содержит недопустимый формат.
    ///
    /// # Пример
    /// ```toml
    /// # world.toml
    /// seed = 42
    /// width = 1024
    /// height = 512
    /// world_type = "Archipelago"
    /// ```
    ///
    /// ```rust
    /// let params = WorldGenerationParams::from_toml_file("world.toml")?;
    /// ```
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
