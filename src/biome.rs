use fastnoise_lite::FastNoiseLite;
use image::ImageBuffer;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

// === КОНСТАНТЫ ГЕНЕРАЦИИ ===
// Скорректированы для работы с диапазоном высот [0.0, 1.0] и sea_level около 0.5
const DEEP_OCEAN_DEPTH: f32 = 0.1;
const MOUNTAIN_START: f32 = 0.75; // Высота начала гор
const MOUNTAIN_PEAK: f32 = 0.85; // Высота ледников/скал
const ICE_TEMP_LIMIT: f32 = 0.1; // Температура замерзания воды
// Сила "размытия" границы (влияет на то, насколько широкой будет переходная зона)
const BOUNDARY_FUZZINESS: f32 = 0.15;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Biome {
    DeepOcean,
    Ocean,
    IcyOcean,
    FrozenOcean,
    Ice,
    Tundra,
    Taiga,
    TemperateForest,
    TropicalRainforest,
    Grassland,
    Shrubland, // Кустарники/Пустоши
    Savanna,
    Desert,
    Swamp,
    RockyMountain,   // Каменистые горы
    GlacialMountain, // Непроходимые ледниковые горы
}

impl Biome {
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

    /// Стоимость перемещения (для геймплея)
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

#[derive(Debug, Clone)]
pub struct BiomeMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<Biome>,
}

/// Назначает биомы на основе высоты, температуры и влажности
#[must_use]
pub fn assign_biomes(
    heightmap: &crate::heightmap::Heightmap,
    temperature: &[f32],
    humidity: &[f32],
    sea_level: f32,
) -> BiomeMap {
    let mut noise_gen = FastNoiseLite::new();
    noise_gen.set_seed(Some(98765));

    let data = heightmap
        .data
        .par_iter()
        .enumerate()
        .map(|(i, &elevation)| {
            let x = (i % heightmap.width as usize) as f32;
            let y = (i / heightmap.width as usize) as f32;

            let temp = temperature[i];
            let humid = humidity[i];

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
        })
        .collect();

    BiomeMap {
        width: heightmap.width,
        height: heightmap.height,
        data,
    }
}

impl BiomeMap {
    #[must_use]
    pub fn to_rgba_image(&self) -> Vec<u8> {
        self.data
            .par_iter()
            .flat_map(|&b| {
                let rgb = b.to_rgb();
                [rgb[0], rgb[1], rgb[2], 255] // RGBA
            })
            .collect()
    }

    pub fn save_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.to_rgba_image())
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
