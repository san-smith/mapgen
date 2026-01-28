use crate::config::{TerrainSettings, WorldType};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use image::{ImageBuffer, Luma};
use rand::{Rng, SeedableRng};
use rayon::prelude::*;

/// Двумерная карта высот: значения от 0.0 (глубокий океан) до 1.0 (высокие горы)
#[derive(Debug, Clone)]
pub struct Heightmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f32>,
}

impl Heightmap {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; (width * height) as usize],
        }
    }

    pub fn get(&self, x: u32, y: u32) -> f32 {
        self.data[(y * self.width + x) as usize]
    }

    pub fn set(&mut self, x: u32, y: u32, value: f32) {
        self.data[(y * self.width + x) as usize] = value;
    }

    pub fn to_grayscale_image(&self) -> Vec<u8> {
        self.data
            .par_iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect()
    }

    pub fn save_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img: ImageBuffer<Luma<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.to_grayscale_image())
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }

    /// Применяет термальную эрозию (гравитационное выветривание)
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
                        // X зацикливаем, Y ограничиваем
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

                    // Если перепад больше порога — перераспределяем
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
                    // X бесшовный через rem_euclid
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

                // Обновляем скорость
                let height_diff = self.data[idx] - min_height;
                speed = speed * 0.9 + height_diff; // Инерция + уклон

                // Эрозия: забираем материал
                let erosion = (speed * erosion_power).min(self.data[idx] * 0.5);
                self.data[idx] -= erosion;
                sediment += erosion;

                // Отложение: если скорость мала
                if speed < 0.1 && sediment > 0.0 {
                    let deposit = sediment * 0.1;
                    self.data[idx] += deposit;
                    sediment -= deposit;
                }

                x = next_x;
                y = next_y;
            }

            // Оставшийся осадок откладываем в конце
            if sediment > 0.0 {
                let final_idx = (y as usize) * width + (x as usize);
                self.data[final_idx] += sediment;
            }
        }
    }
}

/// Генерирует карту высот с бесшовностью по долготе и нелинейной коррекцией
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

    // Параметры для цилиндрической проекции
    let radius = width_f / (2.0 * std::f32::consts::PI);

    // === 1. Базовый шум (3D для бесшовности) ===
    let mut noise = FastNoiseLite::new();
    noise.set_seed(Some(seed as i32));
    noise.set_noise_type(Some(NoiseType::OpenSimplex2));
    noise.set_fractal_type(Some(FractalType::FBm));

    // Адаптируем октавы
    let octaves = match world_type {
        WorldType::Supercontinent | WorldType::Mediterranean => 3,
        WorldType::Archipelago => 4,
        _ => 5,
    };
    noise.set_fractal_octaves(Some(octaves));

    // Частота: крупные формы для континентов
    let base_frequency = match world_type {
        WorldType::Supercontinent | WorldType::Mediterranean => 0.002,
        _ => 0.005,
    };
    noise.set_frequency(Some(base_frequency));

    let mut data: Vec<f32> = (0..(width * height))
        .into_par_iter()
        .map(|i| {
            let x = (i % width) as f32;
            let y = (i / width) as f32;

            // Цилиндрические координаты
            let angle = (x / width_f) * 2.0 * std::f32::consts::PI;
            let nx = radius * angle.cos();
            let nz = radius * angle.sin();
            let ny = y;

            let mut value = noise.get_noise_3d(nx, ny, nz);
            value = (value + 1.0) * 0.5;

            if matches!(world_type, WorldType::Archipelago) {
                value = value * value;
            }
            value
        })
        .collect();

    // === 2. Добавление островов (До эрозии и без масок!) ===
    if island_density > 0.1 {
        let mut island_gen = FastNoiseLite::new();
        island_gen.set_seed(Some(seed.wrapping_add(2_000_000) as i32));
        island_gen.set_noise_type(Some(NoiseType::OpenSimplex2));
        island_gen.set_frequency(Some(0.015)); // Частота для мелких островов

        data.par_iter_mut().enumerate().for_each(|(i, h)| {
            let x = (i % width as usize) as f32;
            let y = (i / width as usize) as f32;
            let angle = (x / width_f) * 2.0 * std::f32::consts::PI;

            let iv = island_gen.get_noise_3d(radius * angle.cos(), y, radius * angle.sin());
            let island_val = (iv + 1.0) * 0.5;

            // Мягкое наложение: острова сильнее проявляются в низинах
            *h = *h + island_val * island_density * 0.25;
        });
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
    for h in &mut data {
        *h = h.powf(terrain.elevation_power);
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

    // Подбор сдвига под долю суши
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

/// Сглаживание через среднее (3×3, 5×5 и т.д.)
pub fn smooth_heightmap(data: &mut Vec<f32>, width: usize, height: usize, radius: usize) {
    if radius == 0 || radius >= width || radius >= height {
        return;
    }

    let mut temp = vec![0.0; data.len()];
    let r = radius as i32;

    // 1. Горизонтальный проход (Бесшовный)
    for y in 0..height {
        let row_offset = y * width;
        let mut window_sum = 0.0;

        // Инициализируем окно, учитывая зацикливание слева
        for dx in -r..=r {
            let x = dx.rem_euclid(width as i32) as usize;
            window_sum += data[row_offset + x];
        }

        for x in 0..width {
            temp[row_offset + x] = window_sum / (2.0 * r as f32 + 1.0);

            // Сдвигаем окно: убираем левый пиксель, добавляем правый
            let left = ((x as i32 - r).rem_euclid(width as i32)) as usize;
            let right = ((x as i32 + r + 1).rem_euclid(width as i32)) as usize;

            window_sum = window_sum - data[row_offset + left] + data[row_offset + right];
        }
    }

    let mut final_data = vec![0.0; data.len()];

    // 2. Вертикальный проход (С ограничением границ)
    for x in 0..width {
        let mut window_sum = 0.0;
        let count = (2 * r + 1) as f32;

        // Инициализируем окно (для Y используем clamp на границах)
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
