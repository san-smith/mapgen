// src/heightmap.rs
//! Карта высот мира
//!
//! Этот модуль реализует процедурную генерацию рельефа с поддержкой:
//! - Бесшовной цилиндрической проекции (соединение восточного и западного краёв)
//! - Физически-мотивированной эрозии (термальная и гидрологическая)
//! - Адаптации под тип мира (суперконтинент, архипелаг и т.д.)
//! - Нелинейной коррекции рельефа через экспоненту высоты
//!
//! ## Алгоритм генерации
//!
//! 1. **Базовый шум (3D для бесшовности)**:
//!    - Используется 3D-шум `OpenSimplex2` для создания бесшовной карты по долготе
//!    - Цилиндрическая проекция: `(x, y)` → `(radius*cos(angle), y, radius*sin(angle))`
//!    - Адаптивные параметры октав и частоты в зависимости от типа мира
//!
//! 2. **Добавление островов**:
//!    - Отдельный генератор шума для мелких островов в океанах
//!    - Интенсивность зависит от параметра `island_density`
//!    - Наиболее выражено в низинах для естественного вида
//!
//! 3. **Сглаживание**:
//!    - Двухпроходное сглаживание (горизонтальное + вертикальное)
//!    - Бесшовное по долготе через `rem_euclid`
//!    - Ограниченное по широте (полюса не соединяются)
//!
//! 4. **Нелинейная коррекция**:
//!    - Возведение в степень `elevation_power` для управления контрастом рельефа
//!    - Значения < 1.0 сглаживают рельеф, > 1.0 усиливают
//!
//! 5. **Эрозия**:
//!    - Термальная эрозия (гравитационное выветривание) — 3 итерации
//!    - Гидрологическая эрозия (моделирование потоков воды) — адаптивное количество капель
//!
//! 6. **Нормализация**:
//!    - Линейная нормализация в диапазон [0.0, 1.0]
//!    - Подбор смещения для достижения целевой доли суши (`target_land_ratio`)
//!
//! ## Особенности реализации
//!
//! - **Бесшовность**: корректная обработка перехода через меридиан (180°)
//! - **Детерминированность**: результат полностью определяется сидом и параметрами
//! - **Эффективность**: параллельная обработка при включённой фиче `parallel`
//! - **Физическая достоверность**: эрозия моделирует реальные геоморфологические процессы

use crate::config::{TerrainSettings, WorldType};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use image::{ImageBuffer, Luma};
use rand::{Rng, SeedableRng};

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Двумерная карта высот: значения от 0.0 (глубокий океан) до 1.0 (высокие горы)
///
/// Карта представляет собой плоский вектор данных размером `ширина × высота`,
/// где каждый элемент — нормализованная высота пикселя.
#[derive(Debug, Clone)]
pub struct Heightmap {
    /// Ширина карты в пикселях
    pub width: u32,
    /// Высота карты в пикселях
    pub height: u32,
    /// Данные карты высот: вектор значений `f32` размером `ширина × высота`
    ///
    /// Каждое значение находится в диапазоне `[0.0, 1.0]`:
    /// - `0.0` — самая низкая точка (глубокий океан)
    /// - `0.5` — уровень моря (по умолчанию)
    /// - `1.0` — самая высокая точка (высочайшие горы)
    ///
    /// Индекс пикселя вычисляется как `y * width + x`.
    pub data: Vec<f32>,
}

