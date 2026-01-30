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
            // Функция теперь возвращает bool, а не Option<bool>
            if merge_one_small_province(provinces, graph, small_id) {
                merged_count += 1;
            } else {
                break; // Если слить не удалось (нет соседа), выходим из цикла поиска
            }
        } else {
            break; // Мелких провинций больше нет
        }
    }
    println!("🧹 Слито {} мелких провинций.", merged_count);
}

fn merge_one_small_province(
    provinces: &mut Vec<Province>,
    graph: &UnGraph<u32, ()>,
    small_id: u32,
) -> bool {
    // Используем if let/match вместо оператора ?, чтобы не менять тип возврата на Option
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

    // Также обрабатываем node_map.get явно, без '?'
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
        // Используем std::mem::take для эффективного перемещения вектора
        let small_pixels = std::mem::take(&mut provinces[small_idx].pixels);
        provinces[large_idx].pixels.extend(small_pixels);
        provinces[large_idx].area = provinces[large_idx].pixels.len();

        // Удаляем мелкую провинцию
        provinces.remove(small_idx);

        // Граф должен быть перестроен в cli.rs после этой функции.

        true // Успешно слили, возвращаем bool
    } else {
        false // Не нашли подходящего соседа для слияния, возвращаем bool
    }
}
