// src/province/merge.rs
use crate::province::Province;
use petgraph::graph::UnGraph;
use std::collections::HashMap;

/// Минимальная площадь провинции в пикселях.
const MIN_AREA_THRESHOLD: usize = 50;

pub fn merge_small_provinces(provinces: &mut Vec<Province>, graph: &UnGraph<u32, ()>) {
    let mut merged_count = 0;

    loop {
        let small_province_id = provinces
            .iter()
            .find(|p| p.area < MIN_AREA_THRESHOLD)
            .map(|p| p.id);

        if let Some(small_id) = small_province_id {
            if merge_one_small_province(provinces, graph, small_id) {
                merged_count += 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    println!("🧹 Слито {} мелких провинций.", merged_count);
}

fn merge_one_small_province(
    provinces: &mut Vec<Province>,
    graph: &UnGraph<u32, ()>,
    small_id: u32,
) -> bool {
    let small_idx = if let Some(idx) = provinces.iter().position(|p| p.id == small_id) {
        idx
    } else {
        return false;
    };

    let is_land = provinces[small_idx].is_land;

    let prov_map: HashMap<u32, usize> = provinces
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id, i))
        .collect();
    let node_map: HashMap<u32, petgraph::graph::NodeIndex> =
        graph.node_indices().map(|idx| (graph[idx], idx)).collect();

    let small_node_idx = if let Some(&idx) = node_map.get(&small_id) {
        idx
    } else {
        return false;
    };

    let largest_neighbor_id = graph
        .neighbors(small_node_idx)
        .filter_map(|n_idx| {
            let n_id = graph[n_idx];
            prov_map.get(&n_id).map(|&idx| &provinces[idx])
        })
        .filter(|&n_prov| n_prov.is_land == is_land)
        .max_by_key(|&n_prov| n_prov.area)
        .map(|p| p.id);

    if let Some(large_id) = largest_neighbor_id {
        let large_idx = prov_map[&large_id];

        // Убедимся, что small_idx < large_idx для корректного split_at_mut
        let (small_idx, large_idx) = if small_idx < large_idx {
            (small_idx, large_idx)
        } else {
            (large_idx, small_idx)
        };

        // Разделяем вектор на две части
        let (left, right) = provinces.split_at_mut(large_idx);
        let small_prov = &left[small_idx];
        let large_prov = &mut right[0]; // large_idx теперь 0 в правой части

        let small_area = small_prov.area as f32;
        let large_area = large_prov.area as f32;
        let total_area = small_area + large_area;

        // Обновляем центр (взвешенное среднее)
        large_prov.center.0 =
            (large_prov.center.0 * large_area + small_prov.center.0 * small_area) / total_area;
        large_prov.center.1 =
            (large_prov.center.1 * large_area + small_prov.center.1 * small_area) / total_area;

        // Объединяем биомы
        for (biome, &small_ratio) in &small_prov.biomes {
            let large_ratio = large_prov.biomes.entry(biome.clone()).or_insert(0.0);
            *large_ratio = (*large_ratio * large_area + small_ratio * small_area) / total_area;
        }

        // Обновляем площадь
        large_prov.area = total_area as usize;

        // Обновляем coastal
        large_prov.coastal = large_prov.coastal || small_prov.coastal;

        // Удаляем мелкую провинцию (индекс мог измениться из-за swap)
        let actual_small_idx = if small_idx < large_idx {
            small_idx
        } else {
            large_idx
        };
        provinces.remove(actual_small_idx);

        true
    } else {
        false
    }
}
