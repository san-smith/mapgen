// src/biome.rs
//! Система биомов мира
//!
//! Этот модуль реализует физически-мотивированную классификацию ландшафтов
//! на основе трёх ключевых факторов:
//! - Высота над уровнем моря
//! - Температура (широта + высота)
//! - Влажность (ветры + рельеф)
//!
//! Биомы разделены на две основные категории:
//! 1. **Водные** — океаны разной глубины и состояния (лёд/вода)
//! 2. **Сухопутные** — от арктических льдов до тропических лесов, включая горные системы
//!
//! Каждый биом имеет:
//! - Уникальный цвет для визуализации
//! - Стоимость перемещения для геймплея
//! - Чёткие переходные зоны с "размытием" для естественности

use fastnoise_lite::FastNoiseLite;
use image::ImageBuffer;
use serde::{Deserialize, Serialize};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

// === КОНСТАНТЫ ГЕНЕРАЦИИ ===
// Скорректированы для работы с диапазоном высот [0.0, 1.0] и sea_level около 0.5
const DEEP_OCEAN_DEPTH: f32 = 0.1;
const MOUNTAIN_START: f32 = 0.75; // Высота начала гор
const MOUNTAIN_PEAK: f32 = 0.85; // Высота ледников/скал
const ICE_TEMP_LIMIT: f32 = 0.1; // Температура замерзания воды
// Сила "размытия" границы (влияет на то, насколько широкой будет переходная зона)
const BOUNDARY_FUZZINESS: f32 = 0.15;

/// Тип биома — классификация ландшафта
///
/// Биомы упорядочены по приоритету обработки:
/// 1. Водные биомы (в порядке глубины и состояния)
/// 2. Горные биомы (по высоте)
/// 3. Климатические биомы (по температуре и влажности)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Biome {
    /// Глубокий океан (>1000м) — тёмно-синий, почти чёрный
    /// Характеристики: холодный, высокое давление, минимальная жизнь
    DeepOcean,
    /// Мелководный океан (<1000м) — ярко-синий
    /// Характеристики: умеренная температура, богатая морская жизнь
    Ocean,
    /// Океан с айсбергами/лёдяной крошкой — светло-голубой
    /// Характеристики: холодная вода с плавающим льдом, сезонное явление
    IcyOcean,
    /// Плотный морской лёд/паковый лёд — приглушённый голубовато-серый
    /// Характеристики: практически непроходим для судов, арктические/антарктические регионы
    FrozenOcean,
    /// Материковый ледниковый щит — очень светлый, почти белый
    /// Характеристики: вечный лёд, минимальная жизнь, полярные регионы
    Ice,
    /// Тундра — серо-зелёная, не жёлтая
    /// Характеристики: короткое лето, вечная мерзлота, мхи и лишайники
    Tundra,
    /// Тайга (хвойный лес) — насыщенный тёмно-зелёный
    /// Характеристики: холодные зимы, умеренные лета, хвойные деревья
    Taiga,
    /// Умеренный лес — ярко-зелёный
    /// Характеристики: четыре сезона, смешанный лес (лиственные + хвойные)
    TemperateForest,
    /// Тропический лес — тёмно-зелёный, почти чёрный
    /// Характеристики: высокая температура и влажность круглый год, максимальное биоразнообразие
    TropicalRainforest,
    /// Степь/луга — свежая трава, ярко-зелёная
    /// Характеристики: умеренная влажность, сезонные дожди, открытые пространства
    Grassland,
    /// Кустарники/Пустоши — оливково-жёлтый (не коричневый!)
    /// Характеристики: засушливые условия, низкорослая растительность
    Shrubland,
    /// Саванна — золотисто-жёлтая
    /// Характеристики: сезонные дожди, разреженные деревья, открытые пространства
    Savanna,
    /// Пустыня — светло-песочный, не тёмный
    /// Характеристики: минимальные осадки, экстремальные температуры дня/ночи
    Desert,
    /// Болото — тёмно-зелёное с серым оттенком
    /// Характеристики: избыточная влажность, стоячая вода, труднопроходимо
    Swamp,
    /// Каменистые горы — средний серый
    /// Характеристики: скалистые склоны, минимальная растительность, сложное перемещение
    RockyMountain,
    /// Непроходимые ледниковые горы — светло-голубоватый (не белый)
    /// Характеристики: вечные снега и ледники, максимальная высота, непроходимо
    GlacialMountain,
}

