use crate::config::WorldType;
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use image::{ImageBuffer, Luma};
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

    let mut noise = FastNoiseLite::new();
    noise.set_seed(Some(seed as i32));
    noise.set_noise_type(Some(NoiseType::OpenSimplex2)); // Более естественный шум
    noise.set_fractal_type(Some(FractalType::FBm)); // Фрактальный шум для деталей
    noise.set_fractal_octaves(Some(5)); // Больше октав = больше деталей

    // Частота теперь работает правильно, так как мы будем передавать x, y
    // Чем меньше значение, тем крупнее объекты
    noise.set_frequency(Some(0.005));

    // === 1. Генерация базового рельефа ===
    // Используем плоский вектор сразу, Rayon отлично с этим справляется
    let mut data: Vec<f32> = (0..(width * height))
        .into_par_iter()
        .map(|i| {
            let x = (i % width) as f32;
            let y = (i / width) as f32;

            // Передаем сырые координаты x, y.
            // noise.get_noise_2d сам применит частоту (frequency)
            let mut value = noise.get_noise_2d(x, y);

            // Приводим из [-1, 1] в [0, 1]
            value = (value + 1.0) * 0.5;

            // Небольшая коррекция типа мира
            if matches!(world_type, WorldType::Archipelago) {
                value = value * value; // Делаем низменности шире (острова острее)
            }
            value
        })
        .collect();

    // === 2. Оценка уровня моря ===
    let mut sorted = data.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let current_sea_level =
        sorted[(((1.0 - target_land_ratio) * sorted.len() as f32) as usize).min(sorted.len() - 1)];

    // === 3. Маска океана и добавление островов ===
    let ocean_mask: Vec<bool> = data.par_iter().map(|&h| h < current_sea_level).collect();

    let island_strength = island_density * 0.3;
    data = data
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

    // === 4. Нормализация под целевую долю суши ===
    let min_h = data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_h = data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

    if max_h > min_h {
        for h in &mut data {
            *h = (*h - min_h) / (max_h - min_h);
        }
    }

    // Подбор сдвига для точного соответствия доле суши
    let mut best_offset = 0.0;
    let mut best_diff = f32::INFINITY;
    for i in 0..100 {
        let offset = (i as f32) / 100.0 - 0.5;
        let land_ratio =
            data.iter().filter(|&&h| h + offset > 0.5).count() as f32 / data.len() as f32;
        let diff = (land_ratio - target_land_ratio).abs();
        if diff < best_diff {
            best_diff = diff;
            best_offset = offset;
        }
    }

    for h in &mut data {
        *h = (*h + best_offset).clamp(0.0, 1.0);
    }

    Heightmap {
        width,
        height,
        data,
    }
}
