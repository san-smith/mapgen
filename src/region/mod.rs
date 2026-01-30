pub mod png;

use std::collections::{HashMap, HashSet};

use crate::province::Province;
use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub id: u32,
    pub name: String,
    pub province_ids: Vec<u32>,
}

pub fn group_provinces_into_regions(
    provinces: &[Province],
    graph: &petgraph::graph::UnGraph<u32, ()>,
    target_size: usize,
) -> Vec<Region> {
    let mut regions = Vec::new();
    let mut assigned = HashSet::new();
    // Быстрый доступ к провинции по ID
    let prov_map: HashMap<u32, &Province> = provinces.iter().map(|p| (p.id, p)).collect();
    // Мапа ID провинции -> NodeIndex
    let node_map: HashMap<u32, NodeIndex> =
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
            let node_idx = node_map[&curr_id];

            for neighbor_idx in graph.neighbors(node_idx) {
                let n_id = graph[neighbor_idx];
                let n_prov = prov_map[&n_id];

                if !assigned.contains(&n_id) && n_prov.is_land == is_land_reg {
                    assigned.insert(n_id);
                    reg_pids.push(n_id);
                    queue.push_back(n_id);
                    if reg_pids.len() >= target_size {
                        break;
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
            province_ids: reg_pids,
        });
        region_id_counter += 1;
    }
    regions
}
