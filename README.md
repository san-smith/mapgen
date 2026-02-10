# 🗺️ mapgen — Procedural World Generator for Chronicles of Realms

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org)

- **Climate simulation** (temperature, humidity, wind patterns)
- **Biome distribution** (16 distinct biomes with natural transitions)
- **Administrative division** (provinces & regions with strategic points)
- **Hydrography** (river networks with flow accumulation)
- **Full determinism** (identical results for identical seeds)

Perfect for 4X strategy games, RPGs, and any project requiring rich, playable worlds.

---

## Features

| Feature                  | Description                                                               |
| ------------------------ | ------------------------------------------------------------------------- |
| **Multiple World Types** | Earth-like, Supercontinent, Archipelago, Mediterranean, Ice Age, Desert   |
| **Realistic Climate**    | Latitude-based temperature, orographic precipitation, polar amplification |
| **Natural Erosion**      | Thermal (talus slopes) + hydraulic (river valleys) modeling               |
| **Province System**      | Administrative units with biomes, coastal status, movement costs          |
| **Region Grouping**      | Logical grouping of provinces (continents, sea basins)                    |
| **Strategic Points**     | Ports, estuaries, mountain passes for gameplay depth                      |
| **Seamless Projection**  | Cylindrical projection with longitude wrapping (no edge artifacts)        |
| **WASM Support**         | Compile to WebAssembly for browser-based demos                            |
| **JSON Export**          | Complete world data for game engine integration                           |
| **Moddable**             | TOML configuration + culture-based naming system (WIP)                    |

---

## Quick Start

### Prerequisites

- Rust 1.70+ (`rustup update stable`)
- Cargo

### Build & Run

```bash
# Clone the repository
git clone https://github.com/san-smith/mapgen.git
cd mapgen

# Build release version (recommended for performance)
cargo build --release

# Generate a world using default configuration
cargo run --release -- --config examples/earthlike.toml --output output/
```

### Example Output

```sh
output/
├── biomes.png        # Biome distribution map
├── heightmap.png     # Grayscale heightmap
├── normals.png       # Normal map for 3D shading
├── provinces.png     # Province boundaries
├── regions.png       # Region grouping
├── rivers.png        # River network
├── provinces.json    # Province data (id, center, biomes, type)
└── regions.json      # Region data (id, color, province_ids)
```

---

## Configuration

Create a `world.toml` file to customize world generation:

```toml
seed = 42
width = 2048
height = 1024
world_type = "EarthLike"

[climate]
global_temperature_offset = 0.0   # -1.0 (cold) to +1.0 (hot)
global_humidity_offset = 0.0      # -1.0 (dry) to +1.0 (wet)
polar_amplification = 1.0         # >1.0 = wider polar zones
climate_latitude_exponent = 0.65  # <1.0 = compressed poles

[islands]
island_density = 0.2              # 0.0 (none) to 1.0 (many)
min_island_size = 200

[terrain]
elevation_power = 0.8             # <1.0 = smoother, >1.0 = more rugged
smooth_radius = 1                 # 0 (none) to 5 (very smooth)
mountain_compression = 0.7        # 0.0 (linear) to 1.0 (peaks only)
total_provinces = 120             # Total land + sea provinces
```

**Available world types:**

- `EarthLike` — Balanced continents & oceans (30% land)
- `Supercontinent` — Pangaea-like landmass (70% land)
- `Archipelago` — Scattered islands (15% land)
- `Mediterranean` — Inland sea surrounded by land (25% land)
- `IceAgeEarth` — Expanded ice caps (35% "land" but mostly frozen)
- `DesertMediterranean` — Arid inland sea region (20% land)

---

## Architecture

```sh
mapgen/
├── src/
│   ├── biome/        # Biome assignment based on climate & elevation
│   ├── climate/      # Temperature & humidity simulation
│   ├── config/       # World configuration structures
│   ├── heightmap/    # Heightmap generation & erosion
│   ├── province/     # Province generation, merging, graph analysis
│   │   ├── generator.rs  # Seed placement & flood-fill growth
│   │   ├── merge.rs      # Small province merging
│   │   ├── graph.rs      # Adjacency graph construction
│   │   ├── png.rs        # Province map visualization
│   │   └── water.rs      # Ocean/lake classification
│   ├── region/       # Region grouping (continents, sea basins)
│   ├── rivers/       # Hydrological modeling & river generation
│   ├── strategic/    # Strategic point detection (ports, passes, estuaries)
│   └── lib.rs        # Public API exports
│
├── examples/         # Sample configuration files
└── output/           # Generated world assets (after running CLI)
```

