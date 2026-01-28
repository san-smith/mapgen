use crate::config::ClimateSettings;
use crate::heightmap::Heightmap;

#[derive(Debug, Clone)]
pub struct ClimateMaps {
    pub temperature: Vec<f32>,
    pub humidity: Vec<f32>,
}

/// Генерирует карты температуры и влажности
pub fn generate_climate_maps(heightmap: &Heightmap, climate: &ClimateSettings) -> ClimateMaps {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let total = width * height;

    let mut temperature = Vec::with_capacity(total);
    let mut humidity = Vec::with_capacity(total);

    for y in 0..height {
        let y_f = y as f32;
        let latitude = (y_f / (height as f32 - 1.0)) * 2.0 - 1.0; // [-1, 1]

        // === Нелинейное сжатие полюсов ===
        let compressed_lat = latitude.abs().powf(climate.climate_latitude_exponent);
        let base_temp = 1.0 - compressed_lat;

        // Полярное усиление
        let base_temp = if latitude.abs() > 0.7 {
            base_temp * (1.0 - climate.polar_amplification * 0.4)
        } else {
            base_temp
        };

        // Глобальный сдвиг
        let base_temp = (base_temp + climate.global_temperature_offset * 0.5).clamp(0.0, 1.0);

        for x in 0..width {
            let idx = y * width + x;
            let elevation = heightmap.data[idx];

            // Температура уменьшается с высотой
            let temp = (base_temp - elevation * 0.3).clamp(0.0, 1.0);
            temperature.push(temp);

            // Влажность: ветер с запада + дождевые тени
            let mut humid = 0.5 + climate.global_humidity_offset * 0.3;
            if x > 0 {
                let west_idx = y * width + (x - 1);
                let west_elev = heightmap.data[west_idx];
                if west_elev > elevation + 0.2 {
                    humid *= 0.7;
                }
            }
            humidity.push(humid.clamp(0.0, 1.0));
        }
    }

    ClimateMaps {
        temperature,
        humidity,
    }
}
