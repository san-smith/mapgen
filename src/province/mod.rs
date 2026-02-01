pub mod generator;
pub mod graph;
pub mod merge;
pub mod png;
pub mod water;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProvinceType {
    Continental,
    Island,
    Oceanic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Province {
    pub id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub province_type: ProvinceType,
    pub color: String, // "#rrggbb"
    pub is_land: bool,
    pub coastal: bool,
    pub center: (f32, f32),
    pub area: usize,
    /// доля каждого биома
    pub biomes: HashMap<String, f32>,
}