impl Biome {
    /// Возвращает цвет биома в формате RGB для визуализации
    ///
    /// Цвета подобраны для максимального контраста и интуитивного восприятия:
    /// - Вода: синие тона (глубже → темнее)
    /// - Суша: зелёные тона (влажнее → ярче)
    /// - Пустыни: жёлто-коричневые тона
    /// - Горы: серые тона (выше → светлее из-за снега)
    ///
    /// # Пример
    /// ```rust
    /// let rgb = Biome::TemperateForest.to_rgb();
    /// assert_eq!(rgb, [60, 140, 60]);
    /// ```
    #[must_use]
    pub fn to_rgb(&self) -> [u8; 3] {
        match self {
            // === ВОДА (чётко различимые) ===
            Biome::DeepOcean => [0, 30, 80], // Глубокий океан — тёмно-синий, почти чёрный
            Biome::Ocean => [0, 70, 140],    // Стандартный океан — ярко-синий
            Biome::IcyOcean => [120, 180, 220], // Лёд на поверхности — светло-голубой (как айсберги)
            Biome::FrozenOcean => [180, 200, 220], // Плотный лёд/пак — приглушённый голубовато-серый

            // === СУША (зелёное доминирует в умеренной зоне) ===
            Biome::Ice => [220, 230, 255], // Чистый лёд — очень светлый, почти белый
            Biome::Tundra => [200, 210, 190], // Светлая тундра — серо-зелёная, не жёлтая
            Biome::Taiga => [80, 120, 80], // Хвойный лес — насыщенный тёмно-зелёный
            Biome::TemperateForest => [60, 140, 60], // Умеренный лес — ярко-зелёный (ваша "умеренная полоса"!)
            Biome::TropicalRainforest => [30, 100, 30], // Тропики — тёмно-зелёный, почти чёрный
            Biome::Grassland => [140, 190, 100],     // Равнины — свежая трава, ярко-зелёная
            Biome::Shrubland => [160, 150, 100], // Кустарники — оливково-жёлтый (не коричневый!)
            Biome::Savanna => [190, 170, 100],   // Саванна — золотисто-жёлтая
            Biome::Desert => [220, 200, 150],    // Пустыня — светло-песочный, не тёмный
            Biome::Swamp => [70, 110, 60],       // Болото — тёмно-зелёное с серым оттенком
            Biome::RockyMountain => [140, 140, 140], // Скалы — средний серый
            Biome::GlacialMountain => [200, 220, 240], // Ледниковые горы — светло-голубоватый (не белый, чтобы отличать от льда)
        }
    }

    /// Стоимость перемещения через биом (для геймплея)
    ///
    /// Значения интерпретируются как множитель к базовой скорости:
    /// - `1.0` — стандартная скорость
    /// - `>1.0` — замедление
    /// - `INFINITY` — непроходимо
    ///
    /// # Примеры
    /// - `DeepOcean`: 1.5× (требуется корабль)
    /// - `Swamp`: 2.0× (труднопроходимо)
    /// - `GlacialMountain`: ∞ (непроходимо без специального снаряжения)
    #[must_use]
    pub fn movement_cost(&self) -> f32 {
        match self {
            Biome::DeepOcean => 1.5,
            Biome::IcyOcean | Biome::Swamp => 2.0,
            Biome::FrozenOcean => 3.0,
            Biome::RockyMountain => 4.0,
            Biome::GlacialMountain => f32::INFINITY, // Непроходимы
            _ => 1.0,
        }
    }
}

