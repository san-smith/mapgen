use fastnoise_lite::FastNoiseLite;

use crate::heightmap::smooth_heightmap;

#[derive(Debug, Clone)]
pub struct ClimateMaps {
    pub temperature: Vec<f32>,
    pub humidity: Vec<f32>,
}

/// Генерирует карты температуры и влажности
pub fn generate_climate_maps(
    seed: u64,
    width: u32,
    height: u32,
    heightmap: &[f32],
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
        // Нелинейный градиент: расширяем экватор, сжимаем полюса
        let lat_factor = (y_f / height_f - 0.5).abs() * 2.0;
        let lat_temp = 1.0 - lat_factor.powf(2.5); // Больше умеренной зоны

        for x in 0..width {
            let idx = (y * width + x) as usize;
            let angle = (x as f32 / width_f) * 2.0 * std::f32::consts::PI;

            let n =
                (noise.get_noise_3d(radius * angle.cos(), y_f, radius * angle.sin()) + 1.0) * 0.5;

            // Температура падает с высотой
            let elevation_loss = heightmap[idx] * 0.4;
            temperatures[idx] = (lat_temp * 0.8 + n * 0.2 - elevation_loss).clamp(0.0, 1.0);

            // Ветер: на экваторе и полюсах дует на запад (-1.0), в умеренных широтах на восток (1.0)
            let wind_dir = if lat_factor > 0.3 && lat_factor < 0.7 {
                1.0
            } else {
                -1.0
            };
            winds[idx] = (wind_dir, 0.0); // Упрощенно: строго горизонтальный ветер
        }
    }
    (temperatures, winds)
}

pub fn calculate_humidity(
    width: u32,
    height: u32,
    heightmap: &[f32],
    winds: &[(f32, f32)], // Теперь используем направление отсюда
    sea_level: f32,
) -> Vec<f32> {
    let mut humidity = vec![0.0; (width * height) as usize];
    let width_i = width as i32;

    for y in 0..height {
        let row_start = (y * width) as usize;

        // Берем направление ветра для этой широты из первого пикселя строки
        // (так как в нашей модели ветра они зависят от широты Y)
        let (wind_x, _) = winds[row_start];
        let is_wind_east = wind_x > 0.0;

        let mut air_moisture = 0.5;

        // Проходим 2 раза для бесшовности.
        // Если ветер восточный, идем по X слева направо, если западный — справа налево.
        for x_step in 0..(width * 2) {
            let x = if is_wind_east {
                (x_step % width) as usize
            } else {
                (width - 1 - (x_step % width)) as usize
            };

            let idx = row_start + x;
            let h = heightmap[idx];

            if h < sea_level {
                // Океан насыщает воздух влагой
                air_moisture = (air_moisture + 0.15_f32).min(1.0_f32);
            } else {
                // Суша забирает влагу. Горы (уклон) заставляют влагу выпадать осадками.
                let next_x = if is_wind_east {
                    (x as i32 + 1).rem_euclid(width_i) as usize
                } else {
                    (x as i32 - 1).rem_euclid(width_i) as usize
                };

                let next_h = heightmap[row_start + next_x];
                let slope = (next_h - h).max(0.0);

                // Эффект дождевой тени: чем круче подъем, тем больше осадков выпадает
                let precipitation = air_moisture * (0.02 + slope * 8.0);
                air_moisture = (air_moisture - precipitation).max(0.0);

                // Влажность почвы в данной точке зависит от выпавших осадков
                if x_step >= width {
                    humidity[idx] = (precipitation * 20.0).clamp(0.0, 1.0);
                }
            }

            // На втором проходе записываем установившееся значение влажности в почве
            if x_step >= width && h < sea_level {
                humidity[idx] = 1.0; // Океан всегда максимально "влажный" для биомов
            }
        }
    }

    // Размываем результат, чтобы не было резких полос от ветра
    smooth_heightmap(&mut humidity, width as usize, height as usize, 3);
    humidity
}
