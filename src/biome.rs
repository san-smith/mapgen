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
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let total = width * height;

    let mut data = Vec::with_capacity(total);

    for i in 0..total {
        let elevation = heightmap.data[i];
        let temp = temperature[i];
        let humid = humidity[i];

        let biome = if elevation < sea_level {
            Biome::Ocean
        } else if elevation > 0.7 {
            Biome::Mountain
        } else if temp < 0.2 {
            Biome::Ice
        } else if temp < 0.4 {
            Biome::Tundra
        } else if humid > 0.7 {
            if temp > 0.7 {
                Biome::TropicalRainforest
            } else {
                Biome::TemperateForest
            }
        } else if humid > 0.4 {
            if temp > 0.6 {
                Biome::Savanna
            } else {
                Biome::Grassland
            }
        } else {
            if temp > 0.5 && humid > 0.2 {
                Biome::Swamp
            } else {
                Biome::Desert
            }
        };

        data.push(biome);
    }

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
