// src/region/png.rs
use std::collections::HashMap;

use crate::region::Region;
use image::{ImageBuffer, Rgba};

pub struct RegionMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>, // region_id
}

impl RegionMap {
    /// Создаёт карту регионов из готовой карты пикселей → province_id
    pub fn from_pixel_map(
        width: u32,
        height: u32,
        pixel_to_id: &[u32],
        regions: &[Region],
    ) -> Self {
        // Создаём маппинг province_id → region_id
        let mut province_to_region = HashMap::new();
        for region in regions {
            for &pid in &region.province_ids {
                province_to_region.insert(pid, region.id);
            }
        }

        // Заполняем карту регионов
        let data: Vec<u32> = pixel_to_id
            .iter()
            .map(|&pid| {
                if pid == u32::MAX {
                    0 // фон
                } else {
                    *province_to_region.get(&pid).unwrap_or(&0)
                }
            })
            .collect();

        Self {
            width,
            height,
            data,
        }
    }

    pub fn to_rgba_image(&self, regions: &[Region]) -> Vec<u8> {
        let mut colors = HashMap::new();

        for reg in regions {
            let hex = &reg.color[1..]; // убираем '#'
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                colors.insert(reg.id, [r, g, b, 255]);
            }
        }

        self.data
            .iter()
            .flat_map(|&rid| colors.get(&rid).copied().unwrap_or([20, 20, 60, 255]))
            .collect()
    }

    pub fn save_as_png(
        &self,
        path: &str,
        regions: &[Region],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let rgba_data = self.to_rgba_image(regions);
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, rgba_data)
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
