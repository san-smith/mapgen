use std::collections::HashMap;

use crate::province::Province;
use crate::region::Region;
use image::{ImageBuffer, Rgba};
use rand::Rng;

pub struct RegionMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>, // region_id
}

impl RegionMap {
    pub fn new(width: u32, height: u32, provinces: &[Province], regions: &[Region]) -> Self {
        let mut data = vec![0; (width * height) as usize];

        let mut province_to_region = std::collections::HashMap::new();
        for region in regions {
            for &pid in &region.province_ids {
                province_to_region.insert(pid, region.id);
            }
        }

        for province in provinces {
            let rid = province_to_region.get(&province.id).copied().unwrap_or(0);
            for &(x, y) in &province.pixels {
                let idx = (y as usize) * (width as usize) + (x as usize);
                if idx < data.len() {
                    data[idx] = rid;
                }
            }
        }

        Self {
            width,
            height,
            data,
        }
    }

    pub fn to_rgba_image(&self, regions: &[Region], provinces: &[Province]) -> Vec<u8> {
        let mut colors = HashMap::new();
        let mut rng = rand::thread_rng();

        let prov_to_land: HashMap<u32, bool> =
            provinces.iter().map(|p| (p.id, p.is_land)).collect();

        for reg in regions {
            let is_land = reg
                .province_ids
                .first()
                .map_or(true, |pid| prov_to_land[pid]);
            let color = if is_land {
                [
                    rng.gen_range(100..220),
                    rng.gen_range(120..255),
                    rng.gen_range(50..100),
                    255,
                ]
            } else {
                [30, 60, rng.gen_range(120..220), 255]
            };
            colors.insert(reg.id, color);
        }

        self.data
            .iter()
            .flat_map(|&rid| {
                colors.get(&rid).copied().unwrap_or([20, 20, 60, 255]) // Темно-синий фон, если регион не найден
            })
            .collect()
    }

    pub fn save_as_png(
        &self,
        path: &str,
        regions: &[Region],
        provinces: &[Province],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Передаем аргументы дальше в to_rgba_image
        let rgba_data = self.to_rgba_image(regions, provinces);
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, rgba_data)
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
