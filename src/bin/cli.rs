// src/bin/cli.rs
//! CLI-интерфейс генератора карт для Chronicles of Realms
//!
//! Этот бинарный файл предоставляет командную строку для генерации процедурных миров
//! с полным экспортом данных в изображения и JSON-файлы.
//!
//! ## Конвейер генерации
//!
//! 1. **Загрузка конфигурации** — чтение параметров из TOML-файла
//! 2. **Генерация карты высот** — процедурная генерация рельефа с эрозией
//! 3. **Климат и биомы** — расчёт температуры, влажности и назначение биомов
//! 4. **Классификация воды** — разделение на океаны и озёра
//! 5. **Генерация рек** — гидрологическое моделирование на основе рельефа
//! 6. **Генерация провинций** — разбиение мира на административные единицы
//! 7. **Слияние мелких провинций** — оптимизация для геймплея
//! 8. **Группировка в регионы** — формирование крупных географических зон
//! 9. **Поиск стратегических точек** — идентификация портов, устьев, перевалов
//! 10. **Экспорт результатов** — сохранение изображений и данных в JSON
//!
//! ## Использование
//! ```bash
//! # Генерация мира из конфигурации
//! cargo run --release -- --config world.toml --output output/
//!
//! # Пример конфигурации (world.toml)
//! seed = 42
//! width = 2048
//! height = 1024
//! world_type = "EarthLike"
//! [climate]
//! global_temperature_offset = 0.0
//! global_humidity_offset = 0.0
//! ```
//!
//! ## Выходные файлы
//! - `heightmap.png` — карта высот (градации серого)
//! - `normals.png` — карта нормалей для шейдинга
//! - `biomes.png` — карта биомов (цветовая схема)
//! - `rivers.png` — гидрографическая сеть
//! - `provinces.png` — административное деление на провинции
//! - `regions.png` — группировка провинций в регионы
//! - `provinces.json` — данные провинций (геометрия, биомы, типы)
//! - `regions.json` — данные регионов (состав провинций, цвета)

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

// Уровень моря (фиксированный для всех миров)
const SEA_LEVEL: f32 = 0.5;

/// Параметры командной строки генератора карт
#[derive(Parser, Debug)]
#[command(
    name = "mapgen",
    author = "Chronicles of Realms Team",
    version,
    about = "Procedural world generator for Chronicles of Realms",
    long_about = "Generates a complete world map with heightmap, biomes, provinces, regions, and rivers.\n\
                  Outputs PNG images and JSON data for game integration."
)]
struct Cli {
    /// Путь к конфигурационному файлу в формате TOML
    ///
    /// Файл должен содержать параметры генерации: сид, размеры, тип мира, климат.
    /// Пример структуры см. в документации модуля `mapgen::config`.
    #[arg(short, long, value_name = "FILE")]
    config: PathBuf,

    /// Каталог для сохранения результатов генерации
    ///
    /// Все изображения и JSON-файлы будут сохранены в этот каталог.
    /// Каталог будет создан автоматически, если не существует.
    /// По умолчанию: `./output`
    #[arg(short, long, default_value = "output", value_name = "DIR")]
    output: PathBuf,
}

/// Сериализуемая версия провинции для экспорта в JSON
///
/// Эта структура оптимизирована для хранения и передачи данных:
/// - Использует массив `[f32; 2]` вместо кортежа `(f32, f32)` для совместимости с JSON
/// - Переименовывает поле `province_type` в `type` для краткости
/// - Сохраняет все необходимые данные для игрового движка
#[derive(Serialize, Debug)]
struct SerializableProvince {
    /// Уникальный идентификатор провинции
    id: u32,

    /// Цвет провинции в формате HEX (#rrggbb)
    ///
    /// Используется для визуальной идентификации на карте.
    color: String,

    /// Центр масс провинции в пиксельных координатах
    ///
    /// Формат: `[x, y]`
    /// Используется для размещения городов и отображения названий.
    center: [f32; 2],

