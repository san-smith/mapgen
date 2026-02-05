// src/region/mod.rs
pub mod png;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub id: u32,
    pub name: String,
    pub color: String, // ← ДОБАВЛЕНО
    pub province_ids: Vec<u32>,
}

fn hash_region_color(region_id: u32) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    region_id.hash(&mut hasher);
    let hash = hasher.finish();
    let r = ((hash >> 16) % 156) as u8 + 50;
    let g = ((hash >> 8) % 156) as u8 + 50;
    let b = (hash % 156) as u8 + 50;
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn group_provinces_into_regions(
    provinces: &[crate::province::Province],
    graph: &petgraph::graph::UnGraph<u32, ()>,
    target_size: usize,
) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut assigned = HashSet::new();
    let prov_map: HashMap<u32, &crate::province::Province> =
        provinces.iter().map(|p| (p.id, p)).collect();
    let node_map: HashMap<u32, petgraph::graph::NodeIndex> =
        graph.node_indices().map(|idx| (graph[idx], idx)).collect();

    let mut region_id_counter = 0;

    for province in provinces {
        if assigned.contains(&province.id) {
            continue;
        }

        let is_land_reg = province.is_land;
        let mut reg_pids = vec![province.id];
        assigned.insert(province.id);

        let mut queue = std::collections::VecDeque::new();
        queue.push_back(province.id);

        while reg_pids.len() < target_size && !queue.is_empty() {
            let curr_id = queue.pop_front().unwrap();
            if let Some(&node_idx) = node_map.get(&curr_id) {
                for neighbor_idx in graph.neighbors(node_idx) {
                    let n_id = graph[neighbor_idx];
                    if !assigned.contains(&n_id)
                        && let Some(n_prov) = prov_map.get(&n_id)
                        && n_prov.is_land == is_land_reg
                    {
                        assigned.insert(n_id);
                        reg_pids.push(n_id);
                        queue.push_back(n_id);
                        if reg_pids.len() >= target_size {
                            break;
                        }
                    }
                }
            }
        }

        regions.push(Region {
            id: region_id_counter,
            name: format!(
                "{}_{}",
                if is_land_reg { "Land" } else { "Sea" },
                region_id_counter
            ),
            color: hash_region_color(region_id_counter), // ← ГЕНЕРАЦИЯ ЦВЕТА
            province_ids: reg_pids,
        });
        region_id_counter += 1;
    }
    regions
}
