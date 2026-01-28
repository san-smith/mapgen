use clap::Parser;
use mapgen::climate::calculate_humidity;
use mapgen::config::TerrainSettings;
use mapgen::{
    WorldGenerationParams, biome::assign_biomes, climate::generate_climate_maps,
    generate_heightmap, rivers::generate_rivers,
};
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
    let terrain = if params.terrain != TerrainSettings::default() {
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

    let height_path = cli.output.join("height.png");
    println!("💾 Сохранение height.png в {:?}", height_path);
    heightmap.save_as_png(height_path.to_str().unwrap())?;

    // === Климат и биомы ===
    println!("🌡️  Генерация климата и биомов...");
    let (temperature, winds) = generate_climate_maps(
        params.seed,
        params.width,
        params.height,
        &heightmap.data,
        params.climate.global_temperature_offset,
        params.climate.polar_amplification,
        params.climate.climate_latitude_exponent,
    );

    // 3. Вычисляем влажность с учетом гор и ветров
    // Функция прогонит "воздух" по кругу, создавая дождевые тени за горами
    let sea_level = 0.5; // Уровень моря после вашей нормализации
    let humidity = calculate_humidity(
        params.width,
        params.height,
        &heightmap.data,
        &winds,
        sea_level,
        params.climate.global_humidity_offset,
    );

    let biome_map = assign_biomes(&heightmap, &temperature, &humidity, sea_level);

    let biomes_path = cli.output.join("biomes.png");
    println!("🎨 Сохранение biomes.png в {:?}", biomes_path);
    biome_map.save_as_png(biomes_path.to_str().unwrap())?;

    // === Реки ===
    println!("🌊 Генерация рек...");
    let river_map = generate_rivers(&heightmap, &biome_map);

    let rivers_path = cli.output.join("rivers.png");
    println!("💧 Сохранение rivers.png в {:?}", rivers_path);
    river_map.save_as_png(rivers_path.to_str().unwrap())?;

    println!("\n✅ Все изображения сохранены в {:?}", cli.output);
    Ok(())
}
