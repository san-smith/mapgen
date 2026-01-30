pub mod generator;
pub mod graph;
pub mod land;
pub mod merge;
pub mod png;
pub mod sea;
pub mod water;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Province {
    pub id: u32,
    pub name: String,
    pub is_land: bool,
    pub biome: Option<crate::biome::Biome>,
    pub center: (f32, f32), // (x, y)
    pub area: usize,
    pub pixels: Vec<(u32, u32)>, // список координат
}
