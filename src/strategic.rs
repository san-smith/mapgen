// src/strategic.rs
use crate::biome::BiomeMap;
use crate::province::Province;
use crate::rivers::RiverMap;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub enum StrategicPoint {
    Port { province_id: u32 },
    Pass { province_id: u32 },
    Estuary { province_id: u32 },
    Strait { province_id: u32 },
}

#[must_use]
pub fn find_strategic_points(
    provinces: &[Province],
    river_map: &RiverMap,
    biome_map: &BiomeMap,
    pixel_to_id: &[u32], // Новый аргумент: карта пикселей → province_id
) -> Vec<StrategicPoint> {
    let mut points = Vec::new();
    let width = biome_map.width as usize;
    let height = biome_map.height as usize;

    // Создаём обратную мапу: province_id → индекс в provinces

    for province in provinces {
        if !province.is_land {
            continue;
        }

        let mut has_river = false;
        let mut has_mountain = false;

        // Проверяем все пиксели карты
        for y in 0..height {
            for x in 0..width {
                let idx = y * width + x;
                if pixel_to_id[idx] != province.id {
                    continue;
                }

                // Проверка реки
                if river_map.data[idx] > 0 {
                    has_river = true;
                }

                // Проверка гор через биомы
                let biome_name = format!("{:?}", biome_map.data[idx]);
                if biome_name == "RockyMountain" || biome_name == "GlacialMountain" {
                    has_mountain = true;
                }
            }
        }

        // Определяем тип стратегической точки
        if province.coastal && has_river {
            points.push(StrategicPoint::Estuary {
                province_id: province.id,
            });
        } else if province.coastal {
            points.push(StrategicPoint::Port {
                province_id: province.id,
            });
        } else if has_mountain {
            points.push(StrategicPoint::Pass {
                province_id: province.id,
            });
        }
    }

    points
}
