// src/province/graph.rs
use crate::province::Province;
use petgraph::graph::UnGraph;
use std::collections::{HashMap, HashSet};

/// Строит граф смежности провинций.
///
/// # Аргументы
/// * `provinces` — список провинций,
/// * `pixel_to_id` — карта пикселей → `province_id` (размер: width × height),
/// * `width`, `height` — размеры карты.
#[must_use]
pub fn build_province_graph_with_map(
    provinces: &[Province],
    pixel_to_id: &[u32],
    width: u32,
    height: u32,
) -> UnGraph<u32, ()> {
    let mut graph = UnGraph::new_undirected();
    let mut id_to_node = HashMap::new();

    // Добавляем узлы для всех провинций
    for province in provinces {
        let node = graph.add_node(province.id);
        id_to_node.insert(province.id, node);
    }

    let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];
    let mut edges = HashSet::new();

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let current_id = pixel_to_id[idx];

            // Пропускаем пиксели без провинции
            if current_id == u32::MAX {
                continue;
            }

            // Проверяем соседей
            for &(dx, dy) in &directions {
                let nx = (x.cast_signed() + dx).rem_euclid(width.cast_signed()) as u32;
                let ny = (y.cast_signed() + dy).clamp(0, height.cast_signed() - 1) as u32;
                let nidx = (ny * width + nx) as usize;
                let neighbor_id = pixel_to_id[nidx];

                // Пропускаем недействительные или одинаковые ID
                if neighbor_id == u32::MAX || neighbor_id == current_id {
                    continue;
                }

                // Создаём упорядоченную пару для избежания дубликатов
                let (a, b) = if current_id < neighbor_id {
                    (current_id, neighbor_id)
                } else {
                    (neighbor_id, current_id)
                };

                // Добавляем ребро, если его ещё нет
                if edges.insert((a, b))
                    && let (Some(&node_a), Some(&node_b)) = (id_to_node.get(&a), id_to_node.get(&b))
                {
                    graph.add_edge(node_a, node_b, ());
                }
            }
        }
    }

    graph
}
