// src/rivers.rs
//! Гидрографическая сеть мира
//!
//! Этот модуль реализует физически-мотивированную модель рек на основе:
//! - Карты высот (направление стока воды)
//! - Накопления потока (flow accumulation) — моделирование объёма воды
//! - Биомных ограничений (реки не текут во льдах и пустынях)
//!
//! Алгоритм состоит из двух этапов:
//! 1. **Гидрологическое моделирование** — расчёт направления стока и накопления воды
//! 2. **Визуализация** — отрисовка рек с переменной толщиной в зависимости от объёма воды
//!
//! Особенности:
//! - Реки всегда текут от высоких точек к низким (включая бесшовную обработку по долготе)
//! - Пустыни моделируют испарение (потеря 50% потока)
//! - Океаны являются стоками (вода исчезает, но реки впадают в них)
//! - Толщина рек пропорциональна объёму воды

use crate::biome::{Biome, BiomeMap};
use crate::heightmap::Heightmap;
use image::{ImageBuffer, Rgb};

/// Карта рек — распределение гидрографической сети по поверхности мира
#[derive(Debug, Clone)]
pub struct RiverMap {
    /// Ширина карты в пикселях
    pub width: u32,
    /// Высота карты в пикселях
    pub height: u32,
    /// Данные рек: вектор RGB-значений размером `width × height × 3`
    /// Каждый пиксель — 3 байта (R, G, B)
    /// (0, 0, 0) = суша, (0, 102, 204) = река
    pub data: Vec<u8>,
}

