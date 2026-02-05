// src/province/generator.rs
use crate::biome::BiomeMap;
use crate::heightmap::Heightmap;
use crate::province::water::WaterType;
use crate::province::{Province, ProvinceType};
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const DIRECTIONS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

#[derive(Debug, Clone)]
pub struct ProvinceSeed {
    pub x: f32,
    pub y: f32,
    pub weight: f32,
    pub is_land: bool,
}

fn hash_to_color(id: u32) -> String {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish();
    let r = ((hash >> 16) % 156) as u8 + 50; // 50..205
    let g = ((hash >> 8) % 156) as u8 + 50;
    let b = (hash % 156) as u8 + 50;
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[must_use]
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

/// Возвращает (провинции, карта пикселей → `province_id`)
#[allow(clippy::too_many_lines)]
#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn generate_provinces_from_seeds(
    heightmap: &Heightmap,
    biome_map: &BiomeMap,
    water_type: &[WaterType],
    seeds: &[ProvinceSeed],
) -> (Vec<Province>, Vec<u32>) {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let total = width * height;

    let mut province_id_map: Vec<Option<u32>> = vec![None; total];
    let mut provinces: Vec<Province> = Vec::with_capacity(seeds.len());
    let mut queue = std::collections::VecDeque::new();

    // ШАГ 1: Инициализация
    for (pid, seed) in seeds.iter().enumerate() {
        let x = seed.x as usize;
        let y = seed.y as usize;
        let idx = y * width + x;

        if idx < total {
            province_id_map[idx] = Some(pid as u32);
            provinces.push(Province {
                id: pid as u32,
                name: format!("Prov_{pid}"),
                province_type: if seed.is_land {
                    ProvinceType::Continental
                } else {
                    ProvinceType::Oceanic
                },
                is_land: seed.is_land,
                coastal: false,
                center: (0.0, 0.0),
                area: 0,
                biomes: HashMap::new(),
                color: hash_to_color(pid as u32),
            });
            queue.push_back((x, y, pid as u32));
        }
    }

    // ШАГ 2: Flood Fill с агрегацией данных
    while let Some((x, y, pid)) = queue.pop_front() {
        let province = &mut provinces[pid as usize];

        // Агрегация данных
        province.area += 1;
        let biome_name = format!("{:?}", biome_map.data[y * width + x]);
        *province.biomes.entry(biome_name).or_insert(0.0) += 1.0;
        province.center.0 += x as f32;
        province.center.1 += y as f32;

        // Проверка прибрежности (только для суши)
        if province.is_land {
            for &(dx, dy) in &DIRECTIONS {
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, (height - 1) as i32) as usize;
                let nidx = ny * width + nx;
                if water_type[nidx] != WaterType::Land {
                    province.coastal = true;
                    break;
                }
            }
        }

        // Добавление соседей
        for &(dx, dy) in &DIRECTIONS {
            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, (height - 1) as i32) as usize;
            let nidx = ny * width + nx;

            if province_id_map[nidx].is_none() {
                let neighbor_is_land = water_type[nidx] == WaterType::Land;
                if province.is_land == neighbor_is_land {
                    province_id_map[nidx] = Some(pid);
                    queue.push_back((nx, ny, pid));
                }
            }
        }
    }

    // ШАГ 3: Финализация
    for province in &mut provinces {
        if province.area > 0 {
            province.center.0 /= province.area as f32;
            province.center.1 /= province.area as f32;

            for count in province.biomes.values_mut() {
                *count /= province.area as f32;
            }

            province.province_type = if !province.is_land {
                ProvinceType::Oceanic
            } else if province.coastal && province.area < 500 {
                ProvinceType::Island
            } else {
                ProvinceType::Continental
            };
        }
    }

    // ШАГ 4: Заполнение оставшихся пикселей
    println!(
        "🔍 Заполнение {} непокрытых пикселей...",
        province_id_map.iter().filter(|o| o.is_none()).count()
    );

    // Собираем центры всех провинций
    let centers: Vec<(f32, f32)> = provinces.iter().map(|p| p.center).collect();

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if province_id_map[idx].is_none() {
                let mut min_d2 = f32::MAX;
                let mut best_pid = 0;
                for (pid, &(cx, cy)) in centers.iter().enumerate() {
                    let d2 = (x as f32 - cx).powi(2) + (y as f32 - cy).powi(2);
                    if d2 < min_d2 {
                        min_d2 = d2;
                        best_pid = pid as u32;
                    }
                }
                province_id_map[idx] = Some(best_pid);
                // Обновляем данные провинции
                provinces[best_pid as usize].area += 1;
            }
        }
    }

    // Преобразуем карту в Vec<u32>
    let pixel_to_id: Vec<u32> = province_id_map
        .into_iter()
        .map(|opt| opt.unwrap())
        .collect();

    (provinces, pixel_to_id)
}
