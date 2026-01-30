use crate::biome::{Biome, BiomeMap};
use crate::heightmap::Heightmap;
use image::{ImageBuffer, Luma};
use imageproc::drawing::draw_filled_circle_mut;

pub struct RiverMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub fn generate_rivers(heightmap: &Heightmap, biomemap: &BiomeMap) -> RiverMap {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;

    // 1. Накопление потока (Flow Accumulation)
    let mut flow = vec![1.0f32; width * height];

    // Сортируем индексы от вершин к низинам
    let mut indices: Vec<usize> = (0..(width * height)).collect();
    indices.sort_by(|&a, &b| {
        heightmap.data[b]
            .partial_cmp(&heightmap.data[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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

    for &idx in &indices {
        let biome = biomemap.data[idx];

        // Реки не текут во льдах и не начинаются в пустынях
        if biome == Biome::Ice
            || biome == Biome::Ocean
            || biome == Biome::DeepOcean
            || biome == Biome::IcyOcean
            || biome == Biome::FrozenOcean
        {
            flow[idx] = 0.0;
            continue;
        }

        let x = (idx % width) as i32;
        let y = (idx / width) as i32;

        let mut min_h = heightmap.data[idx];
        let mut target_idx = idx;

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

    // 2. Рендеринг
    let mut rivers_img: ImageBuffer<Luma<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(heightmap.width, heightmap.height, Luma([0]));

    // Настраиваемые параметры:
    let flow_threshold = 400.0; // Порог появления видимой реки
    let max_flow_thickness = 3000.0; // Порог максимальной толщины

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let current_flow = flow[idx];
            let biome = biomemap.data[idx];

            // Условия отрисовки: достаточно воды, не океан, не лед
            if current_flow > flow_threshold
                && biome != Biome::Ocean
                && biome != Biome::DeepOcean
                && biome != Biome::IcyOcean
                && biome != Biome::FrozenOcean
                && biome != Biome::Ice
            {
                // Толщина: от 1 до 5 пикселей в зависимости от объема воды
                let thickness = (1.0 + (current_flow / max_flow_thickness) * 4.0).min(5.0);

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
    pub fn save_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img: ImageBuffer<Luma<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.data.clone())
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