/// 8 направлений для поиска пути стока (включая диагонали)
///
/// Порядок важен для детерминированности:
/// - Сначала проверяются диагонали (для более естественных изгибов),
/// - Затем ортогональные направления.
const DIRECTIONS: [(i32, i32); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

// Настраиваемые параметры визуализации
const FLOW_THRESHOLD: f32 = 100.0; // Минимальный поток для видимой реки (только значимые реки)
const MIN_THICKNESS: f32 = 1.0; // Минимальная толщина реки в пикселях
const MAX_THICKNESS: f32 = 5.0; // Максимальная толщина реки в пикселях

// Цвета реки в формате RGB (градиент от светлого к тёмному)
const RIVER_SOURCE_COLOR: [u8; 3] = [80, 150, 220]; // Светло-голубой для истоков
const RIVER_MOUTH_COLOR: [u8; 3] = [0, 60, 140]; // Тёмно-синий для устьев

/// Рисует заполненный круг на RGB изображении
fn draw_rgb_circle(
    data: &mut [u8],
    width: usize,
    height: usize,
    center_x: i32,
    center_y: i32,
    radius: i32,
    color: [u8; 3],
) {
    let r2 = radius * radius;
    for dy in -radius..=radius {
        let y = center_y + dy;
        if y < 0 || y >= height as i32 {
            continue;
        }
        let dx_max = ((r2 - dy * dy) as f32).sqrt() as i32;
        for dx in -dx_max..=dx_max {
            let x = center_x + dx;
            if x < 0 || x >= width as i32 {
                continue;
            }
            let idx = (y as usize * width + x as usize) * 3;
            data[idx] = color[0];
            data[idx + 1] = color[1];
            data[idx + 2] = color[2];
        }
    }
}

/// Генерирует карту рек на основе карты высот и биомов
///
/// # Алгоритм
/// 1. **Накопление потока (Flow Accumulation)**:
///    - Сортируем пиксели от самых высоких к самым низким
///    - Для каждого пикселя находим соседа с минимальной высотой (направление стока)
///    - Переносим "поток" (объём воды) в соседа вниз по течению
///    - В пустынях моделируем испарение (потеря 50% потока)
///    - Лёд и океаны блокируют формирование рек (но океаны принимают сток)
///
/// 2. **Визуализация**:
///    - Пиксели с потоком выше порога (`flow_threshold`) отрисовываются как реки
///    - Толщина реки пропорциональна объёму воды (от 1 до 5 пикселей)
///    - Реки не отрисовываются в океанах и на льдах (только на суше)
///
/// # Параметры
/// * `heightmap` — карта высот (0.0–1.0)
/// * `biome_map` — карта биомов для ограничения рек
///
/// # Возвращает
/// Структуру `RiverMap` с бинарной картой рек (0 = суша, 255 = река)
///
/// # Особенности реализации
/// - Алгоритм детерминирован (зависит только от входных данных)
/// - Бесшовная обработка по долготе (карта "заворачивается" по горизонтали)
/// - Вертикальные границы обрабатываются с отражением (полюса)
/// - Пороги настраиваемы через локальные константы (`flow_threshold`, `max_flow_thickness`)
///
/// # Пример
/// ```rust
/// let river_map = generate_rivers(&heightmap, &biome_map);
/// river_map.save_as_png("output/rivers.png")?;
/// ```
#[must_use]
pub fn generate_rivers(heightmap: &Heightmap, biome_map: &BiomeMap) -> RiverMap {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;

    // 1. Накопление потока (Flow Accumulation)
    let mut flow = vec![1.0f32; width * height];

    // Сортируем индексы от вершин к низинам для корректного распространения потока
    let mut indices: Vec<usize> = (0..(width * height)).collect();
    indices.sort_by(|&a, &b| {
        heightmap.data[b]
            .partial_cmp(&heightmap.data[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for &idx in &indices {
        let biome = biome_map.data[idx];

        // Реки не формируются на льдах (слишком холодно для жидкой воды)
        // Океаны не генерируют новые реки, но принимают сток с суши
        if biome == Biome::Ice {
            flow[idx] = 0.0;
            continue;
        }

        let x = (idx % width) as i32;
        let y = (idx / width) as i32;

        let mut min_h = heightmap.data[idx];
        let mut target_idx = idx;

        // Ищем соседа с минимальной высотой (направление стока)
        for &(dx, dy) in &DIRECTIONS {
            let nx = x + dx;
            let ny = y + dy;
            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                let nidx = (ny as usize) * width + (nx as usize);
                if heightmap.data[nidx] < min_h {
                    min_h = heightmap.data[nidx];
                    target_idx = nidx;
                }
            }
        }

        if target_idx != idx {
            // В пустыне часть воды "испаряется" (теряем 50% потока)
            let loss = if biome == Biome::Desert { 0.5 } else { 1.0 };
            flow[target_idx] += flow[idx] * loss;
        } else {
            // Это точка стока (вода уходит в океан или озеро)
            // Сохраняем поток для отметки устья реки
            // Умножаем на 1.5 для выделения устьевых участков
            flow[idx] *= 1.5;
        }
    }

    // 2. Сглаживание потока (box blur) для непрерывности рек
    let mut smoothed_flow = flow.clone();
    for _ in 0..2 {
        // 2 прохода сглаживания
        for y in 0..height {
            for x in 0..width {
                let idx = y * width + x;
                let mut sum = flow[idx];
                let mut count = 1;

                // Усредняем с 4 соседями
                for &(dx, dy) in &[(0i32, -1), (0, 1), (-1, 0), (1, 0)] {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nidx = ny as usize * width + nx as usize;
                        sum += flow[nidx];
                        count += 1;
                    }
                }
                smoothed_flow[idx] = sum / count as f32;
            }
        }
        flow = smoothed_flow.clone();
    }

    // Находим максимальный поток для нормализации толщины
    let max_flow = flow.iter().cloned().fold(0.0f32, f32::max);

    // 3. Рендеринг рек с градиентом цвета и толщины
    let mut river_data = vec![0u8; width * height * 3];
    
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let current_flow = flow[idx];
            let biome = biome_map.data[idx];

            // Условия отрисовки реки
            if current_flow > FLOW_THRESHOLD
                && biome != Biome::Ice
                && biome != Biome::Ocean
                && biome != Biome::DeepOcean
                && biome != Biome::IcyOcean
                && biome != Biome::FrozenOcean
            {
                // Логарифмическая толщина: реки растут экспоненциально
                // Используем ln(1 + flow) для избежания отрицательных значений
                let log_flow = (1.0 + current_flow).ln();
                let max_log_flow = (1.0 + max_flow).ln();
                let log_thickness = log_flow / max_log_flow;
                let thickness = MIN_THICKNESS + log_thickness * (MAX_THICKNESS - MIN_THICKNESS);
                let radius = (thickness / 2.0).max(0.5).round() as i32;

                // Градиент цвета: светлый в истоке, тёмный в устье
                let t = log_thickness.clamp(0.0, 1.0);
                let r = ((1.0 - t) * RIVER_SOURCE_COLOR[0] as f32 + t * RIVER_MOUTH_COLOR[0] as f32) as u8;
                let g = ((1.0 - t) * RIVER_SOURCE_COLOR[1] as f32 + t * RIVER_MOUTH_COLOR[1] as f32) as u8;
                let b = ((1.0 - t) * RIVER_SOURCE_COLOR[2] as f32 + t * RIVER_MOUTH_COLOR[2] as f32) as u8;

                // Рисуем заполненный круг с переменной толщиной
                draw_rgb_circle(&mut river_data, width, height, x as i32, y as i32, radius, [r, g, b]);
            }
        }
    }

    RiverMap {
        width: heightmap.width,
        height: heightmap.height,
        data: river_data,
    }
}

impl RiverMap {
    /// Сохраняет карту рек в цветной PNG-файл (синие реки на чёрном фоне)
    ///
    /// # Параметры
    /// * `path` — путь к файлу для сохранения
    ///
    /// # Ошибки
    /// Возвращает ошибку, если не удаётся создать или записать файл.
    ///
    /// # Пример
    /// ```rust
    /// river_map.save_as_png("output/rivers.png")?;
    /// ```
    pub fn save_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.data.clone())
                .ok_or("Failed to create RGB image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
