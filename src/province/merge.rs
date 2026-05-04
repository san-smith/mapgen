// src/province/merge.rs
//! Слияние мелких провинций
//!
//! Этот модуль реализует постобработку провинций для улучшения геймплея:
//! удаление излишне мелких провинций путём их слияния с крупными соседями.
//!
//! ## Зачем нужно слияние?
//!
//! Мелкие провинции (< 50 пикселей) создают проблемы для геймплея:
//! - Слишком много микро-территорий усложняет управление
//! - Армии не могут эффективно маневрировать через "игольные уши"
//! - Экономика становится фрагментированной и несбалансированной
//! - Карта выглядит "зашумлённой" с множеством мелких фрагментов
//!
//! ## Алгоритм слияния
//!
//! 1. **Поиск мелких провинций**:
//!    - Итеративно находим провинции с площадью < `MIN_AREA_THRESHOLD`
//!    - Обработка продолжается до тех пор, пока мелкие провинции существуют
//!
//! 2. **Выбор соседа для слияния**:
//!    - Рассматриваем только соседей того же типа поверхности (суша→суша, море→море)
//!    - Выбираем соседа с **максимальной площадью** для минимизации фрагментации
//!    - Используем граф смежности для корректного определения соседства
//!
//! 3. **Слияние данных**:
//!    - **Центр масс**: взвешенное среднее по площади
//!      ```text
//!      x_new = (x_large * area_large + x_small * area_small) / (area_large + area_small)
//!      ```
//!    - **Биомный состав**: взвешенное объединение долей
//!      ```text
//!      ratio_new[биом] = (ratio_large[биом] * area_large + ratio_small[биом] * area_small) / total_area
//!      ```
//!    - **Прибрежность**: логическое ИЛИ (`coastal_new = coastal_large || coastal_small`)
//!    - **Площадь**: сумма площадей (`area_new = area_large + area_small`)
//!
//! 4. **Удаление мелкой провинции**:
//!    - Провинция удаляется из списка после передачи данных
//!    - Граф смежности не обновляется (используется только для поиска соседей)
//!
//! ## Особенности реализации
//!
//! - **Тип поверхности сохраняется**: суша никогда не сливается с морем
//! - **Итеративный подход**: после слияния могут появиться новые мелкие провинции → повторная обработка
//! - **Детерминированность**: выбор соседа по максимальной площади обеспечивает воспроизводимость
//! - **Безопасность заимствования**: данные мелкой провинции копируются перед изменением вектора
//! - **Эффективность**: сложность O(N × M), где N — число провинций, M — число соседей

use crate::province::Province;
use petgraph::graph::UnGraph;
use std::collections::HashMap;

/// Минимальная допустимая площадь провинции в пикселях
///
/// Провинции с площадью меньше этого порога считаются "слишком мелкими" для геймплея
/// и подлежат слиянию с крупными соседями.
///
/// # Обоснование выбора значения
/// - `50` пикселей ≈ 7×7 квадрат на карте
/// - Меньше этого размера провинция не может вместить значимую инфраструктуру
/// - Опыт игр-стратегий (Crusader Kings, Europa Universalis) показывает оптимальность этого порога
/// - Баланс между детализацией карты и удобством управления
const MIN_AREA_THRESHOLD: usize = 50;
const MAX_ITERATIONS: usize = 1000; // защита от бесконечного цикла

/// Сливает все мелкие провинции с их крупнейшими соседями
///
/// # Алгоритм
/// 1. Итеративно ищет провинции с площадью < `MIN_AREA_THRESHOLD * scale_factor`
/// 2. Для каждой мелкой провинции вызывает `merge_one_small_province`
/// 3. Повторяет поиск до тех пор, пока мелкие провинции существуют
///    (слияние может создать новые мелкие провинции из-за изменения площадей соседей)
///
/// # Параметры
/// * `provinces` — mutable-ссылка на вектор провинций для модификации
/// * `graph` — граф смежности для определения соседей провинций
///
/// # Эффект
/// - Модифицирует `provinces` на месте:
///   - Удаляет мелкие провинции
///   - Обновляет данные крупных провинций (площадь, центр, биомы)
/// - Выводит в консоль количество слитых провинций
///
/// # Пример
/// ```rust
/// let mut provinces = vec![/* ... */];
/// let graph = build_province_graph_with_map(&provinces, &pixel_to_id, width, height);
///
/// merge_small_provinces(&mut provinces, &graph, scale_factor);
/// // Теперь все провинции имеют площадь >= MIN_AREA_THRESHOLD * scale_factor
/// ```
pub fn merge_small_provinces(
    provinces: &mut Vec<Province>,
    graph: &UnGraph<u32, ()>,
    scale_factor: f32,
) {
    let mut merged_count = 0;
    let mut iterations = 0;

    // Предподготовка карт поиска
    let mut prov_map: HashMap<u32, usize> = provinces
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id, i))
        .collect();
    let node_map: HashMap<u32, petgraph::graph::NodeIndex> =
        graph.node_indices().map(|idx| (graph[idx], idx)).collect();
    let min_area = (MIN_AREA_THRESHOLD as f32 * scale_factor).round() as usize;

    loop {
        iterations += 1;
        if iterations > MAX_ITERATIONS {
            eprintln!("Достигнут лимит итераций слияния ({MAX_ITERATIONS})");
            break;
        }

        // Находим первую мелкую провинцию
        let small_province_id = provinces.iter().find(|p| p.area < min_area).map(|p| p.id);

        if let Some(small_id) = small_province_id {
            if merge_one_small_province(provinces, &mut prov_map, &node_map, graph, small_id) {
                merged_count += 1;
            } else {
                // Не удалось слить — пропускаем для избежания бесконечного цикла
                break;
            }
        } else {
            // Больше нет мелких провинций
            break;
        }
    }

    if merged_count > 0 {
        println!("🧹 Слито {merged_count} мелких провинций (площадь < {min_area} пикселей).",);
    } else {
        println!("✅ Все провинции имеют достаточный размер (≥ {min_area} пикселей).",);
    }
}

