use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Biome {
    Ocean,
    Ice,
    Tundra,
    Taiga,
    TemperateForest,
    TropicalRainforest,
    Grassland,
    Savanna,
    Desert,
    Swamp,
    Mountain,
}

impl Biome {
    pub fn to_rgb(&self) -> [u8; 3] {
        match self {
            Biome::Ocean => [0, 64, 128],
            Biome::Ice => [220, 220, 255],
            Biome::Tundra => [200, 220, 180],
            Biome::Taiga => [100, 150, 100],
            Biome::TemperateForest => [60, 120, 60],
            Biome::TropicalRainforest => [30, 100, 30],
            Biome::Grassland => [150, 200, 100],
            Biome::Savanna => [200, 180, 100],
            Biome::Desert => [200, 180, 120],
            Biome::Swamp => [80, 100, 60],
            Biome::Mountain => [150, 150, 150],
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
pub fn assign_biomes(
    heightmap: &crate::heightmap::Heightmap,
    temperature: &[f32],
    humidity: &[f32],
    sea_level: f32,
) -> BiomeMap {
    let data = heightmap
        .data
        .iter()
        .enumerate()
        .map(|(i, &elevation)| {
            let temp = temperature[i];
            let humid = humidity[i];

            if elevation < sea_level {
                return Biome::Ocean;
            }

            // 1. Горы: поднимаем порог до 0.8+ и учитываем температуру
            // На холоде горы превращаются в лед (Ice), а не в серые скалы
            if elevation > 0.85 {
                return Biome::Mountain;
            }
            if elevation > 0.75 && temp < 0.3 {
                return Biome::Ice;
            }

            // 2. Распределение зон (измененные пороги для расширения умеренной зоны)
            if temp < 0.15 {
                // Сильно уменьшили зону льда
                Biome::Ice
            } else if temp < 0.3 {
                // Уменьшили тундру
                if humid < 0.4 {
                    Biome::Tundra
                } else {
                    Biome::Taiga
                }
            } else if temp < 0.65 {
                // УВЕЛИЧИЛИ умеренную зону (было 0.5)
                if humid < 0.35 {
                    Biome::Grassland
                } else if humid < 0.7 {
                    Biome::TemperateForest
                } else {
                    Biome::Swamp
                }
            } else {
                // Тропики и пустыни
                if humid < 0.25 {
                    Biome::Desert
                } else if humid < 0.55 {
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
    pub fn to_rgba_image(&self) -> Vec<u8> {
        self.data
            .iter()
            .flat_map(|&b| {
                let rgb = b.to_rgb();
                [rgb[0], rgb[1], rgb[2], 255] // RGBA
            })
            .collect()
    }

    pub fn save_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(self.width, self.height, self.to_rgba_image())
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
