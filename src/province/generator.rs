// src/province/generator.rs
//! Генератор провинций мира
//!
//! Этот модуль реализует двухэтапный алгоритм создания провинций:
//! 1. **Размещение семян** — выбор оптимальных начальных точек для сухопутных и морских регионов
//! 2. **Рост провинций** — распространение от семян с агрегацией географических данных
//!
//! ## Алгоритм генерации
//!
//! ### Этап 1: Генерация семян (`generate_province_seeds`)
//! - **Суша**: выбираются точки с высокой "пригодностью" на основе:
//!   - Температуры (умеренные зоны предпочтительнее)
//!   - Влажности (леса и равнины > пустыни и горы)
//!   - Высоты (избегаем экстремальных высот)
//! - **Море**: случайное равномерное распределение по океаническим регионам
//!
//! ### Этап 2: Рост провинций (`generate_provinces_from_seeds`)
//! 1. **Инициализация** — размещение семян на карте
//! 2. **Flood Fill** — распространение от семян с ограничением по типу поверхности:
//!    - Суша распространяется только на сушу
//!    - Море распространяется только на море
//! 3. **Агрегация данных** во время роста:
//!    - Подсчёт площади
//!    - Накопление координат для центра масс
//!    - Сбор статистики по биомам
//!    - Определение прибрежности
//! 4. **Финализация** — нормализация данных:
//!    - Вычисление центра масс: `(Σx / площадь, Σy / площадь)`
//!    - Нормализация биомов: `доля = количество_пикселей / площадь`
//!    - Определение типа провинции (континент/остров/океан)
//! 5. **Заполнение "дыр"** — назначение оставшихся пикселей ближайшей провинции
//!
//! ## Особенности реализации
//! - **Детерминированность**: все этапы зависят только от сида и входных данных
//! - **Тип поверхности**: строгое разделение суша/море предотвращает артефакты
//! - **Прибрежность**: определяется автоматически при первом контакте с водой
//! - **Цвета**: генерируются детерминированно на основе `id` для стабильности

use crate::biome::BiomeMap;
use crate::heightmap::Heightmap;
use crate::province::water::WaterType;
use crate::province::{Province, ProvinceType};
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 4 основных направления для распространения провинций (без диагоналей)
///
/// Используется в алгоритме Flood Fill для:
/// - Создания более "естественных" границ (без диагональных артефактов)
/// - Обеспечения связности провинций (4-связность вместо 8-связности)
/// - Упрощения определения прибрежности
const DIRECTIONS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

const MIN_SEA_POINTS: usize = 10;

/// Семя провинции — начальная точка для роста территории
///
/// Семена размещаются в "благоприятных" локациях на основе географических критериев.
/// Каждое семя порождает одну провинцию через алгоритм Flood Fill.
#[derive(Debug, Clone)]
pub struct ProvinceSeed {
    /// X-координата семени в пикселях (`0..ширина_карты`)
    pub x: f32,
    /// Y-координата семени в пикселях (`0..высота_карты`)
    pub y: f32,
    /// Вес "благоприятности" локации для размещения семени
    ///
    /// Вычисляется на основе:
    /// - Температуры (умеренные зоны имеют больший вес)
    /// - Влажности (плодородные биомы имеют больший вес)
    /// - Высоты (избегаем экстремальных высот)
    ///
    /// Используется для сортировки кандидатов при выборе семян.
    pub weight: f32,
    /// Тип поверхности семени
    ///
    /// Определяет, будет ли провинция сухопутной или морской:
    /// - `true` — семя размещается на суше → континентальная/островная провинция
    /// - `false` — семя размещается в океане → океаническая провинция
    pub is_land: bool,
}

