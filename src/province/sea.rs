use crate::province::Province;
use crate::province::water::WaterType;
use rand::{Rng, SeedableRng};

pub fn generate_sea_provinces_voronoi(
    water_type: &[WaterType],
    width: u32,
    num_sea: usize,
    seed: u64,
    start_id: u32,
) -> Vec<Province> {
    let width = width as usize;

    // Берем Ocean И Lake
    let water_pixels: Vec<(usize, usize)> = water_type
        .iter()
        .enumerate()
        .filter(|&(_, t)| *t != WaterType::Land)
        .map(|(i, _)| (i % width, i / width))
        .collect();

    if water_pixels.is_empty() || num_sea == 0 {
        return Vec::new();
    }

    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    let mut centers = Vec::new();
    for _ in 0..num_sea {
        let p = water_pixels[rng.gen_range(0..water_pixels.len())];
        centers.push((p.0 as f32, p.1 as f32));
    }

    let mut groups: std::collections::HashMap<u32, Vec<(u32, u32)>> =
        std::collections::HashMap::new();

    for &(x, y) in &water_pixels {
        let mut min_d2 = f32::MAX;
        let mut best_pid = 0;
        for (pid, &(cx, cy)) in centers.iter().enumerate() {
            let d2 = (x as f32 - cx).powi(2) + (y as f32 - cy).powi(2);
            if d2 < min_d2 {
                min_d2 = d2;
                best_pid = pid as u32;
            }
        }
        groups
            .entry(best_pid)
            .or_default()
            .push((x as u32, y as u32));
    }

    groups
        .into_iter()
        .map(|(pid, pixels)| {
            let area = pixels.len();
            Province {
                id: start_id + pid,
                name: format!("Sea_{}", start_id + pid),
                is_land: false,
                biome: None,
                center: (0.0, 0.0),
                area,
                pixels,
            }
        })
        .collect()
}
