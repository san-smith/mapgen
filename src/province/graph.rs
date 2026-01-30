use crate::province::Province;
use petgraph::graph::UnGraph;
use std::collections::{HashMap, HashSet};

pub fn build_province_graph(provinces: &[Province], width: u32, height: u32) -> UnGraph<u32, ()> {
    let mut graph = UnGraph::new_undirected();
    let mut id_to_node = HashMap::new();
    let mut pixel_to_id = vec![u32::MAX; (width * height) as usize];

    for province in provinces {
        let node = graph.add_node(province.id);
        id_to_node.insert(province.id, node);
        for &(x, y) in &province.pixels {
            pixel_to_id[(y * width + x) as usize] = province.id;
        }
    }

    let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];
    let mut edges = HashSet::new();

    for province in provinces {
        for &(x, y) in &province.pixels {
            for &(dx, dy) in &directions {
                let nx = (x as i32 + dx).rem_euclid(width as i32) as u32;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as u32;
                let n_id = pixel_to_id[(ny * width + nx) as usize];

                if n_id != u32::MAX && n_id != province.id {
                    let (a, b) = if province.id < n_id {
                        (province.id, n_id)
                    } else {
                        (n_id, province.id)
                    };
                    if edges.insert((a, b)) {
                        graph.add_edge(id_to_node[&a], id_to_node[&b], ());
                    }
                }
            }
        }
    }
    graph
}
