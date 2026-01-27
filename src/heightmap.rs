use crate::config::WorldType;
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

                    // Проверяем 4 соседа (можно расширить до 8)
                    for &(dx, dy) in &[(0, 1), (1, 0), (0, -1), (-1, 0)] {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                            let nidx = (ny as usize) * width + (nx as usize);
                            let diff = current_height - self.data[nidx];
                            if diff > max_diff {
                                max_diff = diff;
                            }
                        }
                    }

                    // Если перепад больше порога — перераспределяем
                    if max_diff > talus_angle {
                        let move_amount = (max_diff - talus_angle) * 0.5;
                        temp_data[idx] -= move_amount;
                        // Материал "падает" вниз — упрощённо просто уменьшаем высоту
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
                if idx >= self.data.len() {
                    break;
                }

                // Найдём самый низкий сосед
                let mut min_height = self.data[idx];
                let mut next_x = x;
                let mut next_y = y;

                for &(dx, dy) in &[(0, 1), (1, 0), (0, -1), (-1, 0)] {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                        let nidx = (ny as usize) * width + (nx as usize);
                        if self.data[nidx] < min_height {
                            min_height = self.data[nidx];
                            next_x = nx;
                            next_y = ny;
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
                let idx = (y as usize) * width + (x as usize);
                if idx < self.data.len() {
                    self.data[idx] += sediment;
                }
            }
        }
    }
}

/// Генерирует карту высот на основе параметров
pub fn generate_heightmap(
    seed: u64,
    width: u32,
    height: u32,
    world_type: WorldType,
    island_density: f32,
) -> Heightmap {
    let width_f = width as f32;
    let height_f = height as f32;
    let target_land_ratio = world_type.target_land_ratio();

    // === 1. Базовый шум ===
    let mut noise = FastNoiseLite::new();
    noise.set_seed(Some(seed as i32));
    noise.set_noise_type(Some(NoiseType::OpenSimplex2));
    noise.set_fractal_type(Some(FractalType::FBm));
    noise.set_fractal_octaves(Some(5));
    noise.set_frequency(Some(0.005)); // низкая частота для крупных форм

    let data: Vec<f32> = (0..(width * height))
        .into_par_iter()
        .map(|i| {
            let x = (i % width) as f32;
            let y = (i / width) as f32;
            let mut value = noise.get_noise_2d(x, y);
            value = (value + 1.0) * 0.5; // [-1,1] → [0,1]

            if matches!(world_type, WorldType::Archipelago) {
                value = value * value; // усиливаем острова
            }
            value
        })
        .collect();

    // Создаём временный heightmap для эрозии
    let mut heightmap = Heightmap {
        width,
        height,
        data,
    };

    // === 2. Эрозия ===
    heightmap.apply_thermal_erosion(3, 0.02);
    heightmap.apply_hydraulic_erosion(seed, (width * height / 100) as usize, 0.01);

    // === 3. Оценка уровня моря ===
    let mut sorted = heightmap.data.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let current_sea_level =
        sorted[(((1.0 - target_land_ratio) * sorted.len() as f32) as usize).min(sorted.len() - 1)];

    // === 4. Маска океана и добавление островов ===
    let ocean_mask: Vec<bool> = heightmap
        .data
        .par_iter()
        .map(|&h| h < current_sea_level)
        .collect();

    let island_strength = island_density * 0.3;
    heightmap.data = heightmap
        .data
        .into_par_iter()
        .enumerate()
        .map(|(i, h)| {
            if ocean_mask[i] {
                let x = (i % width as usize) as f32;
                let y = (i / width as usize) as f32;
                let nx = x / width_f;
                let ny = y / height_f;

                let mut island_gen = FastNoiseLite::new();
                island_gen.set_seed(Some(seed.wrapping_add(2_000_000) as i32));
                island_gen.set_noise_type(Some(NoiseType::OpenSimplex2));
                island_gen.set_frequency(Some(6.0 / width_f));
                island_gen.set_fractal_type(Some(FractalType::FBm));
                island_gen.set_fractal_octaves(Some(3));
                island_gen.set_fractal_lacunarity(Some(2.0));
                island_gen.set_fractal_gain(Some(0.4));

                let island_val = island_gen.get_noise_2d(nx, ny);
                let island_val = (island_val + 1.0) * 0.5; // [-1,1] → [0,1]
                h + island_val * island_strength
            } else {
                h
            }
        })
        .collect();

    // === 5. Нормализация под целевую долю суши ===
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

    // Подбор сдвига для точного соответствия доле суши
    let mut best_offset = 0.0;
    let mut best_diff = f32::INFINITY;
    for i in 0..100 {
        let offset = (i as f32) / 100.0 - 0.5;
        let land_ratio = heightmap.data.iter().filter(|&&h| h + offset > 0.5).count() as f32
            / heightmap.data.len() as f32;
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
