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

    // Предварительно вычислим базовую влажность (равномерную)
    let base_humidity = 0.5 + climate.global_humidity_offset * 0.3;

    for y in 0..height {
        let y_f = y as f32;
        let latitude = (y_f / (height as f32 - 1.0)) * 2.0 - 1.0; // [-1, 1]

        // Базовая температура по широте
        let mut base_temp = 1.0 - latitude.abs();
        // Полярное усиление
        if latitude.abs() > 0.7 {
            base_temp *= 1.0 - climate.polar_amplification * 0.4;
        }
        // Глобальный сдвиг
        base_temp += climate.global_temperature_offset * 0.5;
        base_temp = base_temp.clamp(0.0, 1.0);

        for x in 0..width {
            let idx = y * width + x;
            let elevation = heightmap.data[idx];

            // Температура уменьшается с высотой
            let temp = (base_temp - elevation * 0.3).clamp(0.0, 1.0);
            temperature.push(temp);

            // Влажность: симуляция ветра с запада
            let mut humid = base_humidity;
            // Дождевые тени: если есть горы на западе — меньше влаги
            if x > 0 {
                let west_idx = y * width + (x - 1);
                let west_elevation = heightmap.data[west_idx];
                if west_elevation > elevation + 0.2 {
                    humid *= 0.7; // тень
                }
            }
            humid = humid.clamp(0.0, 1.0);
            humidity.push(humid);
        }
    }

    ClimateMaps {
        temperature,
        humidity,
    }
}