impl Heightmap {
    /// Создаёт новую пустую карту высот заданных размеров
    ///
    /// Все значения инициализируются нулями.
    ///
    /// # Параметры
    /// * `width` — ширина карты в пикселях
    /// * `height` — высота карты в пикселях
    ///
    /// # Пример
    /// ```rust
    /// let map = Heightmap::new(1024, 512);
    /// assert_eq!(map.width, 1024);
    /// assert_eq!(map.height, 512);
    /// assert_eq!(map.data.len(), 1024 * 512);
    /// ```
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; (width * height) as usize],
        }
    }

    /// Возвращает значение высоты в заданных координатах
    ///
    /// # Параметры
    /// * `x` — координата по оси X (0..width)
    /// * `y` — координата по оси Y (0..height)
    ///
    /// # Паника
    /// Паникует, если координаты выходят за пределы карты.
    ///
    /// # Пример
    /// ```rust
    /// let h = map.get(100, 200);
    /// assert!((0.0..=1.0).contains(&h));
    /// ```
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> f32 {
        self.data[(y * self.width + x) as usize]
    }

    /// Устанавливает значение высоты в заданных координатах
    ///
    /// # Параметры
    /// * `x` — координата по оси X (0..width)
    /// * `y` — координата по оси Y (0..height)
    /// * `value` — новое значение высоты (обычно в диапазоне [0.0, 1.0])
    ///
    /// # Паника
    /// Паникует, если координаты выходят за пределы карты.
    pub fn set(&mut self, x: u32, y: u32, value: f32) {
        self.data[(y * self.width + x) as usize] = value;
    }

    /// Преобразует карту высот в градации серого для визуализации
    ///
    /// Каждое значение высоты преобразуется в яркость:
    /// ```text
    /// яркость = высота × 255
    /// ```
    ///
    /// # Возвращает
    /// Вектор байт длиной `ширина × высота`, где каждый байт — яркость пикселя (0..255).
    ///
    /// # Пример
    /// ```rust
    /// let grayscale = map.to_grayscale_image();
    /// assert_eq!(grayscale.len(), (map.width * map.height) as usize);
    /// ```
    #[must_use]
    pub fn to_grayscale_image(&self) -> Vec<u8> {
        #[cfg(feature = "parallel")]
        {
            self.data
                .par_iter()
                .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
                .collect()
        }
        #[cfg(not(feature = "parallel"))]
        {
            self.data
                .iter()
                .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
                .collect()
        }
    }

    /// Сохраняет карту высот в монохромный PNG-файл
    ///
    /// # Параметры
    /// * `path` — путь к файлу для сохранения
    ///
    /// # Ошибки
    /// Возвращает ошибку, если не удаётся создать или записать файл.
    ///
    /// # Пример
    /// ```rust
    /// map.save_as_png("output/heightmap.png")?;
    /// ```
    pub fn save_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img: ImageBuffer<Luma<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.to_grayscale_image())
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }

    /// Применяет термальную эрозию (гравитационное выветривание)
    ///
    /// Моделирует процесс осыпания материала с крутых склонов под действием гравитации.
    /// Материал перемещается вниз по склону, создавая более пологие формы.
    ///
    /// # Параметры
    /// * `iterations` — количество итераций эрозии (обычно 2-5)
    /// * `talus_angle` — критический угол склона (в единицах высоты), выше которого происходит осыпание
    ///
    /// # Эффект
    /// - Сглаживание крутых склонов
    /// - Формирование конусов выноса у подножия гор
    /// - Уменьшение экстремальных перепадов высот
    pub fn apply_thermal_erosion(&mut self, iterations: usize, talus_angle: f32) {
        let width = self.width as usize;
        let height = self.height as usize;
        let mut temp_data = self.data.clone();

        for _ in 0..iterations {
            for y in 0..height {
                for x in 0..width {
                    let idx = y * width + x;
                    let current_height = self.data[idx];

                    let mut max_diff = 0.0;
                    let mut target_idx = idx;

                    for &(dx, dy) in &[(0, 1), (1, 0), (0, -1), (-1, 0)] {
                        // X зацикливаем по долготе, Y ограничиваем по широте
                        let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                        let ny = y as i32 + dy;

                        if ny >= 0 && ny < height as i32 {
                            let nidx = (ny as usize) * width + nx;
                            let diff = current_height - self.data[nidx];
                            if diff > max_diff {
                                max_diff = diff;
                                target_idx = nidx;
                            }
                        }
                    }

                    // Если перепад больше порога — перераспределяем материал
                    if max_diff > talus_angle {
                        let move_amount = (max_diff - talus_angle) * 0.3; // Коэффициент переноса
                        temp_data[idx] -= move_amount;
                        temp_data[target_idx] += move_amount; // Материал перемещается, а не исчезает
                    }
                }
            }
            self.data.copy_from_slice(&temp_data);
        }
    }

    /// Применяет гидрологическую эрозию
    ///
    /// Моделирует воздействие воды на рельеф: вымывание материала в верховьях рек
    /// и отложение осадков в низинах.
    ///
    /// # Параметры
    /// * `seed` — сид для генерации случайных траекторий капель
    /// * `drops` — количество "капель воды" для симуляции (обычно ~1% от площади карты)
    /// * `erosion_power` — интенсивность эрозии (обычно 0.01–0.05)
    ///
    /// # Алгоритм
    /// 1. Каждая "капля" начинает путь с случайной точки
    /// 2. Движется вниз по склону, выбирая самый низкий сосед
    /// 3. Забирает материал при высокой скорости (эрозия)
    /// 4. Откладывает материал при низкой скорости (седиментация)
    /// 5. Максимальная длина пути — 30 пикселей
    pub fn apply_hydraulic_erosion(&mut self, seed: u64, drops: usize, erosion_power: f32) {
        let width = self.width as usize;
        let height = self.height as usize;
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);

        for _ in 0..drops {
            // Случайная стартовая точка
            let mut x = rng.gen_range(0..width) as i32;
            let mut y = rng.gen_range(0..height) as i32;

            let mut sediment = 0.0; // Несомый материал
            let mut speed = 0.0;

            for _ in 0..30 {
                // Макс. длина пути капли
                let idx = (y as usize) * width + (x as usize);

                let mut min_height = self.data[idx];
                let (mut next_x, mut next_y) = (x, y);

                for &(dx, dy) in &[(0, 1), (1, 0), (0, -1), (-1, 0)] {
                    // X бесшовный через rem_euclid, Y ограничен
                    let nx_val = (x + dx).rem_euclid(width as i32);
                    let ny_val = y + dy;

                    if ny_val >= 0 && ny_val < height as i32 {
                        let nidx = (ny_val as usize) * width + (nx_val as usize);
                        if self.data[nidx] < min_height {
                            min_height = self.data[nidx];
                            next_x = nx_val;
                            next_y = ny_val;
                        }
                    }
                }

                // Если не можем двигаться дальше — выходим
                if next_x == x && next_y == y {
                    break;
                }

                // Обновляем скорость (инерция + уклон)
                let height_diff = self.data[idx] - min_height;
                speed = speed * 0.9 + height_diff;

                // Эрозия: забираем материал при высокой скорости
                let erosion = (speed * erosion_power).min(self.data[idx] * 0.5);
                self.data[idx] -= erosion;
                sediment += erosion;

                // Отложение: при низкой скорости осаждаем материал
                if speed < 0.1 && sediment > 0.0 {
                    let deposit = sediment * 0.1;
                    self.data[idx] += deposit;
                    sediment -= deposit;
                }

                x = next_x;
                y = next_y;
            }

            // Оставшийся осадок откладываем в точке остановки
            if sediment > 0.0 {
                let final_idx = (y as usize) * width + (x as usize);
                self.data[final_idx] += sediment;
            }
        }
    }

    /// Генерирует карту нормалей из карты высот
    ///
    /// Нормали используются для шейдинга в 3D-рендере или для вычисления освещения.
    /// Каждая нормаль — это вектор `(nx, ny, nz)`, преобразованный в цвет `[r, g, b]`.
    ///
    /// # Алгоритм
    /// 1. Вычисление градиента по X через разницу соседних пикселей (с учётом бесшовности)
    /// 2. Вычисление градиента по Y через разницу соседних пикселей (с ограничением на полюсах)
    /// 3. Формирование нормали: `(-dx, -dy, 1.0)` с последующей нормализацией
    /// 4. Преобразование из диапазона `[-1.0, 1.0]` в `[0, 255]`
    ///
    /// # Возвращает
    /// Вектор троек `[r, g, b]` размером `ширина × высота`.
    #[must_use]
    pub fn generate_normals(&self) -> Vec<[u8; 3]> {
        let width = self.width as usize;
        let height = self.height as usize;
        let mut normals = Vec::with_capacity(width * height);

        // Масштаб высоты для вычисления градиента (1.0 = стандартный)
        let height_scale = 1.0;

        for y in 0..height {
            for x in 0..width {
                // Градиент по X (разница между соседями по долготе с бесшовностью)
                let left = self.get(
                    ((x as i32 - 1).rem_euclid(self.width.cast_signed())) as u32,
                    y as u32,
                );
                let right = self.get(
                    ((x as i32 + 1).rem_euclid(self.width.cast_signed())) as u32,
                    y as u32,
                );
                let dx = (right - left) * height_scale;

                // Градиент по Y (разница между соседями по широте с ограничением на полюсах)
                let top = if y > 0 {
                    self.get(x as u32, (y - 1) as u32)
                } else {
                    self.get(x as u32, y as u32)
                };
                let bottom = if y < height - 1 {
                    self.get(x as u32, (y + 1) as u32)
                } else {
                    self.get(x as u32, y as u32)
                };
                let dy = (bottom - top) * height_scale;

                // Нормаль = (-dx, -dy, 1), затем нормализуем
                let nx = -dx;
                let ny = -dy;
                let nz = 1.0;
                let len = (nx * nx + ny * ny + nz * nz).sqrt().max(f32::EPSILON);
                let nx = nx / len;
                let ny = ny / len;
                let nz = nz / len;

                // Преобразуем из [-1, 1] → [0, 1] → [0, 255]
                let r = ((nx * 0.5 + 0.5) * 255.0) as u8;
                let g = ((ny * 0.5 + 0.5) * 255.0) as u8;
                let b = ((nz * 0.5 + 0.5) * 255.0) as u8;

                normals.push([r, g, b]);
            }
        }

        normals
    }

    /// Сохраняет карту нормалей в цветной PNG-файл
    ///
    /// Нормали визуализируются как цветное изображение, где:
    /// - Красный канал = X-компонента нормали
    /// - Зелёный канал = Y-компонента нормали
    /// - Синий канал = Z-компонента нормали
    ///
    /// # Параметры
    /// * `path` — путь к файлу для сохранения
    ///
    /// # Ошибки
    /// Возвращает ошибку, если не удаётся создать или записать файл.
    ///
    /// # Пример
    /// ```rust
    /// map.save_normals_as_png("output/normals.png")?;
    /// ```
    pub fn save_normals_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        use image::{ImageBuffer, Rgb};
        let normals = self.generate_normals();
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(
            self.width,
            self.height,
            normals.into_iter().flatten().collect(),
        )
        .ok_or("Failed to create normals image buffer")?;
        img.save(path)?;
        Ok(())
    }
}

