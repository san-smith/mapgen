use serde::Serialize;

use crate::biome::Biome;
use crate::province::Province;
use crate::rivers::RiverMap;

#[derive(Debug, Clone, Serialize)]
pub enum StrategicPoint {
    Port { province_id: u32 },
    Pass { province_id: u32 },
    Estuary { province_id: u32 },
    Strait { province_id: u32 },
}

pub fn find_strategic_points(
    provinces: &[Province],
    river_map: &RiverMap,
    biome_map: &crate::biome::BiomeMap,
) -> Vec<StrategicPoint> {
    let mut points = Vec::new();

    for province in provinces {
        if province.is_land {
            let mut has_coast = false;
            let mut has_river = false;
            let mut has_mountain = false;

            for &(x, y) in &province.pixels {
                let idx = (y as usize) * (biome_map.width as usize) + (x as usize);
                if biome_map.data[idx] == Biome::DeepOcean
                    || biome_map.data[idx] == Biome::Ocean
                    || biome_map.data[idx] == Biome::IcyOcean
                    || biome_map.data[idx] == Biome::FrozenOcean
                {
                    has_coast = true;
                }
                if river_map.data[idx] > 0 {
                    has_river = true;
                }
                if province.biome == Some(Biome::RockyMountain)
                    || province.biome == Some(Biome::GlacialMountain)
                {
                    has_mountain = true;
                }
            }

            if has_coast && has_river {
                points.push(StrategicPoint::Estuary {
                    province_id: province.id,
                });
            } else if has_coast {
                points.push(StrategicPoint::Port {
                    province_id: province.id,
                });
            } else if has_mountain {
                points.push(StrategicPoint::Pass {
                    province_id: province.id,
                });
            }
        }
    }

    points
}
