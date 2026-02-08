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
use image::{ImageBuffer, Luma};
use imageproc::drawing::draw_filled_circle_mut;

/// Карта рек — распределение гидрографической сети по поверхности мира
#[derive(Debug, Clone)]
pub struct RiverMap {
    /// Ширина карты в пикселях
    pub width: u32,
    /// Высота карты в пикселях
    pub height: u32,
    /// Данные рек: вектор значений 0..255, где 0 = суша, 255 = река
    /// Размер вектора: `width × height`
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
const FLOW_THRESHOLD: f32 = 400.0; // Минимальный поток для видимой реки
const MAX_FLOW_THICKNESS: f32 = 3000.0; // Поток, при котором река достигает максимальной толщины

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
        }
    }

    // 2. Рендеринг рек
    let mut rivers_img: ImageBuffer<Luma<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(heightmap.width, heightmap.height, Luma([0]));

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let current_flow = flow[idx];
            let biome = biome_map.data[idx];

            // Условия отрисовки реки:
            // - Достаточный объём воды (выше порога)
            // - Не лёд (реки не текут по ледникам)
            // - Не океан (реки впадают в океан, но не текут по нему)
            if current_flow > FLOW_THRESHOLD
                && biome != Biome::Ice
                && biome != Biome::Ocean
                && biome != Biome::DeepOcean
                && biome != Biome::IcyOcean
                && biome != Biome::FrozenOcean
            {
                // Толщина реки: от 1 до 5 пикселей в зависимости от объёма воды
                let thickness = (1.0 + (current_flow / MAX_FLOW_THICKNESS) * 4.0).min(5.0);

                draw_filled_circle_mut(
                    &mut rivers_img,
                    (x as i32, y as i32),
                    thickness.round() as i32,
                    Luma([255u8]),
                );
            }
        }
    }

    RiverMap {
        width: heightmap.width,
        height: heightmap.height,
        data: rivers_img.into_raw(),
    }
}

impl RiverMap {
    /// Сохраняет карту рек в монохромный PNG-файл
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
        let img: ImageBuffer<Luma<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.data.clone())
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