/// Генерирует карту высот с бесшовностью по долготе и нелинейной коррекцией
///
/// # Алгоритм
/// 1. **Базовый шум** — 3D-шум для бесшовной цилиндрической проекции
/// 2. **Острова** — дополнительный шум для создания мелких островов в океанах
/// 3. **Сглаживание** — двухпроходное усреднение для устранения артефактов шума
/// 4. **Нелинейная коррекция** — возведение в степень для управления контрастом рельефа
/// 5. **Эрозия** — термальная и гидрологическая для создания реалистичных форм
/// 6. **Нормализация** — линейное преобразование и подбор смещения под целевую долю суши
///
/// # Параметры
/// * `seed` — сид для детерминированной генерации
/// * `width`, `height` — размеры карты в пикселях
/// * `world_type` — тип генерируемого мира (влияет на параметры шума и эрозии)
/// * `island_density` — плотность мелких островов в океанах (0.0–1.0)
/// * `terrain` — настройки рельефа (сглаживание, экспонента высоты)
///
/// # Возвращает
/// Структуру `Heightmap` с нормализованными данными высот [0.0, 1.0].
///
/// # Пример
/// ```rust
/// let heightmap = generate_heightmap(
///     42,
///     2048,
///     1024,
///     WorldType::EarthLike,
///     0.2,
///     &TerrainSettings::default(),
/// );
/// ```
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn generate_heightmap(
    seed: u64,
    width: u32,
    height: u32,
    world_type: WorldType,
    island_density: f32,
    terrain: &TerrainSettings,
) -> Heightmap {
    let width_f = width as f32;
    let target_land_ratio = world_type.target_land_ratio();

    // Параметры для цилиндрической проекции (радиус цилиндра)
    let radius = width_f / (2.0 * std::f32::consts::PI);

    // === 1. Базовый шум (3D для бесшовности) ===
    let mut noise = FastNoiseLite::new();
    noise.set_seed(Some(seed as i32));
    noise.set_noise_type(Some(NoiseType::OpenSimplex2));
    noise.set_fractal_type(Some(FractalType::FBm));

    // Адаптируем октавы под тип мира
    let octaves = match world_type {
        WorldType::Supercontinent | WorldType::Mediterranean => 3,
        WorldType::Archipelago => 4,
        _ => 5,
    };
    noise.set_fractal_octaves(Some(octaves));

    // Частота: крупные формы для континентов, мелкие для архипелагов
    let base_frequency = match world_type {
        WorldType::Supercontinent | WorldType::Mediterranean => 0.002,
        _ => 0.005,
    };
    noise.set_frequency(Some(base_frequency));

    // Генерация базовой карты высот
    #[cfg(feature = "parallel")]
    let mut data: Vec<f32> = (0..(width * height))
        .into_par_iter()
        .map(|i| generate_height_value(i, width, &noise, world_type, radius))
        .collect();

    #[cfg(not(feature = "parallel"))]
    let mut data: Vec<f32> = (0..(width * height))
        .map(|i| generate_height_value(i, width, &noise, world_type, radius))
        .collect();

    // === 2. Добавление островов (До эрозии!) ===
    if island_density > 0.1 {
        let mut island_gen = FastNoiseLite::new();
        island_gen.set_seed(Some(seed.wrapping_add(2_000_000) as i32));
        island_gen.set_noise_type(Some(NoiseType::OpenSimplex2));
        island_gen.set_frequency(Some(0.015)); // Частота для мелких островов

        #[cfg(feature = "parallel")]
        {
            data.par_iter_mut().enumerate().for_each(|(i, h)| {
                add_island_effect(i, h, width, &island_gen, radius, island_density);
            });
        }
        #[cfg(not(feature = "parallel"))]
        {
            data.iter_mut().enumerate().for_each(|(i, h)| {
                add_island_effect(i, h, width, &island_gen, radius, island_density);
            });
        }
    }

    // === 3. Сглаживание (Оптимизированное бесшовное) ===
    if terrain.smooth_radius > 0 {
        smooth_heightmap(
            &mut data,
            width as usize,
            height as usize,
            terrain.smooth_radius,
        );
    }

    // === 4. Возведение в степень (Экспонента рельефа) ===
    #[cfg(feature = "parallel")]
    {
        data.par_iter_mut().for_each(|h| {
            *h = h.powf(terrain.elevation_power);
        });
    }
    #[cfg(not(feature = "parallel"))]
    {
        data.iter_mut().for_each(|h| {
            *h = h.powf(terrain.elevation_power);
        });
    }

    let mut heightmap = Heightmap {
        width,
        height,
        data,
    };

    // === 5. Эрозия (Бесшовная) ===
    heightmap.apply_thermal_erosion(3, 0.015);
    heightmap.apply_hydraulic_erosion(seed, (width * height / 80) as usize, 0.01);

    // === 6. Нормализация и подгонка уровня суши ===
    let min_h = heightmap.data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_h = heightmap
        .data
        .iter()
        .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    if max_h > min_h {
        for h in &mut heightmap.data {
            *h = (*h - min_h) / (max_h - min_h);
        }
    }

    // Подбор сдвига для достижения целевой доли суши
    let mut best_offset = 0.0;
    let mut best_diff = f32::INFINITY;
    for i in 0..100 {
        let offset = (i as f32) / 100.0 - 0.5;
        let land_count = heightmap
            .data
            .iter()
            .filter(|&&h| (h + offset).clamp(0.0, 1.0) > 0.5)
            .count();
        let land_ratio = land_count as f32 / heightmap.data.len() as f32;
        let diff = (land_ratio - target_land_ratio).abs();
        if diff < best_diff {
            best_diff = diff;
            best_offset = offset;
        }
    }

    for h in &mut heightmap.data {
        *h = (*h + best_offset).clamp(0.0, 1.0);
    }

    heightmap
}