    /// Площадь провинции в пикселях
    ///
    /// Определяет игровую значимость провинции (налоги, рекрутинг).
    area: usize,

    /// Тип провинции (континент/остров/океан)
    ///
    /// Сериализуется как строка в нижнем регистре:
    /// - `"continental"`
    /// - `"island"`
    /// - `"oceanic"`
    #[serde(rename = "type")]
    province_type: mapgen::province::ProvinceType,

    /// Имеет ли провинция выход к морю
    ///
    /// Определяет возможность строительства портов и морской торговли.
    coastal: bool,

    /// Биомный состав провинции
    ///
    /// Ключ — название биома (например, `"TemperateForest"`),
    /// Значение — доля биома в провинции (0.0–1.0).
    ///
    /// Пример:
    /// ```json
    /// {
    ///   "TemperateForest": 0.65,
    ///   "Grassland": 0.25,
    ///   "Swamp": 0.1
    /// }
    /// ```
    biomes: std::collections::HashMap<String, f32>,
}

/// Сериализуемая версия региона для экспорта в JSON
///
/// Эта структура содержит минимальный набор данных для игрового движка:
/// - Идентификатор и цвет для визуализации
/// - Список провинций для логических операций
#[derive(Serialize)]
struct SerializableRegion {
    /// Уникальный идентификатор региона
    id: u32,

    /// Цвет региона в формате HEX (#rrggbb)
    ///
    /// Используется для визуальной идентификации на карте регионов.
    color: String,

    /// Список идентификаторов провинций, входящих в регион
    ///
    /// Порядок не гарантируется. Все провинции имеют одинаковый тип поверхности.
    province_ids: Vec<u32>,
}

