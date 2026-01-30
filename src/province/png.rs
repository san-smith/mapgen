use crate::province::Province;
use image::{ImageBuffer, Rgba};
use rand::Rng;

pub struct ProvinceMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>, // province_id
}

impl ProvinceMap {
    pub fn new(width: u32, height: u32, provinces: &[Province]) -> Self {
        let mut data = vec![0; (width * height) as usize];
        for province in provinces {
            for &(x, y) in &province.pixels {
                let idx = (y as usize) * (width as usize) + (x as usize);
                if idx < data.len() {
                    data[idx] = province.id;
                }
            }
        }
        Self {
            width,
            height,
            data,
        }
    }

    pub fn to_rgba_image(&self) -> Vec<u8> {
        let max_province = *self.data.iter().max().unwrap_or(&0) as usize;
        let mut colors = vec![[0u8; 4]; max_province + 1];
        colors[0] = [0, 0, 0, 255]; // фон (океан)

        let mut rng = rand::thread_rng();
        for i in 1..=max_province {
            colors[i] = [
                rng.gen_range(50..200),
                rng.gen_range(50..200),
                rng.gen_range(50..200),
                255,
            ];
        }

        self.data
            .iter()
            .flat_map(|&pid| {
                let c = colors[pid as usize];
                [c[0], c[1], c[2], c[3]]
            })
            .collect()
    }

    pub fn save_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.to_rgba_image())
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
