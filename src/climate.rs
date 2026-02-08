// src/climate.rs
//! Климатическая система мира
//!
//! Этот модуль реализует физически-вдохновлённую модель климата, включающую:
//! - Распределение температуры по широте и высоте
//! - Моделирование глобальных ветровых потоков
//! - Расчёт влажности с учётом испарения над океанами и осадков над сушей
//! - Влияние рельефа на климат (орографические осадки)
//!
//! Климат генерируется детерминированно на основе сида и карты высот.

use fastnoise_lite::FastNoiseLite;

use crate::heightmap::smooth_heightmap;

/// Контейнер для климатических карт (зарезервирован для будущего использования)
///
/// В текущей реализации климатические данные возвращаются напрямую как кортежи,
/// но эта структура предназначена для будущей унификации интерфейса.
#[derive(Debug, Clone)]
pub struct ClimateMaps {
    /// Карта температуры: значения от 0.0 (полюс) до 1.0 (экватор)
    pub temperature: Vec<f32>,
    /// Карта влажности: значения от 0.0 (пустыня) до 1.0 (тропический лес)
    pub humidity: Vec<f32>,
}

/// Генерирует карты температуры и ветровых потоков
///
/// # Алгоритм
/// 1. **Базовая температура по широте**: синусоидальное распределение от экватора к полюсам
/// 2. **Полярное усиление**: настраиваемое охлаждение полярных зон через `polar_amplification`
/// 3. **Высотная коррекция**: охлаждение с высотой (~6.5°C на 1000м в реальности, здесь упрощено)
/// 4. **Океаническое охлаждение**: вода имеет большую теплоёмкость → медленнее нагревается/остывает
/// 5. **Шумовая вариация**: небольшие локальные колебания для естественности
///
/// # Параметры
/// * `seed` — сид для генерации шума
/// * `width`, `height` — размеры карты в пикселях
/// * `heightmap` — карта высот (0.0–1.0)
/// * `global_temperature_offset` — глобальный сдвиг температуры (-1.0..+1.0)
/// * `polar_amplification` — усиление полярного охлаждения (1.0 = стандартное)
/// * `climate_latitude_exponent` — экспонента для сжатия/расширения климатических зон по широте
/// * `sea_level` — уровень моря (обычно 0.5)
///
/// # Возвращает
/// Кортеж `(температура, ветры)`:
/// * `температура` — вектор значений 0.0..1.0
/// * `ветры` — вектор пар `(направление_x, направление_y)`, где направление в диапазоне -1.0..+1.0
///
/// # Пример
/// ```rust
/// let (temperature, winds) = generate_climate_maps(
///     42,
///     1024,
///     512,
///     &heightmap.data,
///     0.0,    // нейтральный температурный офсет
///     1.0,    // стандартное полярное охлаждение
///     0.65,   // сжатые полюсы
///     0.5,    // уровень моря
/// );
/// ```
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
        // Расстояние от экватора (0.0 = экватор, 1.0 = полюс)
        let lat_factor = (y_f / height_f - 0.5).abs() * 2.0;
        // Базовая температура по широте с нелинейным сжатием полюсов
        let lat_temp_base = 1.0 - lat_factor.powf(climate_latitude_exponent);
        // Дополнительное охлаждение к полюсам с настраиваемым усилением
        let local_offset = global_temperature_offset * (1.0 + lat_factor * polar_amplification);

        for x in 0..width {
            let idx = (y * width + x) as usize;
            let angle = (x as f32 / width_f) * 2.0 * std::f32::consts::PI;
            // Локальная вариация температуры через шум
            let n =
                (noise.get_noise_3d(radius * angle.cos(), y_f, radius * angle.sin()) + 1.0) * 0.5;
            // Охлаждение с высотой
            let elevation_loss = heightmap[idx] * 0.4;

            let mut temp = (lat_temp_base * 0.8 + n * 0.2) + local_offset - elevation_loss;
            temp = temp.clamp(0.0, 1.0);

            // === ОХЛАЖДЕНИЕ ВОДЫ ===
            // Вода имеет большую теплоёмкость → медленнее нагревается летом и остывает зимой
            // Здесь моделируем умеренное охлаждение тропических вод и сильное охлаждение полярных
            if heightmap[idx] < sea_level {
                let water_cooling = (1.0 - heightmap[idx]).min(0.5) * 0.2;
                temp = (temp - water_cooling).clamp(0.0, 1.0);
            }

            temperatures[idx] = temp;

            // === ГЛОБАЛЬНЫЕ ВЕТРЫ ===
            // Упрощённая модель трёхклеточной циркуляции Атмосферы:
            // - Тропики (0.3–0.7 от полюса к экватору): восточные пассаты
            // - Умеренные широты: западные ветры
            // - Полярные зоны: восточные полярные ветры
            let wind_dir = if lat_factor > 0.3 && lat_factor < 0.7 {
                1.0 // Восточные ветры (пассаты)
            } else {
                -1.0 // Западные ветры
            };
            winds[idx] = (wind_dir, 0.0);
        }
    }
    (temperatures, winds)
}