### Generation Pipeline

1. **Heightmap** → 3D noise + cylindrical projection + erosion
2. **Climate** → Temperature (latitude) + Humidity (wind + elevation)
3. **Biomes** → Classification based on height + temperature + humidity
4. **Water** → Ocean/lake classification via BFS from map edges
5. **Rivers** → Flow accumulation + erosion-based carving
6. **Provinces** → Seed placement → Flood-fill growth → Small province merging
7. **Regions** → BFS grouping of provinces by adjacency & land/sea type
8. **Strategic Points** → Detection of ports, estuaries, mountain passes
9. **Export** → PNG visualization + JSON data export

---

## Web Demo

A live WebAssembly demo is available at:
**[https://san-smith.github.io/mapgen-web-demo/](https://san-smith.github.io/mapgen-web-demo/)**

Features:

- Interactive world generation in the browser
- Real-time parameter adjustment (seed, temperature, humidity)
- Layer toggling (heightmap, biomes, provinces, regions)
- Click-to-inspect province details
- No server required — runs entirely client-side

_Source code: <https://github.com/san-smith/mapgen-web-demo>_

---

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
mapgen = { git="https://github.com/san-smith/mapgen", rev="092fd3a" }
```

Generate a world programmatically:

```rust
use mapgen::{
    WorldGenerationParams,
    WorldType,
    generate_heightmap,
    climate::{generate_climate_maps, calculate_humidity},
    biome::assign_biomes,
    province::{
        water::classify_water,
        generator::{generate_province_seeds, generate_provinces_from_seeds},
    },
    region::group_provinces_into_regions,
};

// Configure world parameters
let params = WorldGenerationParams {
    seed: 42,
    width: 2048,
    height: 1024,
    world_type: WorldType::EarthLike,
    ..Default::default()
};

// Generate heightmap
let heightmap = generate_heightmap(
    params.seed,
    params.width,
    params.height,
    params.world_type,
    params.islands.island_density,
    &params.terrain,
);

// Generate climate & biomes
let sea_level = 0.5;
let (temperature, winds) = generate_climate_maps(...);
let humidity = calculate_humidity(...);
let biome_map = assign_biomes(&heightmap, &temperature, &humidity, sea_level);

// Generate provinces
let water_type = classify_water(&heightmap, sea_level);
let seeds = generate_province_seeds(...);
let (provinces, pixel_to_id) = generate_provinces_from_seeds(...);

// Export to game engine
let province_data: Vec<ProvinceData> = provinces.iter().map(|p| {
    ProvinceData {
        id: p.id,
        center: p.center,
        biomes: p.biomes.clone(),
        // ... other fields
    }
}).collect();
```

---

## Game Integration

### Data Format (`provinces.json`)

```json
[
  {
    "id": 42,
    "color": "#a1b2c3",
    "center": [363.65, 314.06],
    "area": 3101,
    "type": "continental",
    "coastal": true,
    "biomes": {
      "TemperateForest": 0.513,
      "Swamp": 0.197,
      "RockyMountain": 0.096,
      "Taiga": 0.173,
      "Tundra": 0.006,
      "TropicalRainforest": 0.009,
      "Ice": 0.005
    }
  }
  // ... more provinces
]
```

### Recommended Game Mechanics

- **Movement Cost**: Use `Biome::movement_cost()` for pathfinding
- **Province Value**: Scale resources by `area` and fertile biomes (`Grassland`, `TemperateForest`)
- **Naval Access**: Coastal provinces enable port construction
- **Strategic Chokepoints**: Mountain passes (`Pass` strategic points) provide defensive bonuses
- **Trade Routes**: Estuaries enable river-to-sea trade bonuses

---

## Determinism Guarantee

All generation is **fully deterministic**:

```rust
// Same seed + same parameters = identical worlds
let world1 = generate_world(seed: 42, ...);
let world2 = generate_world(seed: 42, ...);
assert_eq!(world1.provinces, world2.provinces); // Always true!
```

This enables:

- Reproducible bug reports ("seed 42 crashes at province 147")
- Multiplayer synchronization (share only the seed)
- Procedural content sharing ("try my world: seed 12345")
- Modding support (culture packs applied to identical base worlds)

---

## License

Licensed under either of:

- **MIT License** ([LICENSE-MIT](LICENSE-MIT))
- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

---

## Acknowledgements

- **FastNoise Lite** — High-performance noise generation
- **PetGraph** — Graph algorithms for province adjacency
- **Procedural Worlds** — Inspiration for erosion algorithms
- **Europa Universalis III** — Province/region design inspiration