/// Генерирует детерминированный цвет для провинции на основе её идентификатора
///
/// # Алгоритм
/// 1. Хешируем `id` через `DefaultHasher`
/// 2. Извлекаем компоненты RGB из хеша:
///    - Красный: биты 16-23
///    - Зелёный: биты 8-15
///    - Синий: биты 0-7
/// 3. Смещаем в диапазон 50-205 для обеспечения хорошего контраста
/// 4. Форматируем как HEX-строку `"#rrggbb"`
///
/// # Гарантии
/// - Одинаковый `id` → одинаковый цвет (детерминированность)
/// - Цвета достаточно контрастны для визуального различения
/// - Избегаем слишком тёмных (`<50`) и слишком светлых (`>205`) оттенков
///
/// # Пример
/// ```rust
/// let color = hash_to_color(42);
/// assert_eq!(color, "#a1b2c3"); // примерное значение
/// ```
fn hash_to_color(id: u32) -> String {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish();
    let r = ((hash >> 16) % 156) as u8 + 50; // 50..205
    let g = ((hash >> 8) % 156) as u8 + 50;
    let b = (hash % 156) as u8 + 50;
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Генерирует набор семян для провинций на основе географических критериев
///
/// # Алгоритм выбора сухопутных семян
/// 1. Собираем все точки суши как кандидаты
/// 2. Вычисляем вес каждой точки:
///    ```text
///    weight = температурный_фактор × влажностный_фактор × высотный_фактор
///    ```
///    - Температурный фактор: `1.0 - |высота - 0.5|` (умеренные высоты предпочтительнее)
///    - Влажностный фактор: зависит от биома:
///      - Болота/тропики: 1.0 (максимальная плодородность)
///      - Равнины/леса: 0.7 (хорошая плодородность)
///      - Остальные: 0.3 (низкая плодородность)
///    - Высотный фактор: `1.0 - |высота - 0.5|` (избегаем экстремумов)
/// 3. Сортируем кандидатов по весу по убыванию
/// 4. Равномерно выбираем `num_land` точек из отсортированного списка
///
/// # Алгоритм выбора морских семян
/// 1. Собираем все точки океана
/// 2. Случайно выбираем `num_sea` точек (равномерное распределение)
/// 3. Вес морских семян фиксирован (`0.5`) — не влияет на алгоритм
///
/// # Параметры
/// * `heightmap` — карта высот для определения рельефа
/// * `biome_map` — карта биомов для оценки плодородности
/// * `water_type` — классификация воды (суша/океан) для разделения типов семян
/// * `num_land` — количество сухопутных семян для генерации
/// * `num_sea` — количество морских семян для генерации
/// * `seed` — сид для детерминированной генерации морских семян
///
/// # Возвращает
/// Вектор `ProvinceSeed` длиной `num_land + num_sea`
///
/// # Пример
/// ```rust
/// let seeds = generate_province_seeds(
///     &heightmap,
///     &biome_map,
///     &water_type,
///     80,  // 80 сухопутных провинций
///     40,  // 40 морских провинций
///     42,  // сид
/// );
/// ```
#[must_use]
pub fn generate_province_seeds(
    heightmap: &Heightmap,
    biome_map: &BiomeMap,
    water_type: &[WaterType],
    num_land: usize,
    num_sea: usize,
    seed: u64,
) -> Vec<ProvinceSeed> {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);

    let mut candidates = Vec::new();

    // Собираем кандидатов на суше с весом на основе плодородности
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            if water_type[idx] == WaterType::Land {
                let h = heightmap.data[idx];
                let b = biome_map.data[idx];
                let temp = 1.0 - h.abs();
                let humid = match b {
                    crate::biome::Biome::Swamp | crate::biome::Biome::TropicalRainforest => 1.0,
                    crate::biome::Biome::Grassland | crate::biome::Biome::TemperateForest => 0.7,
                    _ => 0.3,
                };
                let weight = temp * humid * (1.0 - h.abs());
                candidates.push(ProvinceSeed {
                    x: x as f32,
                    y: y as f32,
                    weight,
                    is_land: true,
                });
            }
        }
    }

    // Сортируем по весу (наиболее благоприятные локации первыми)
    candidates.sort_by(|a, b| {
        b.weight
            .partial_cmp(&a.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut selected = Vec::with_capacity(num_land + num_sea);

    // Выбираем сухопутные семена равномерно из отсортированного списка
    if !candidates.is_empty() && num_land > 0 {
        let step = (candidates.len() - 1) / num_land.max(1);
        for i in 0..num_land {
            let idx = (i * step).min(candidates.len() - 1);
            selected.push(candidates[idx].clone());
        }
    }
    // Собираем точки океана для морских семян
    let mut sea_points = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if water_type[y * width + x] == WaterType::Ocean {
                sea_points.push((x as f32, y as f32));
            }
        }
    }

    // Валидация количества океанических точек
    if sea_points.len() < MIN_SEA_POINTS {
        eprintln!(
            "Внимание: найдено только {} океанических точек (рекомендуется > {})",
            sea_points.len(),
            MIN_SEA_POINTS
        );
    }

    // Логирование статистики генерации
    println!(
        "Генерация провинций: {} сухопутных семян, {} морских семян, {} океанических точек",
        selected.iter().filter(|s| s.is_land).count(),
        num_sea,
        sea_points.len()
    );

    // Случайно выбираем морские семена
    let actual_sea = num_sea.min(sea_points.len());
    for _ in 0..actual_sea {
        if sea_points.is_empty() {
            break;
        }
        let i = rng.gen_range(0..sea_points.len());
        let (x, y) = sea_points.remove(i);
        selected.push(ProvinceSeed {
            x,
            y,
            weight: 0.5,
            is_land: false,
        });
    }

    if actual_sea < num_sea {
        eprintln!("Внимание: запрошено {num_sea} морских провинций, создано только {actual_sea}");
    }

    selected
}