/// Рассчитывает карту влажности на основе ветров и рельефа
///
/// # Алгоритм (модель "воздушной массы")
/// 1. **Испарение над океаном**: воздух насыщается влагой при прохождении над водой
/// 2. **Орографические осадки**: при подъёме воздуха над горами влага конденсируется и выпадает
/// 3. **Дождевые тени**: после гор воздух становится сухим → образуются пустыни
/// 4. **Глобальный офсет влажности**: сдвигает баланс испарение/осадки
///
/// # Параметры
/// * `width`, `height` — размеры карты
/// * `heightmap` — карта высот
/// * `winds` — карта ветровых потоков (результат `generate_climate_maps`)
/// * `sea_level` — уровень моря
/// * `global_humidity_offset` — глобальный сдвиг влажности (-1.0 = сухо, +1.0 = влажно)
///
/// # Возвращает
/// Вектор значений влажности 0.0..1.0 для каждого пикселя карты
///
/// # Особенности реализации
/// - Обработка выполняется построчно с учётом направления ветра
/// - Моделируется накопление влаги в воздушной массе при прохождении над океаном
/// - Осадки усиливаются на подветренных склонах гор
/// - Финальное сглаживание (радиус 3) устраняет артефакты дискретизации
///
/// # Пример
/// ```rust
/// let humidity = calculate_humidity(
///     1024,
///     512,
///     &heightmap.data,
///     &winds,
///     0.5,    // уровень моря
///     0.0,    // нейтральный офсет влажности
/// );
/// ```
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

        // Проходим дважды для корректной обработки бесшовной карты
        for x_step in 0..(width * 2) {
            let x = if is_wind_east {
                (x_step % width) as usize
            } else {
                (width - 1 - (x_step % width)) as usize
            };

            let idx = row_start + x;
            let h = heightmap[idx];

            if h < sea_level {
                // === ИСПАРЕНИЕ НАД ОКЕАНОМ ===
                // Вода насыщает воздух влагой. Офсет влияет на скорость испарения.
                let evaporation = (0.15 + global_humidity_offset * 0.1).max(0.05);
                air_moisture = (air_moisture + evaporation).min(1.0);
            } else {
                // === ОСАДКИ НАД СУШЕЙ ===
                let next_x = if is_wind_east {
                    (x as i32 + 1).rem_euclid(width_i) as usize
                } else {
                    (x as i32 - 1).rem_euclid(width_i) as usize
                };

                let next_h = heightmap[row_start + next_x];
                // Наклон в направлении ветра → подъём воздуха → осадки
                let slope = (next_h - h).max(0.0);

                // Осадки зависят от влажности воздуха и рельефа
                let precipitation_factor = 0.02 + slope * 8.0;
                let mut precipitation = air_moisture * precipitation_factor;

                // global_humidity_offset напрямую влияет на количество выпавших осадков
                precipitation = (precipitation + global_humidity_offset * 0.05).max(0.0);

                air_moisture = (air_moisture - precipitation).max(0.0);

                // Записываем влажность только на втором проходе (после полного накопления)
                if x_step >= width {
                    // Усиливаем влияние офсета на влажность почвы
                    humidity[idx] = (precipitation * 20.0 + global_humidity_offset).clamp(0.0, 1.0);
                }
            }

            // Океан всегда имеет максимальную влажность
            if x_step >= width && h < sea_level {
                humidity[idx] = 1.0;
            }
        }
    }

    // Сглаживание для устранения артефактов дискретизации
    smooth_heightmap(&mut humidity, width as usize, height as usize, 3);
    humidity
}
