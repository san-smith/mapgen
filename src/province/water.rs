use crate::heightmap::Heightmap;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterType {
    Ocean,
    Lake,
    Land,
}

const DIRECTIONS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

#[allow(clippy::needless_range_loop)]
#[must_use]
pub fn classify_water(heightmap: &Heightmap, sea_level: f32) -> Vec<WaterType> {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let total = width * height;

    let mut water_type = vec![WaterType::Land; total];
    let mut visited = vec![false; total];

    // Очередь для BFS
    let mut queue = VecDeque::new();

    // Добавляем все прибрежные пиксели как "океан"
    for x in 0..width {
        for y in &[0, height - 1] {
            let idx = y * width + x;
            if heightmap.data[idx] < sea_level {
                water_type[idx] = WaterType::Ocean;
                visited[idx] = true;
                queue.push_back((x as i32, *y as i32));
            }
        }
    }

    // Также добавляем левый/правый край (бесшовность!)
    for y in 0..height {
        for &x in &[0, width - 1] {
            let idx = y * width + x;
            if heightmap.data[idx] < sea_level && !visited[idx] {
                water_type[idx] = WaterType::Ocean;
                visited[idx] = true;
                queue.push_back((x as i32, y as i32));
            }
        }
    }

    // BFS от краёв
    while let Some((x, y)) = queue.pop_front() {
        for &(dx, dy) in &DIRECTIONS {
            let nx = (x + dx).rem_euclid(width as i32);
            let ny = (y + dy).clamp(0, height as i32 - 1) as usize;
            let nidx = ny * width + nx as usize;

            if !visited[nidx] && heightmap.data[nidx] < sea_level {
                water_type[nidx] = WaterType::Ocean;
                visited[nidx] = true;
                queue.push_back((nx, ny as i32));
            }
        }
    }

    // Всё остальное — озёра
    for i in 0..total {
        if heightmap.data[i] < sea_level && water_type[i] == WaterType::Land {
            water_type[i] = WaterType::Lake;
        }
    }

    water_type
}