/// Вспомогательная функция для генерации значения высоты в одной точке
///
/// # Параметры
/// * `i` — линейный индекс пикселя (0..ширина×высота)
/// * `width` — ширина карты в пикселях
/// * `noise` — генератор шума с настроенными параметрами
/// * `world_type` — тип мира (влияет на постобработку)
/// * `radius` — радиус цилиндра для проекции
///
/// # Возвращает
/// Значение высоты в диапазоне [0.0, 1.0] до нормализации.
fn generate_height_value(
    i: u32,
    width: u32,
    noise: &FastNoiseLite,
    world_type: WorldType,
    radius: f32,
) -> f32 {
    let x = (i % width) as f32;
    let y = (i / width) as f32;

    // Цилиндрические координаты для бесшовности по долготе
    let angle = (x / width as f32) * 2.0 * std::f32::consts::PI;
    let nx = radius * angle.cos();
    let nz = radius * angle.sin();
    let ny = y;

    let mut value = noise.get_noise_3d(nx, ny, nz);
    value = (value + 1.0) * 0.5;

    // Для архипелагов усиливаем контраст для создания множества островов
    if matches!(world_type, WorldType::Archipelago) {
        value = value * value;
    }
    value
}

/// Вспомогательная функция для добавления эффекта островов
///
/// # Параметры
/// * `i` — линейный индекс пикселя
/// * `h` — mutable-ссылка на значение высоты для модификации
/// * `width` — ширина карты в пикселях
/// * `island_gen` — генератор шума для островов
/// * `radius` — радиус цилиндра для проекции
/// * `island_density` — плотность островов (0.0–1.0)
///
/// # Эффект
/// Увеличивает высоту пикселя пропорционально значению шума и плотности островов.
/// Эффект сильнее проявляется в низинах для естественного вида.
fn add_island_effect(
    i: usize,
    h: &mut f32,
    width: u32,
    island_gen: &FastNoiseLite,
    radius: f32,
    island_density: f32,
) {
    let x = (i % width as usize) as f32;
    let y = (i / width as usize) as f32;
    let angle = (x / width as f32) * 2.0 * std::f32::consts::PI;

    let iv = island_gen.get_noise_3d(radius * angle.cos(), y, radius * angle.sin());
    let island_val = (iv + 1.0) * 0.5;

    // Мягкое наложение: острова сильнее проявляются в низинах
    *h += island_val * island_density * 0.25;
}

