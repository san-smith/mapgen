use clap::Parser;
use mapgen::{
    WorldGenerationParams,
    biome::assign_biomes,
    climate::{calculate_humidity, generate_climate_maps},
    generate_heightmap,
    province::{
        generator::{generate_province_seeds, generate_provinces_from_seeds},
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

#[derive(Serialize)]
struct WorldData {
    provinces: Vec<mapgen::province::Province>,
    regions: Vec<mapgen::region::Region>,
    strategic_points: Vec<mapgen::strategic::StrategicPoint>,
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

    println!("🗺️  Генерация провинций...");

    let land_pixels_count = water_type.iter().filter(|&&t| t == WaterType::Land).count();
    let total_pixels = (params.width * params.height) as usize;
    let land_ratio = land_pixels_count as f32 / total_pixels as f32;

    let total_provinces = terrain.total_provinces;

    // Распределяем пропорционально реальной карте, но даем суше больший вес (например, 70% от total_provinces всегда выделяется под сушу)
    let land_priority_ratio = 0.7;

    let mut num_land = (total_provinces as f32 * land_priority_ratio).round() as usize;
    let mut num_sea = total_provinces - num_land;

    // Гарантируем ненулевые значения
    if num_land == 0 {
        num_land = 1;
    }
    if num_sea == 0 {
        num_sea = 1;
    }

    // Если на карте очень мало суши, корректируем пропорции
    if land_ratio < 0.3 {
        num_sea = (total_provinces as f32 * 0.5).round() as usize;
        num_land = total_provinces - num_sea;
    }

    // 1. Генерируем семена для обеих типов поверхностей
    println!("🌱 Генерация семян провинций...");
    let seeds = generate_province_seeds(
        &heightmap,
        &biome_map,
        &water_type,
        num_land,
        num_sea,
        params.seed,
    );

    // 2. Используем Flood Fill от семян (дает более выпуклые и равномерные провинции)
    let mut all_provinces =
        generate_provinces_from_seeds(&heightmap, &biome_map, &water_type, &seeds);

    // 3. Слияние мелких провинций
    println!("🔨 Объединение мелких провинций...");
    let mut graph =
        mapgen::province::graph::build_province_graph(&all_provinces, params.width, params.height);
    merge_small_provinces(&mut all_provinces, &graph);

    graph =
        mapgen::province::graph::build_province_graph(&all_provinces, params.width, params.height);

    let province_map = ProvinceMap::new(params.width, params.height, &all_provinces);
    province_map.save_as_png(cli.output.join("provinces.png").to_str().unwrap())?;

    println!("🧩 Группировка регионов...");
    let target_region_size = 8;
    let regions = group_provinces_into_regions(&all_provinces, &graph, target_region_size);

    println!("🖼️  Сохранение регионов...");
    let region_map = RegionMap::new(params.width, params.height, &all_provinces, &regions);
    region_map.save_as_png(
        cli.output.join("regions.png").to_str().unwrap(),
        &regions,
        &all_provinces,
    )?;

    println!("🎯 Поиск стратегических точек...");
    let strategic_points = find_strategic_points(&all_provinces, &river_map, &biome_map);

    let world_data = WorldData {
        provinces: all_provinces,
        regions,
        strategic_points,
    };

    let world_path = cli.output.join("world.toml");
    fs::write(&world_path, toml::to_string_pretty(&world_data)?)?;

    println!("\n✅ Генерация завершена. Результаты в {:?}", cli.output);
    Ok(())
}
