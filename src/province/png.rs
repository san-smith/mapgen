// src/province/png.rs
use crate::province::Province;
use image::{ImageBuffer, Rgba};
use std::collections::HashMap;

pub struct ProvinceMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u32>, // province_id
}

impl ProvinceMap {
    /// Создаёт карту провинций из готовой карты пикселей → `province_id`
    #[must_use]
    pub fn from_pixel_map(width: u32, height: u32, pixel_to_id: &[u32]) -> Self {
        Self {
            width,
            height,
            data: pixel_to_id.to_vec(),
        }
    }

    /// Возвращает HEX-цвет для провинции по её ID
    #[must_use]
    pub fn get_province_color(&self, provinces: &[Province], province_id: u32) -> String {
        if let Some(province) = provinces.iter().find(|p| p.id == province_id) {
            province.color.clone()
        } else {
            "#000000".to_string()
        }
    }

    #[must_use]
    pub fn to_rgba_image(&self, provinces: &[Province]) -> Vec<u8> {
        // Создаём маппинг ID → цвет
        let mut color_map: HashMap<u32, [u8; 4]> = HashMap::new();

        // Добавляем цвета для всех провинций
        for province in provinces {
            let hex = &province.color[1..]; // убираем '#'
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                color_map.insert(province.id, [r, g, b, 255]);
            }
        }

        // Цвет по умолчанию для неотнесённых пикселей
        let default_color = [0, 0, 0, 255];

        self.data
            .iter()
            .flat_map(|&pid| color_map.get(&pid).copied().unwrap_or(default_color))
            .collect()
    }

    pub fn save_as_png(
        &self,
        provinces: &[Province],
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.to_rgba_image(provinces))
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