/// Генерирует провинции мира из набора семян через алгоритм роста
///
/// # Пошаговый алгоритм
///
/// ## Шаг 1: Инициализация
/// - Каждое семя становится центром новой провинции
/// - Создаётся карта `province_id_map` для отслеживания принадлежности пикселей
/// - Инициализируется очередь для алгоритма Flood Fill
///
/// ## Шаг 2: Flood Fill с агрегацией данных
/// Для каждого пикселя из очереди:
/// 1. **Агрегация площади**: `провинция.площадь += 1`
/// 2. **Агрегация центра масс**:
///    ```text
///    провинция.центр.x += x
///    провинция.центр.y += y
///    ```
/// 3. **Агрегация биомов**:
///    ```text
///    провинция.биомы[текущий_биом] += 1.0
///    ```
/// 4. **Определение прибрежности** (только для суши):
///    - Проверяем 4 соседа
///    - Если хотя бы один сосед — вода → `провинция.coastal = true`
/// 5. **Добавление соседей в очередь**:
///    - Добавляем только соседей того же типа поверхности (суша→суша, море→море)
///    - Игнорируем уже занятые пиксели
///
/// ## Шаг 3: Финализация провинций
/// Для каждой провинции:
/// 1. **Нормализация центра масс**:
///    ```text
///    центр.x = Σx / площадь
///    центр.y = Σy / площадь
///    ```
/// 2. **Нормализация биомов**:
///    ```text
///    доля[биом] = количество_пикселей[биом] / площадь
///    ```
/// 3. **Определение типа провинции**:
///    - Море → `Oceanic`
///    - Суша + прибрежная + площадь < 500 → `Island`
///    - Суша + остальные случаи → `Continental`
///
/// ## Шаг 4: Заполнение оставшихся пикселей
/// - Некоторые пиксели могут остаться непокрытыми из-за изолированных областей
/// - Каждый непокрытый пиксель назначается ближайшей провинции по расстоянию до центра
/// - Площадь и биомы провинции обновляются для сохранения консистентности данных
///
/// # Параметры
/// * `heightmap` — карта высот для определения рельефа
/// * `biome_map` — карта биомов для агрегации состава провинций
/// * `water_type` — классификация воды для разделения типов поверхности
/// * `seeds` — набор семян, сгенерированный через `generate_province_seeds`
///
/// # Возвращает
/// Кортеж `(провинции, карта_пикселей)`:
/// * `провинции` — вектор структур `Province` со всей агрегированной информацией
/// * `карта_пикселей` — вектор `u32` длиной `ширина × высота`, где каждый элемент — `province_id`
///
/// # Гарантии
/// - Каждый пиксель карты принадлежит ровно одной провинции
/// - Все провинции связны (4-связность)
/// - Суша и море никогда не смешиваются в одной провинции
/// - Алгоритм детерминирован для одинаковых входных данных
///
/// # Пример
/// ```rust
/// let (provinces, pixel_to_id) = generate_provinces_from_seeds(
///     &heightmap,
///     &biome_map,
///     &water_type,
///     &seeds,
/// );
/// ```
#[allow(clippy::too_many_lines)]
#[allow(clippy::missing_panics_doc)]
#[must_use]
pub fn generate_provinces_from_seeds(
    heightmap: &Heightmap,
    biome_map: &BiomeMap,
    water_type: &[WaterType],
    seeds: &[ProvinceSeed],
) -> (Vec<Province>, Vec<u32>) {
    let width = heightmap.width as usize;
    let height = heightmap.height as usize;
    let total = width * height;

    let mut province_id_map: Vec<Option<u32>> = vec![None; total];
    let mut provinces: Vec<Province> = Vec::with_capacity(seeds.len());
    let mut queue = std::collections::VecDeque::new();

    // ШАГ 1: Инициализация — размещение семян
    for (pid, seed) in seeds.iter().enumerate() {
        let x = seed.x as usize;
        let y = seed.y as usize;
        let idx = y * width + x;

        if idx < total {
            province_id_map[idx] = Some(pid as u32);
            let color = hash_to_color(pid as u32);
            // Validate color format (should always be valid, but check for safety)
            if !color.starts_with('#') || color.len() != 7 {
                eprintln!(
                    "Провинция {pid} получила некорректный цвет: {color}. Используется fallback."
                );
            }
            provinces.push(Province {
                id: pid as u32,
                name: format!("Prov_{pid}"),
                province_type: if seed.is_land {
                    ProvinceType::Continental
                } else {
                    ProvinceType::Oceanic
                },
                is_land: seed.is_land,
                coastal: false,
                center: (0.0, 0.0),
                area: 0,
                biomes: HashMap::new(),
                color: hash_to_color(pid as u32),
            });
            queue.push_back((x, y, pid as u32));
        }
    }

    // ШАГ 2: Flood Fill с агрегацией данных
    while let Some((x, y, pid)) = queue.pop_front() {
        let province = &mut provinces[pid as usize];
        let idx = y * width + x;

        // Агрегация данных
        province.area += 1;
        let biome_name = format!("{:?}", biome_map.data[idx]);
        *province.biomes.entry(biome_name).or_insert(0.0) += 1.0;
        province.center.0 += x as f32;
        province.center.1 += y as f32;

        // Проверка прибрежности (только для суши)
        if province.is_land {
            for &(dx, dy) in &DIRECTIONS {
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, (height - 1) as i32) as usize;
                let nidx = ny * width + nx;
                if water_type[nidx] != WaterType::Land {
                    province.coastal = true;
                    break;
                }
            }
        }

        // Добавление соседей (только того же типа поверхности)
        for &(dx, dy) in &DIRECTIONS {
            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, (height - 1) as i32) as usize;
            let nidx = ny * width + nx;

            if province_id_map[nidx].is_none() {
                let neighbor_is_land = water_type[nidx] == WaterType::Land;
                if province.is_land == neighbor_is_land {
                    province_id_map[nidx] = Some(pid);
                    queue.push_back((nx, ny, pid));
                }
            }
        }
    }

    // ШАГ 3: Финализация — нормализация данных
    for province in &mut provinces {
        if province.area > 0 {
            // Нормализация центра масс
            province.center.0 /= province.area as f32;
            province.center.1 /= province.area as f32;

            // Нормализация биомов
            for count in province.biomes.values_mut() {
                *count /= province.area as f32;
            }

            // Определение типа провинции
            province.province_type = if !province.is_land {
                ProvinceType::Oceanic
            } else if province.coastal && province.area < 500 {
                ProvinceType::Island
            } else {
                ProvinceType::Continental
            };
        }
    }

    // ШАГ 4: Заполнение оставшихся пикселей
    let uncovered = province_id_map.iter().filter(|o| o.is_none()).count();
    if uncovered > 0 {
        println!("🔍 Заполнение {uncovered} непокрытых пикселей...");

        // Собираем центры всех провинций для расчёта расстояний
        let centers: Vec<(f32, f32)> = provinces.iter().map(|p| p.center).collect();

        for y in 0..height {
            for x in 0..width {
                let idx = y * width + x;
                if province_id_map[idx].is_none() {
                    // Находим ближайшую провинцию по расстоянию до центра
                    let mut min_d2 = f32::MAX;
                    let mut best_pid = 0;
                    for (pid, &(cx, cy)) in centers.iter().enumerate() {
                        let d2 = (x as f32 - cx).powi(2) + (y as f32 - cy).powi(2);
                        if d2 < min_d2 {
                            min_d2 = d2;
                            best_pid = pid as u32;
                        }
                    }

                    // Назначаем пиксель провинции
                    province_id_map[idx] = Some(best_pid);

                    // Обновляем данные провинции для консистентности
                    let province = &mut provinces[best_pid as usize];
                    province.area += 1;

                    // Обновляем биом (важно для точности состава)
                    let biome_name = format!("{:?}", biome_map.data[idx]);
                    *province.biomes.entry(biome_name).or_insert(0.0) += 1.0;

                    // Обновляем центр масс
                    province.center.0 += x as f32;
                    province.center.1 += y as f32;
                }
            }
        }

        // Повторная нормализация после заполнения "дыр"
        for province in &mut provinces {
            if province.area > 0 {
                province.center.0 /= province.area as f32;
                province.center.1 /= province.area as f32;

                for count in province.biomes.values_mut() {
                    *count /= province.area as f32;
                }
            }
        }
    }

    // Преобразуем карту в вектор u32
    let pixel_to_id: Vec<u32> = province_id_map
        .into_iter()
        .map(|opt| opt.unwrap()) // Все пиксели покрыты после Шага 4
        .collect();

    (provinces, pixel_to_id)
}