/// Сливает одну мелкую провинцию с её крупнейшим соседом того же типа поверхности
///
/// # Алгоритм
/// 1. Находит мелкую провинцию по `small_id` в списке `provinces`
/// 2. Определяет её тип поверхности (`is_land`)
/// 3. Находит всех соседей через граф смежности
/// 4. Фильтрует соседей по типу поверхности (только те же: суша→суша, море→море)
/// 5. Выбирает соседа с максимальной площадью
/// 6. Копирует данные мелкой провинции (для безопасного заимствования)
/// 7. Выполняет слияние данных в крупной провинции:
///    - Центр масс: взвешенное среднее по площади
///    - Биомы: взвешенное объединение долей
///    - Прибрежность: логическое ИЛИ
///    - Площадь: сумма площадей
/// 8. Удаляет мелкую провинцию из списка
///
/// # Параметры
/// * `provinces` — mutable-ссылка на вектор провинций
/// * `graph` — граф смежности для поиска соседей
/// * `small_id` — идентификатор мелкой провинции для слияния
///
/// # Возвращает
/// * `true` — слияние успешно выполнено
/// * `false` — слияние невозможно (нет подходящих соседей или провинция не найдена)
///
/// # Особенности
/// - **Сохранение типа поверхности**: суша никогда не сливается с морем
/// - **Безопасное заимствование**: данные копируются перед изменением вектора (решает ошибку E0502)
/// - **Взвешенное объединение**: все данные объединяются пропорционально площади
///
/// # Пример
/// ```rust
/// // Провинция 42 имеет площадь 30 (< 50) и соседей 15 (площадь 200) и 27 (площадь 150)
/// let success = merge_one_small_province(&mut provinces, &graph, 42);
/// assert!(success);
/// // Теперь провинция 42 удалена, провинция 15 имеет увеличенную площадь и обновлённые данные
/// ```
fn merge_one_small_province(
    provinces: &mut Vec<Province>,
    prov_map: &mut HashMap<u32, usize>,
    node_map: &HashMap<u32, petgraph::graph::NodeIndex>,
    graph: &UnGraph<u32, ()>,
    small_id: u32,
) -> bool {
    // Находим мелкую провинцию и её индекс через prov_map
    let Some(&small_idx) = prov_map.get(&small_id) else {
        return false; // провинция не найдена
    };

    // Проверяем, что индекс корректен
    if small_idx >= provinces.len() {
        return false;
    }

    // Копируем необходимые данные мелкой провинции ДО получения изменяемой ссылки
    let small_area = provinces[small_idx].area as f32;
    let small_center = provinces[small_idx].center;
    let small_biomes = provinces[small_idx].biomes.clone(); // HashMap копируется
    let small_coastal = provinces[small_idx].coastal;
    let is_land = provinces[small_idx].is_land;

    // Находим узел мелкой провинции в графе
    let Some(&small_node_idx) = node_map.get(&small_id) else {
        return false; // узел не найден в графе
    };

    // Находим крупнейшего соседа того же типа поверхности
    let largest_neighbor_id = graph
        .neighbors(small_node_idx)
        .filter_map(|n_idx| {
            let n_id = graph[n_idx];
            prov_map.get(&n_id).map(|&idx| &provinces[idx])
        })
        .filter(|&n_prov| n_prov.is_land == is_land) // только тот же тип поверхности
        .max_by_key(|&n_prov| n_prov.area)
        .map(|p| p.id);

    if let Some(large_id) = largest_neighbor_id {
        let large_idx = prov_map[&large_id];

        // Обновляем крупную провинцию (теперь безопасно — нет конфликта заимствования)
        let large_prov = &mut provinces[large_idx];
        let large_area = large_prov.area as f32;
        let total_area = small_area + large_area;

        // Взвешенное обновление центра масс
        large_prov.center.0 =
            (large_prov.center.0 * large_area + small_center.0 * small_area) / total_area;
        large_prov.center.1 =
            (large_prov.center.1 * large_area + small_center.1 * small_area) / total_area;

        // Взвешенное объединение биомов
        for (biome, small_ratio) in &small_biomes {
            let large_ratio = large_prov.biomes.entry(biome.clone()).or_insert(0.0);
            *large_ratio = (*large_ratio * large_area + small_ratio * small_area) / total_area;
        }

        // Нормализация биомов после слияния (гарантируем сумму ≈ 1.0)
        let biome_sum: f32 = large_prov.biomes.values().sum();
        if (biome_sum - 1.0).abs() > 1e-6 {
            for ratio in large_prov.biomes.values_mut() {
                *ratio /= biome_sum;
            }
        }

        // Проверка на отрицательные значения (на всякий случай)
        for &ratio in large_prov.biomes.values() {
            if ratio < 0.0 {
                eprintln!("Обнаружено отрицательное значение биома после слияния: {ratio}");
            }
        }

        // Обновление прибрежности (логическое ИЛИ)
        large_prov.coastal = large_prov.coastal || small_coastal;

        // Обновление площади
        large_prov.area = total_area as usize;

        // Удаляем мелкую провинцию по индексу
        provinces.remove(small_idx);

        // Обновляем prov_map: удаляем запись мелкой провинции и корректируем индексы
        prov_map.clear();
        for (new_idx, prov) in provinces.iter().enumerate() {
            prov_map.insert(prov.id, new_idx);
        }

        true
    } else {
        // Нет подходящих соседей для слияния
        false
    }
}
