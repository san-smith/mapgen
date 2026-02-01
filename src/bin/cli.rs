// src/bin/cli.rs
use clap::Parser;
use mapgen::{
    WorldGenerationParams,
    biome::assign_biomes,
    climate::{calculate_humidity, generate_climate_maps},
    generate_heightmap,
    province::{
        generator::{generate_province_seeds, generate_provinces_from_seeds},
        graph::build_province_graph_with_map,
        merge::merge_small_provinces,
        png::ProvinceMap,
        water::{WaterType, classify_water},
    },
    region::{group_provinces_into_regions, png::RegionMap},
    rivers::generate_rivers,
    strategic::find_strategic_points,
};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;

/// Генератор карт для Chronicles of Realms
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Путь к конфигурационному файлу в формате TOML
    #[arg(short, long)]
    config: PathBuf,

    /// Каталог для сохранения изображений (по умолчанию: ./output)
    #[arg(short, long, default_value = "output")]
    output: PathBuf,
}

#[derive(Serialize, Debug)]
struct SerializableProvince {
    id: u32,
    color: String,
    center: [f32; 2],
    area: usize,
    #[serde(rename = "type")]
    province_type: mapgen::province::ProvinceType,
    coastal: bool,
    biomes: std::collections::HashMap<String, f32>,
}

#[derive(Serialize)]
struct SerializableRegion {
    id: u32,
    color: String,
    province_ids: Vec<u32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Создаём каталог вывода
    fs::create_dir_all(&cli.output)?;

    println!("🔍 Загрузка конфигурации...");
    let params = WorldGenerationParams::from_toml_file(cli.config.to_str().unwrap())?;

    println!(
        "🌍 Генерация карты высот (размер: {}×{})...",
        params.width, params.height
    );
    let terrain = if params.terrain != mapgen::config::TerrainSettings::default() {
        params.terrain.clone()
    } else {
        params.world_type.default_terrain()
    };

    let heightmap = generate_heightmap(
        params.seed,
        params.width,
        params.height,
        params.world_type,
        params.islands.island_density,
        &terrain,
    );

    let sea_level = 0.5;
    // === Климат и биомы ===
    println!("🌡️  Генерация климата...");
    let (temperature, winds) = generate_climate_maps(
        params.seed,
        params.width,
        params.height,
        &heightmap.data,
        params.climate.global_temperature_offset,
        params.climate.polar_amplification,
        params.climate.climate_latitude_exponent,
        sea_level,
    );

    let humidity = calculate_humidity(
        params.width,
        params.height,
        &heightmap.data,
        &winds,
        sea_level,
        params.climate.global_humidity_offset,
    );
    let biome_map = assign_biomes(&heightmap, &temperature, &humidity, sea_level);

    println!("🖼️  Сохранение карты биомов...");
    biome_map.save_as_png(cli.output.join("biomes.png").to_str().unwrap())?;

    let water_type = classify_water(&heightmap, sea_level);
    let river_map = generate_rivers(&heightmap, &biome_map);

    println!("🖼️  Сохранение карты рек...");
    river_map.save_as_png(cli.output.join("rivers.png").to_str().unwrap())?;

    let normals_path = cli.output.join("normals.png");
    println!("⛰️  Сохранение normals.png в {:?}", normals_path);
    heightmap.save_normals_as_png(normals_path.to_str().unwrap())?;

    println!("🗺️  Генерация провинций...");

    let land_pixels_count = water_type.iter().filter(|&&t| t == WaterType::Land).count();
    let total_pixels = (params.width * params.height) as usize;
    let land_ratio = land_pixels_count as f32 / total_pixels as f32;

    let total_provinces = terrain.total_provinces;

    let land_priority_ratio = 0.7;
    let mut num_land = (total_provinces as f32 * land_priority_ratio).round() as usize;
    let mut num_sea = total_provinces - num_land;

    if num_land == 0 {
        num_land = 1;
    }
    if num_sea == 0 {
        num_sea = 1;
    }

    if land_ratio < 0.3 {
        num_sea = (total_provinces as f32 * 0.5).round() as usize;
        num_land = total_provinces - num_sea;
    }

    println!("🌱 Генерация семян провинций...");
    let seeds = generate_province_seeds(
        &heightmap,
        &biome_map,
        &water_type,
        num_land,
        num_sea,
        params.seed,
    );

    // 2. Генерация провинций + карта пикселей
    let (mut all_provinces, pixel_to_id) =
        generate_provinces_from_seeds(&heightmap, &biome_map, &water_type, &seeds);

    // 3. Слияние мелких провинций
    println!("🔨 Объединение мелких провинций...");
    let mut graph =
        build_province_graph_with_map(&all_provinces, &pixel_to_id, params.width, params.height);
    merge_small_provinces(&mut all_provinces, &graph);

    // После слияния перестраиваем pixel_to_id и граф
    graph =
        build_province_graph_with_map(&all_provinces, &pixel_to_id, params.width, params.height);

    // 4. Сохранение провинций
    println!("🖼️  Сохранение карты провинций...");
    let province_map = ProvinceMap::from_pixel_map(params.width, params.height, &pixel_to_id);
    province_map.save_as_png(
        &all_provinces,
        cli.output.join("provinces.png").to_str().unwrap(),
    )?;

    // 5. Группировка регионов
    println!("🧩 Группировка регионов...");
    let target_region_size = 8;
    let regions = group_provinces_into_regions(&all_provinces, &graph, target_region_size);

    // 6. Сохранение регионов
    println!("🖼️  Сохранение карты регионов...");
    let region_map = RegionMap::from_pixel_map(params.width, params.height, &pixel_to_id, &regions);
    region_map.save_as_png(cli.output.join("regions.png").to_str().unwrap(), &regions)?;

    // 7. Стратегические точки
    println!("🎯 Поиск стратегических точек...");
    let strategic_points =
        find_strategic_points(&all_provinces, &river_map, &biome_map, &pixel_to_id);

    // 8. Экспорт данных
    println!("📦 Сохранение provinces.json...");
    let serializable_provinces: Vec<SerializableProvince> = all_provinces
        .into_iter()
        .map(|p| SerializableProvince {
            id: p.id,
            color: p.color,
            center: [p.center.0, p.center.1],
            area: p.area,
            province_type: p.province_type,
            coastal: p.coastal,
            biomes: p.biomes,
        })
        .collect();

    let provinces_json = serde_json::to_string_pretty(&serializable_provinces)?;
    fs::write(cli.output.join("provinces.json"), provinces_json)?;

    println!("📦 Сохранение regions.json...");
    let serializable_regions: Vec<SerializableRegion> = regions
        .into_iter()
        .map(|r| SerializableRegion {
            id: r.id,
            color: r.color,
            province_ids: r.province_ids,
        })
        .collect();

    let regions_json = serde_json::to_string_pretty(&serializable_regions)?;
    fs::write(cli.output.join("regions.json"), regions_json)?;

    println!("\n✅ Генерация завершена. Результаты в {:?}", cli.output);
    Ok(())
}
