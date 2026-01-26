use clap::Parser;
use mapgen::WorldGenerationParams;
use std::path::PathBuf;

/// Генератор карт для Chronicles of Realms
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Путь к конфигурационному файлу в формате TOML
    #[arg(short, long)]
    config: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Парсим аргументы командной строки
    let cli = Cli::parse();

    // Загружаем конфиг из файла
    let params = WorldGenerationParams::from_toml_file(cli.config.to_str().unwrap())?;

    // Выводим параметры в человекочитаемом виде
    println!("✅ Загружены параметры генерации:");
    println!();
    println!("  Seed:                  {}", params.seed);
    println!("  Размер:                {} × {}", params.width, params.height);
    println!("  Тип мира:              {:?}", params.world_type);
    println!("  Целевая доля суши:     {:.1}%", params.world_type.target_land_ratio() * 100.0);
    println!();
    println!("  Климат:");
    println!("    Глобальная температура:  {:+.2}", params.climate.global_temperature_offset);
    println!("    Глобальная влажность:    {:+.2}", params.climate.global_humidity_offset);
    println!("    Полярное усиление:       {:.2}", params.climate.polar_amplification);
    println!();
    println!("  Острова:");
    println!("    Плотность:               {:.2}", params.islands.island_density);
    println!("    Мин. размер (пиксели):   {}", params.islands.min_island_size);
    println!();
    println!("  Регионы:");
    println!("    Количество:              {}", params.num_regions);
    println!("    Масштаб морских пров.:   ×{:.1}", params.sea_province_scale);

    Ok(())
}