/// Карта биомов — распределение ландшафтов по поверхности мира
#[derive(Debug, Clone)]
pub struct BiomeMap {
    /// Ширина карты в пикселях
    pub width: u32,
    /// Высота карты в пикселях
    pub height: u32,
    /// Данные биомов: вектор значений `Biome` размером `width × height`
    pub data: Vec<Biome>,
}

/// Назначает биомы на основе высоты, температуры и влажности
///
/// # Алгоритм принятия решений
/// 1. **Водные биомы** (приоритет 0):
///    - Если высота < уровня моря → вода
///    - Глубина определяет тип океана (мелкий/глубокий)
///    - Температура определяет состояние воды (жидкая/лёд)
///
/// 2. **Горные биомы** (приоритет 1 — самый высокий для суши):
///    - Высота > `MOUNTAIN_PEAK` → `GlacialMountain` (если холодно) или `RockyMountain`
///    - Высота > `MOUNTAIN_START` → горы с учётом температуры
///    - Горы имеют приоритет над климатом — реалистично для высокогорья
///
/// 3. **Климатические биомы** (приоритет 2):
///    - Температура определяет широтную зону (полярная/умеренная/тропическая)
///    - Влажность определяет тип растительности внутри зоны
///    - Случайное "размытие" (`BOUNDARY_FUZZINESS`) создаёт естественные переходы
///
/// # Параметры
/// * `heightmap` — карта высот (0.0–1.0)
/// * `temperature` — карта температуры (0.0–1.0)
/// * `humidity` — карта влажности (0.0–1.0)
/// * `sea_level` — уровень моря (обычно 0.5)
///
/// # Возвращает
/// Структуру `BiomeMap` с распределением биомов по карте
///
/// # Особенности
/// - Алгоритм детерминирован (зависит только от входных данных)
/// - Горы всегда имеют приоритет над климатом (реалистично)
/// - Переходы между биомами имеют естественное "размытие" через шум
#[must_use]
pub fn assign_biomes(
    heightmap: &crate::heightmap::Heightmap,
    temperature: &[f32],
    humidity: &[f32],
    sea_level: f32,
) -> BiomeMap {
    #[cfg(feature = "parallel")]
    {
        // ПАРАЛЛЕЛЬНАЯ ВЕРСИЯ — ИСПРАВЛЕНО
        let data: Vec<Biome> = heightmap
            .data
            .par_iter()
            .enumerate()
            .map(|(i, &elevation)| {
                let x = (i % heightmap.width as usize) as f32;
                let y = (i / heightmap.width as usize) as f32;
                let temp = temperature[i];
                let humid = humidity[i];

                let mut noise_gen = FastNoiseLite::new();
                noise_gen.set_seed(Some(98765)); // ← ОДИНАКОВЫЙ SEED

                assign_biome_at_point(elevation, temp, humid, sea_level, x, y, &mut noise_gen)
            })
            .collect();

        BiomeMap {
            width: heightmap.width,
            height: heightmap.height,
            data,
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        // ПОСЛЕДОВАТЕЛЬНАЯ ВЕРСИЯ — ИСПРАВЛЕНО
        let mut noise_gen = FastNoiseLite::new();
        noise_gen.set_seed(Some(98765)); // ← ОДИНАКОВЫЙ SEED

        let data: Vec<Biome> = heightmap
            .data
            .iter()
            .enumerate()
            .map(|(i, &elevation)| {
                let x = (i % heightmap.width as usize) as f32;
                let y = (i / heightmap.width as usize) as f32;
                let temp = temperature[i];
                let humid = humidity[i];

                assign_biome_at_point(elevation, temp, humid, sea_level, x, y, &mut noise_gen)
            })
            .collect();

        BiomeMap {
            width: heightmap.width,
            height: heightmap.height,
            data,
        }
    }
}

/// Вспомогательная функция для назначения биома в одной точке
fn assign_biome_at_point(
    elevation: f32,
    temp: f32,
    humid: f32,
    sea_level: f32,
    x: f32,
    y: f32,
    noise_gen: &mut FastNoiseLite,
) -> Biome {
    if elevation < sea_level {
        // --- ЛОГИКА ВОДЫ ---
        let depth = sea_level - elevation;
        if temp < ICE_TEMP_LIMIT - 0.05 {
            Biome::FrozenOcean
        } else if temp < ICE_TEMP_LIMIT + 0.15 {
            // Широкая зона айсбергов
            Biome::IcyOcean
        } else if depth > DEEP_OCEAN_DEPTH {
            Biome::DeepOcean
        } else {
            Biome::Ocean
        }
    } else {
        // --- ЛОГИКА СУШИ И ГОР ---

        // ПРИОРИТЕТ 1: Горы всегда определяются по высоте первыми!
        // Сначала определяем, насколько холодно, потом какой тип горы
        if elevation > MOUNTAIN_PEAK {
            // Если наверху холодно, это всегда GlacialMountain
            if temp < 0.3 {
                return Biome::GlacialMountain;
            }

            return Biome::RockyMountain;
        } else if elevation > MOUNTAIN_START {
            // Если на средней высоте холодно, это GlacialMountain, иначе RockyMountain
            if temp < 0.25 {
                return Biome::GlacialMountain;
            }

            return Biome::RockyMountain;
        }

        // ПРИОРИТЕТ 2: Затем используем климат
        // Создаем уникальное случайное смещение для каждого пикселя
        let dither = noise_gen.get_noise_2d(x, y) * BOUNDARY_FUZZINESS;

        if temp < 0.15 + dither {
            Biome::Ice
        } else if temp < 0.3 + dither {
            if humid < 0.4 + dither {
                Biome::Tundra
            } else {
                Biome::Taiga
            }
        } else if temp < 0.65 + dither {
            if humid < 0.2 + dither {
                Biome::Shrubland
            } else if humid < 0.4 + dither {
                Biome::Grassland
            } else if humid < 0.7 + dither {
                Biome::TemperateForest
            } else {
                Biome::Swamp
            }
        } else if humid < 0.25 + dither {
            Biome::Desert
        } else if humid < 0.55 + dither {
            Biome::Savanna
        } else {
            Biome::TropicalRainforest
        }
    }
}

impl BiomeMap {
    /// Преобразует карту биомов в RGBA-изображение для визуализации
    ///
    /// Каждый пиксель преобразуется в 4 байта (R, G, B, A), где:
    /// - R, G, B — цвет биома через `Biome::to_rgb()`
    /// - A — альфа-канал (всегда 255 = непрозрачный)
    ///
    /// # Возвращает
    /// Вектор байт длиной `width × height × 4`
    #[must_use]
    pub fn to_rgba_image(&self) -> Vec<u8> {
        #[cfg(feature = "parallel")]
        {
            self.data
                .par_iter()
                .flat_map(|&b| {
                    let rgb = b.to_rgb();
                    [rgb[0], rgb[1], rgb[2], 255] // RGBA
                })
                .collect()
        }
        #[cfg(not(feature = "parallel"))]
        {
            self.data
                .iter()
                .flat_map(|&b| {
                    let rgb = b.to_rgb();
                    [rgb[0], rgb[1], rgb[2], 255] // RGBA
                })
                .collect()
        }
    }

    /// Сохраняет карту биомов в PNG-файл
    ///
    /// # Параметры
    /// * `path` — путь к файлу для сохранения
    ///
    /// # Ошибки
    /// Возвращает ошибку, если не удаётся создать или записать файл.
    ///
    /// # Пример
    /// ```rust
    /// biome_map.save_as_png("output/biomes.png")?;
    /// ```
    pub fn save_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.to_rgba_image())
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