/// Основная функция CLI-генератора
///
/// Реализует полный конвейер генерации мира от карты высот до экспорта данных.
/// Возвращает `Ok(())` при успешной генерации или ошибку при сбое.
///
/// # Этапы генерации
/// 1. Загрузка конфигурации из TOML
/// 2. Генерация карты высот с эрозией
/// 3. Расчёт климата (температура, влажность, ветры)
/// 4. Назначение биомов на основе климата и высоты
/// 5. Классификация водных поверхностей (океаны vs озёра)
/// 6. Генерация гидрографической сети (реки)
/// 7. Разбиение мира на провинции (семена + рост)
/// 8. Слияние мелких провинций для улучшения геймплея
/// 9. Группировка провинций в регионы
/// 10. Поиск стратегических точек (порты, устья, перевалы)
/// 11. Экспорт всех результатов в изображения и JSON
///
/// # Пример вызова
/// ```bash
/// cargo run -- --config world.toml --output output/
/// ```
#[allow(clippy::too_many_lines)] // CLI-бинарник допускает длинную функцию main()
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // === ЭТАП 0: Парсинг аргументов командной строки ===
    let cli = Cli::parse();

    // Создаём каталог вывода (рекурсивно)
    fs::create_dir_all(&cli.output)?;
    println!("📁 Вывод: {}", cli.output.display());

    // === ЭТАП 1: Загрузка конфигурации ===
    println!("🔍 Загрузка конфигурации из {}...", cli.config.display());
    let params = WorldGenerationParams::from_toml_file(cli.config.to_str().unwrap())?;
    println!(
        "⚙️  Сид: {}, Тип мира: {:?}, Размер: {}×{}",
        params.seed, params.world_type, params.width, params.height
    );

    // Масштабный коэффициент для определения минимального размера провинций
    // и выбранных октавов шума для генерации карты высот.
    let scale_factor = params.width as f32 / 512.0;

    // === ЭТАП 2: Генерация карты высот ===
    println!(
        "🌍 Генерация карты высот (размер: {}×{})...",
        params.width, params.height
    );
    let terrain = if params.terrain == mapgen::config::TerrainSettings::default() {
        params.world_type.default_terrain()
    } else {
        params.terrain.clone()
    };

    let heightmap = generate_heightmap(
        params.seed,
        params.width,
        params.height,
        scale_factor,
        params.world_type,
        params.world_type.adjusted_island_density(),
        &terrain,
    );
    println!("✅ Карта высот сгенерирована");

    // === ЭТАП 3: Генерация климата ===
    println!("🌡️  Генерация климата...");
    let (temperature, winds) = generate_climate_maps(
        params.seed,
        params.width,
        params.height,
        &heightmap.data,
        params.climate.global_temperature_offset,
        params.climate.polar_amplification,
        params.climate.climate_latitude_exponent,
        SEA_LEVEL,
    );

    let humidity = calculate_humidity(
        params.width,
        params.height,
        &heightmap.data,
        &winds,
        SEA_LEVEL,
        params.climate.global_humidity_offset,
    );
    println!("✅ Климат сгенерирован");

    // === ЭТАП 4: Назначение биомов ===
    println!("🌿 Назначение биомов...");
    let biome_map = assign_biomes(&heightmap, &temperature, &humidity, SEA_LEVEL);
    println!("✅ Биомы назначены");

    // === ЭТАП 5: Сохранение карты биомов ===
    println!("🖼️  Сохранение карты биомов...");
    biome_map.save_as_png(cli.output.join("biomes.png").to_str().unwrap())?;
    println!("✅ biomes.png сохранён");

    // === ЭТАП 6: Классификация воды и генерация рек ===
    println!("💧 Классификация водных поверхностей...");
    let target_water_ratio = 1.0 - params.world_type.target_land_ratio();
    let water_type = classify_water(&heightmap, SEA_LEVEL, Some(target_water_ratio));
    println!("✅ Вода классифицирована");

    println!("🌊 Генерация рек...");
    let river_map = generate_rivers(&heightmap, &biome_map);
    println!("✅ Реки сгенерированы");

    println!("🖼️  Сохранение карты рек...");
    river_map.save_as_png(cli.output.join("rivers.png").to_str().unwrap())?;
    println!("✅ rivers.png сохранён");

    // === ЭТАП 7: Сохранение карты нормалей (для шейдинга в движке) ===
    let normals_path = cli.output.join("normals.png");
    println!(
        "⛰️  Сохранение карты нормалей в {}...",
        normals_path.display()
    );
    heightmap.save_normals_as_png(normals_path.to_str().unwrap())?;
    println!("✅ normals.png сохранён");

    // === ЭТАП 8: Генерация провинций ===
    println!("🗺️  Генерация провинций...");

    // Расчёт распределения провинций по типу поверхности
    let land_pixels_count = water_type.iter().filter(|&&t| t == WaterType::Land).count();
    let total_pixels = (params.width * params.height) as usize;
    let land_ratio = land_pixels_count as f32 / total_pixels as f32;
    let total_provinces = terrain.total_provinces;

    // Балансировка: 70% суша / 30% море по умолчанию
    let land_priority_ratio = 0.7;
    let mut num_land = (total_provinces as f32 * land_priority_ratio).round() as usize;
    let mut num_sea = total_provinces - num_land;

    // Защита от деления на ноль
    if num_land == 0 {
        num_land = 1;
    }
    if num_sea == 0 {
        num_sea = 1;
    }

    // Для водянистых миров (архипелаги) увеличиваем долю морских провинций
    if land_ratio < 0.3 {
        num_sea = (total_provinces as f32 * 0.5).round() as usize;
        num_land = total_provinces - num_sea;
    }

    println!(
        "📊 Распределение провинций: суша={} ({}%), море={} ({}%)",
        num_land,
        (num_land as f32 / total_provinces as f32 * 100.0).round(),
        num_sea,
        (num_sea as f32 / total_provinces as f32 * 100.0).round()
    );

    // Генерация семян провинций
    println!("🌱 Генерация семян провинций...");
    let seeds = generate_province_seeds(
        &heightmap,
        &biome_map,
        &water_type,
        num_land,
        num_sea,
        params.seed,
    );
    println!("✅ Семена сгенерированы: {num_land} суша, {num_sea} море");

    // Рост провинций от семян
    println!("📈 Рост провинций от семян...");
    let (mut all_provinces, pixel_to_id) =
        generate_provinces_from_seeds(&heightmap, &biome_map, &water_type, &seeds);
    println!("✅ Провинции сгенерированы: {}", all_provinces.len());

    // === ЭТАП 9: Слияние мелких провинций ===
    println!("🔨 Объединение мелких провинций...");
    let mut graph =
        build_province_graph_with_map(&all_provinces, &pixel_to_id, params.width, params.height);
    merge_small_provinces(&mut all_provinces, &graph, scale_factor);
    println!("✅ Мелкие провинции объединены");

    // Перестроение графа после слияния
    graph =
        build_province_graph_with_map(&all_provinces, &pixel_to_id, params.width, params.height);

    // === ЭТАП 10: Сохранение карты провинций ===
    println!("🖼️  Сохранение карты провинций...");
    let province_map = ProvinceMap::from_pixel_map(params.width, params.height, &pixel_to_id);
    province_map.save_as_png(
        &all_provinces,
        cli.output.join("provinces.png").to_str().unwrap(),
    )?;
    println!("✅ provinces.png сохранён");

    // === ЭТАП 11: Группировка в регионы ===
    println!(
        "🧩 Группировка провинций в регионы (цель: ~{} провинций на регион)...",
        8
    );
    let target_region_size = 8;
    let regions = group_provinces_into_regions(&all_provinces, &graph, target_region_size);
    println!("✅ Регионы сформированы: {} регионов", regions.len());

    // === ЭТАП 12: Сохранение карты регионов ===
    println!("🖼️  Сохранение карты регионов...");
    let region_map = RegionMap::from_pixel_map(params.width, params.height, &pixel_to_id, &regions);
    region_map.save_as_png(cli.output.join("regions.png").to_str().unwrap(), &regions)?;
    println!("✅ regions.png сохранён");

    // === ЭТАП 13: Поиск стратегических точек ===
    println!("🎯 Поиск стратегических точек...");
    let strategic_points =
        find_strategic_points(&all_provinces, &river_map, &biome_map, &pixel_to_id);
    println!(
        "✅ Найдено стратегических точек: {} (порты: {}, устья: {}, перевалы: {})",
        strategic_points.len(),
        strategic_points
            .iter()
            .filter(|p| matches!(p, mapgen::strategic::StrategicPoint::Port { .. }))
            .count(),
        strategic_points
            .iter()
            .filter(|p| matches!(p, mapgen::strategic::StrategicPoint::Estuary { .. }))
            .count(),
        strategic_points
            .iter()
            .filter(|p| matches!(p, mapgen::strategic::StrategicPoint::Pass { .. }))
            .count()
    );

    // === ЭТАП 14: Экспорт данных в JSON ===
    println!("📦 Экспорт данных провинций в provinces.json...");
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
    println!(
        "✅ provinces.json сохранён ({} провинций)",
        serializable_provinces.len()
    );

    println!("📦 Экспорт данных регионов в regions.json...");
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
    println!(
        "✅ regions.json сохранён ({} регионов)",
        serializable_regions.len()
    );

    // === ЗАВЕРШЕНИЕ ===
    println!(
        "\n✅ Генерация завершена успешно! Результаты сохранены в: {}",
        cli.output.display()
    );
    println!("\n📊 Статистика мира:");
    println!("   • Провинций: {}", serializable_provinces.len());
    println!("   • Регионов: {}", serializable_regions.len());
    println!("   • Стратегических точек: {}", strategic_points.len());
    println!("   • Площадь суши: {:.1}%", land_ratio * 100.0);

    Ok(())
}