/// Сглаживание через скользящее среднее (оптимизированное)
///
/// Реализует двухпроходное сглаживание:
/// 1. Горизонтальный проход — бесшовный по долготе через `rem_euclid`
/// 2. Вертикальный проход — ограниченный по широте через `clamp`
///
/// # Параметры
/// * `data` — mutable-ссылка на данные высот для модификации
/// * `width`, `height` — размеры карты в пикселях
/// * `radius` — радиус окна сглаживания в пикселях
///
/// # Особенности
/// - Сложность O(width × height) вместо O(width × height × radius²)
/// - Бесшовность по долготе сохраняется
/// - Полюса обрабатываются с отражением (не соединяются)
pub fn smooth_heightmap(data: &mut [f32], width: usize, height: usize, radius: usize) {
    if radius == 0 || radius >= width || radius >= height {
        return;
    }

    let mut temp = vec![0.0; data.len()];
    let r = radius as i32;

    // 1. Горизонтальный проход (бесшовный по долготе)
    for y in 0..height {
        let row_offset = y * width;
        let mut window_sum = 0.0;

        // Инициализация окна с учётом зацикливания слева
        for dx in -r..=r {
            let x = dx.rem_euclid(width as i32) as usize;
            window_sum += data[row_offset + x];
        }

        for x in 0..width {
            temp[row_offset + x] = window_sum / (2.0 * r as f32 + 1.0);

            // Сдвиг окна: убираем левый пиксель, добавляем правый
            let left = ((x as i32 - r).rem_euclid(width as i32)) as usize;
            let right = ((x as i32 + r + 1).rem_euclid(width as i32)) as usize;

            window_sum = window_sum - data[row_offset + left] + data[row_offset + right];
        }
    }

    let mut final_data = vec![0.0; data.len()];

    // 2. Вертикальный проход (ограниченный по широте)
    for x in 0..width {
        let mut window_sum = 0.0;
        let count = (2 * r + 1) as f32;

        // Инициализация окна с отражением на полюсах
        for dy in -r..=r {
            let y = dy.clamp(0, height as i32 - 1) as usize;
            window_sum += temp[y * width + x];
        }

        for y in 0..height {
            final_data[y * width + x] = window_sum / count;

            let top = (y as i32 - r).clamp(0, height as i32 - 1) as usize;
            let bottom = (y as i32 + r + 1).clamp(0, height as i32 - 1) as usize;

            window_sum = window_sum - temp[top * width + x] + temp[bottom * width + x];
        }
    }

    data.copy_from_slice(&final_data);
}
