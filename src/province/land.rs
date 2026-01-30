use crate::biome::BiomeMap;
use crate::heightmap::Heightmap;
use crate::province::Province;
use crate::province::water::WaterType;

pub fn generate_land_provinces(
    heightmap: &Heightmap,
    biome_map: &BiomeMap,
    water_type: &[WaterType],
    start_id: u32, // Смещение для ID
) -> Vec<Province> {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let mut visited = vec![false; width * height];
    let mut provinces = Vec::new();
    let mut current_id = start_id;

    const DIRECTIONS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if visited[idx] || water_type[idx] != WaterType::Land {
                continue;
            }

            let mut pixels = Vec::new();
            let mut queue = std::collections::VecDeque::new();
            let start_biome = biome_map.data[idx];
            queue.push_back((x as i32, y as i32));
            visited[idx] = true;

            while let Some((cx, cy)) = queue.pop_front() {
                pixels.push((cx as u32, cy as u32));
                for &(dx, dy) in &DIRECTIONS {
                    let nx = (cx + dx).rem_euclid(width as i32);
                    let ny = (cy + dy).clamp(0, height as i32 - 1);
                    let nidx = (ny as usize) * width + (nx as usize);

                    if !visited[nidx]
                        && water_type[nidx] == WaterType::Land
                        && biome_map.data[nidx] == start_biome
                    {
                        visited[nidx] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }

            provinces.push(Province {
                id: current_id,
                name: format!("Land_{}", current_id),
                is_land: true,
                biome: Some(start_biome),
                center: (0.0, 0.0), // Центр считается позже
                area: pixels.len(),
                pixels,
            });
            current_id += 1;
        }
    }
    provinces
}
