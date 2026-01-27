use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::heightmap::Heightmap;

#[derive(Debug, Clone)]
pub struct RiverMap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f32>, // flow accumulation
}

/// Генерирует карту рек через flow accumulation
pub fn generate_rivers(heightmap: &Heightmap) -> RiverMap {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let total = width * height;

    // Инициализируем flow = 1 для каждого пикселя
    let mut flow = vec![1.0; total];

    // Обходим пиксели от самых высоких к самым низким
    let mut indices: Vec<usize> = (0..total).collect();
    indices.sort_by(|&a, &b| {
        heightmap.data[b]
            .partial_cmp(&heightmap.data[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Направления соседей (D8)
    let directions = [
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
        let x = idx % width;
        let y = idx / width;

        // Найдём самого низкого соседа
        let mut min_height = heightmap.data[idx];
        let mut min_idx = idx;

        for &(dx, dy) in &directions {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                let nidx = (ny as usize) * width + (nx as usize);
                if heightmap.data[nidx] < min_height {
                    min_height = heightmap.data[nidx];
                    min_idx = nidx;
                }
            }
        }

        // Если есть более низкий сосед — передаём flow
        if min_idx != idx {
            flow[min_idx] += flow[idx];
        }
    }

    RiverMap {
        width: heightmap.width,
        height: heightmap.height,
        data: flow,
    }
}

impl RiverMap {
    pub fn to_grayscale_image(&self) -> Vec<u8> {
        // Логарифмическая шкала для лучшей видимости
        let max_flow = *self
            .data
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(&1.0);
        self.data
            .par_iter()
            .map(|&f| {
                let log_f = f.ln().max(0.0) / max_flow.ln().max(1.0);
                (log_f.clamp(0.0, 1.0) * 255.0) as u8
            })
            .collect()
    }

    pub fn save_as_png(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img: image::ImageBuffer<image::Luma<u8>, Vec<u8>> =
            image::ImageBuffer::from_raw(self.width, self.height, self.to_grayscale_image())
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
