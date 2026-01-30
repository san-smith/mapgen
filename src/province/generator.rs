// src/province/generator.rs
use crate::biome::BiomeMap;
use crate::heightmap::Heightmap;
use crate::province::water::WaterType;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ProvinceSeed {
    pub x: f32,
    pub y: f32,
    pub weight: f32,
    pub is_land: bool, // Добавлено для четкого разделения при генерации
}

pub fn generate_province_seeds(
    heightmap: &Heightmap,
    biome_map: &BiomeMap,
    water_type: &[WaterType],
    num_land: usize,
    num_sea: usize,
    seed: u64,
) -> Vec<ProvinceSeed> {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);

    let mut candidates = Vec::new();

    // Сбор кандидатов для суши
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if water_type[idx] == WaterType::Land {
                let h = heightmap.data[idx];
                let b = biome_map.data[idx];
                let temp = 1.0 - h.abs();
                let humid = match b {
                    crate::biome::Biome::Swamp | crate::biome::Biome::TropicalRainforest => 1.0,
                    crate::biome::Biome::Grassland | crate::biome::Biome::TemperateForest => 0.7,
                    _ => 0.3,
                };
                let weight = temp * humid * (1.0 - h.abs());
                candidates.push(ProvinceSeed {
                    x: x as f32,
                    y: y as f32,
                    weight,
                    is_land: true,
                });
            }
        }
    }

    candidates.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected = Vec::with_capacity(num_land + num_sea);
    if !candidates.is_empty() && num_land > 0 {
        let step = (candidates.len() - 1) / num_land.max(1);
        for i in 0..num_land {
            let idx = (i * step).min(candidates.len() - 1);
            selected.push(candidates[idx].clone());
        }
    }

    // Добавление морских семян
    let mut sea_points = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if water_type[y * width + x] == WaterType::Ocean {
                sea_points.push((x as f32, y as f32));
            }
        }
    }

    for _ in 0..num_sea {
        if sea_points.is_empty() {
            break;
        }
        let i = rng.gen_range(0..sea_points.len());
        let (x, y) = sea_points.remove(i);
        selected.push(ProvinceSeed {
            x,
            y,
            weight: 0.5,
            is_land: false,
        });
    }

    selected
}

pub fn generate_provinces_from_seeds(
    heightmap: &Heightmap,
    biome_map: &BiomeMap,
    water_type: &[WaterType],
    seeds: &[ProvinceSeed],
) -> Vec<crate::province::Province> {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let total = width * height;

    let mut province_id_map: Vec<Option<u32>> = vec![None; total];
    let mut provinces: Vec<crate::province::Province> = Vec::with_capacity(seeds.len());
    let mut queue = std::collections::VecDeque::new();

    // ШАГ 1: Инициализация (и суша, и море)
    for (pid, seed) in seeds.iter().enumerate() {
        let x = seed.x as usize;
        let y = seed.y as usize;
        let idx = y * width + x;

        if idx < total {
            province_id_map[idx] = Some(pid as u32);
            provinces.push(crate::province::Province {
                id: pid as u32,
                name: format!("Prov_{}", pid),
                is_land: seed.is_land,
                biome: None,
                center: (seed.x, seed.y),
                area: 1,
                pixels: vec![(x as u32, y as u32)],
            });
            queue.push_back((x, y, pid as u32));
        }
    }

    // ШАГ 2: Flood Fill с проверкой типа поверхности
    const DIRECTIONS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

    while let Some((x, y, pid)) = queue.pop_front() {
        let is_land_province = provinces[pid as usize].is_land;

        for &(dx, dy) in &DIRECTIONS {
            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, (height - 1) as i32) as usize;
            let nidx = ny * width + nx;

            if province_id_map[nidx].is_none() {
                let target_is_land = water_type[nidx] == WaterType::Land;

                // Ключевое исправление: провинция растет только по своему типу поверхности
                if is_land_province == target_is_land {
                    province_id_map[nidx] = Some(pid);
                    provinces[pid as usize].pixels.push((nx as u32, ny as u32));
                    provinces[pid as usize].area += 1;
                    queue.push_back((nx, ny, pid));
                }
            }
        }
    }

    // ШАГ 3: Финализация (центры и биомы)
    for province in &mut provinces {
        if province.pixels.is_empty() {
            continue;
        }

        let sum_x: f32 = province.pixels.iter().map(|p| p.0 as f32).sum();
        let sum_y: f32 = province.pixels.iter().map(|p| p.1 as f32).sum();
        province.center = (sum_x / province.area as f32, sum_y / province.area as f32);

        if province.is_land {
            let mut biome_count = HashMap::new();
            for &(px, py) in &province.pixels {
                let idx = (py as usize) * width + (px as usize);
                *biome_count.entry(biome_map.data[idx]).or_insert(0) += 1;
            }
            province.biome = biome_count
                .into_iter()
                .max_by_key(|&(_, count)| count)
                .map(|(b, _)| b);
        }
    }

    provinces
}
