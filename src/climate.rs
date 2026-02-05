use fastnoise_lite::FastNoiseLite;

use crate::heightmap::smooth_heightmap;

#[derive(Debug, Clone)]
pub struct ClimateMaps {
    pub temperature: Vec<f32>,
    pub humidity: Vec<f32>,
}

/// Генерирует карты температуры и влажности
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn generate_climate_maps(
    seed: u64,
    width: u32,
    height: u32,
    heightmap: &[f32],
    global_temperature_offset: f32,
    polar_amplification: f32,
    climate_latitude_exponent: f32,
    sea_level: f32,
) -> (Vec<f32>, Vec<(f32, f32)>) {
    let width_f = width as f32;
    let height_f = height as f32;
    let radius = width_f / (2.0 * std::f32::consts::PI);

    let mut noise = FastNoiseLite::new();
    noise.set_seed(Some(seed.wrapping_add(500) as i32));
    noise.set_frequency(Some(0.005));

    let mut temperatures = vec![0.0; (width * height) as usize];
    let mut winds = vec![(0.0, 0.0); (width * height) as usize];

    for y in 0..height {
        let y_f = y as f32;
        let lat_factor = (y_f / height_f - 0.5).abs() * 2.0;
        let lat_temp_base = 1.0 - lat_factor.powf(climate_latitude_exponent);
        let local_offset = global_temperature_offset * (1.0 + lat_factor * polar_amplification);

        for x in 0..width {
            let idx = (y * width + x) as usize;
            let angle = (x as f32 / width_f) * 2.0 * std::f32::consts::PI;
            let n =
                (noise.get_noise_3d(radius * angle.cos(), y_f, radius * angle.sin()) + 1.0) * 0.5;
            let elevation_loss = heightmap[idx] * 0.4;

            let mut temp = (lat_temp_base * 0.8 + n * 0.2) + local_offset - elevation_loss;
            temp = temp.clamp(0.0, 1.0);

            // === ОХЛАЖДЕНИЕ ВОДЫ ===
            if heightmap[idx] < sea_level {
                let water_cooling = (1.0 - heightmap[idx]).min(0.5) * 0.2;
                temp = (temp - water_cooling).clamp(0.0, 1.0);
            }

            temperatures[idx] = temp;

            let wind_dir = if lat_factor > 0.3 && lat_factor < 0.7 {
                1.0
            } else {
                -1.0
            };
            winds[idx] = (wind_dir, 0.0);
        }
    }
    (temperatures, winds)
}

#[must_use]
pub fn calculate_humidity(
    width: u32,
    height: u32,
    heightmap: &[f32],
    winds: &[(f32, f32)],
    sea_level: f32,
    global_humidity_offset: f32,
) -> Vec<f32> {
    let mut humidity = vec![0.0; (width * height) as usize];
    let width_i = width.cast_signed();

    for y in 0..height {
        let row_start = (y * width) as usize;
        let (wind_x, _) = winds[row_start];
        let is_wind_east = wind_x > 0.0;

        // Базовая влажность воздуха на старте с учетом офсета
        let mut air_moisture = (0.5 + global_humidity_offset).clamp(0.0, 1.0);

        for x_step in 0..(width * 2) {
            let x = if is_wind_east {
                (x_step % width) as usize
            } else {
                (width - 1 - (x_step % width)) as usize
            };

            let idx = row_start + x;
            let h = heightmap[idx];

            if h < sea_level {
                // Океан насыщает воздух влагой. Офсет влияет на скорость испарения.
                let evaporation = (0.15 + global_humidity_offset * 0.1).max(0.05);
                air_moisture = (air_moisture + evaporation).min(1.0);
            } else {
                let next_x = if is_wind_east {
                    (x as i32 + 1).rem_euclid(width_i) as usize
                } else {
                    (x as i32 - 1).rem_euclid(width_i) as usize
                };

                let next_h = heightmap[row_start + next_x];
                let slope = (next_h - h).max(0.0);

                // Осадки зависят от влажности воздуха и рельефа
                let precipitation_factor = 0.02 + slope * 8.0;
                let mut precipitation = air_moisture * precipitation_factor;

                // global_humidity_offset напрямую влияет на количество выпавших осадков
                precipitation = (precipitation + global_humidity_offset * 0.05).max(0.0);

                air_moisture = (air_moisture - precipitation).max(0.0);

                if x_step >= width {
                    // Усиливаем влияние офсета на влажность почвы
                    humidity[idx] = (precipitation * 20.0 + global_humidity_offset).clamp(0.0, 1.0);
                }
            }

            if x_step >= width && h < sea_level {
                humidity[idx] = 1.0;
            }
        }
    }

    smooth_heightmap(&mut humidity, width as usize, height as usize, 3);
    humidity
}
