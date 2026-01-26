use clap::Parser;
use mapgen::{WorldGenerationParams, generate_heightmap};
use std::path::PathBuf;

/// Генератор карт для Chronicles of Realms
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Путь к конфигурационному файлу в формате TOML
    #[arg(short, long)]
    config: PathBuf,

    /// Путь для сохранения height.png (по умолчанию: ./height.png)
    #[arg(short, long, default_value = "height.png")]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    println!("🔍 Загрузка конфигурации...");
    let params = WorldGenerationParams::from_toml_file(cli.config.to_str().unwrap())?;

    println!(
        "Генерация карты высот (размер: {}×{})...",
        params.width, params.height
    );
    let heightmap = generate_heightmap(
        params.seed,
        params.width,
        params.height,
        params.world_type,
        params.islands.island_density,
    );

    println!("Сохранение в {:?}", cli.output);
    heightmap.save_as_png(cli.output.to_str().unwrap())?;

    println!("\nГотово! Heightmap сохранена.");
    Ok(())
}
