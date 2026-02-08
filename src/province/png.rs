// src/province/png.rs
//! Визуализация провинций в изображение
//!
//! Этот модуль преобразует данные провинций в визуальное представление:
//! - Карту пикселей → `province_id` в цветное изображение
//! - Цвета провинций берутся из их уникальных цветов (`Province::color`)
//! - Поддержка сохранения в PNG для отладки и экспорта
//!
//! ## Архитектура
//!
//! 1. **`ProvinceMap`** — структура данных, хранящая карту пикселей:
//!    - Каждый пиксель содержит `province_id` (идентификатор провинции)
//!    - Не хранит цвета напрямую — цвета берутся из внешнего списка `Province`
//!
//! 2. **Конвертация в изображение**:
//!    - Строится маппинг `province_id → RGBA`
//!    - Каждый пиксель преобразуется в 4 байта (R, G, B, A)
//!    - Неизвестные ID получают чёрный цвет (`#000000`) для обнаружения ошибок
//!
//! 3. **Сохранение в PNG**:
//!    - Используется библиотека `image` для создания и записи изображения
//!    - Поддержка альфа-канала (всегда 255 = непрозрачный)
//!
//! ## Особенности реализации
//!
//! - **Разделение данных и визуализации**: карта хранит только `province_id`, цвета — в `Province`
//!   - Позволяет менять цвета без перестроения карты
//!   - Упрощает сериализацию (цвета не дублируются)
//! - **Безопасность**: защита от некорректных `province_id` через цвет по умолчанию
//! - **Эффективность**: однократное построение маппинга цветов перед рендерингом
//! - **Согласованность**: цвета провинций совпадают с `Province::color` для детерминированности
//!
//! ## Пример использования
//! ```rust
//! // Генерация провинций
//! let (provinces, pixel_to_id) = generate_provinces_from_seeds(...);
//!
//! // Создание карты провинций
//! let province_map = ProvinceMap::from_pixel_map(
//!     width,
//!     height,
//!     &pixel_to_id,
//! );
//!
//! // Сохранение в PNG
//! province_map.save_as_png(&provinces, "output/provinces.png")?;
//!
//! // Получение цвета провинции по ID
//! let color = province_map.get_province_color(&provinces, 42);
//! assert_eq!(color, "#a1b2c3");
//! ```

use crate::province::Province;
use image::{ImageBuffer, Rgba};
use std::collections::HashMap;

/// Карта провинций — пространственное распределение провинций по карте
///
/// Содержит сырые данные о принадлежности каждого пикселя к провинции.
/// Цвета провинций не хранятся напрямую — они берутся из внешнего списка `Province`.
#[derive(Debug, Clone)]
pub struct ProvinceMap {
    /// Ширина карты в пикселях
    pub width: u32,

    /// Высота карты в пикселях
    pub height: u32,

    /// Данные карты: вектор `province_id` размером `width × height`
    ///
    /// Каждый элемент — идентификатор провинции (`u32`), которому принадлежит пиксель.
    /// Индекс вычисляется как `y * width + x`.
    ///
    /// # Гарантии
    /// - Все пиксели имеют корректный `province_id` (после заполнения "дыр" в генераторе)
    /// - `province_id` соответствует одной из провинций в списке `provinces`
    pub data: Vec<u32>, // province_id
}

impl ProvinceMap {
    /// Создаёт карту провинций из готовой карты пикселей → `province_id`
    ///
    /// # Параметры
    /// * `width` — ширина карты в пикселях
    /// * `height` — высота карты в пикселях
    /// * `pixel_to_id` — срез длиной `width × height`, где каждый элемент — `province_id`
    ///
    /// # Возвращает
    /// Новую структуру `ProvinceMap` с копией данных из `pixel_to_id`
    ///
    /// # Пример
    /// ```rust
    /// let pixel_to_id = vec![0, 0, 1, 1, 0, 1, 2, 2]; // 2×4 карта
    /// let map = ProvinceMap::from_pixel_map(2, 4, &pixel_to_id);
    /// assert_eq!(map.width, 2);
    /// assert_eq!(map.height, 4);
    /// assert_eq!(map.data, pixel_to_id);
    /// ```
    #[must_use]
    pub fn from_pixel_map(width: u32, height: u32, pixel_to_id: &[u32]) -> Self {
        Self {
            width,
            height,
            data: pixel_to_id.to_vec(),
        }
    }

    /// Возвращает цвет провинции в формате HEX по её идентификатору
    ///
    /// # Параметры
    /// * `provinces` — список всех провинций для поиска
    /// * `province_id` — идентификатор искомой провинции
    ///
    /// # Возвращает
    /// * Цвет в формате `"#rrggbb"` (например, `"#a1b2c3"`), если провинция найдена
    /// * `"#000000"` (чёрный), если провинция не найдена (защита от ошибок)
    ///
    /// # Пример
    /// ```rust
    /// let color = map.get_province_color(&provinces, 42);
    /// assert!(color.starts_with('#'));
    /// assert_eq!(color.len(), 7);
    /// ```
    #[must_use]
    pub fn get_province_color(&self, provinces: &[Province], province_id: u32) -> String {
        if let Some(province) = provinces.iter().find(|p| p.id == province_id) {
            province.color.clone()
        } else {
            "#000000".to_string() // Чёрный цвет для неизвестных провинций (визуальный сигнал ошибки)
        }
    }

    /// Преобразует карту провинций в RGBA-изображение для визуализации
    ///
    /// # Алгоритм
    /// 1. Строит маппинг `province_id → [R, G, B, A]` на основе цветов из `provinces`:
    ///    - Извлекает компоненты из HEX-строки (`"#rrggbb"` → `[r, g, b, 255]`)
    ///    - Игнорирует некорректные цвета (но в валидных данных их не должно быть)
    /// 2. Для каждого пикселя в `data`:
    ///    - Ищет цвет в маппинге по `province_id`
    ///    - Если не найден — использует чёрный цвет (`[0, 0, 0, 255]`) для обнаружения ошибок
    /// 3. Формирует плоский вектор байт в порядке `[R, G, B, A, R, G, B, A, ...]`
    ///
    /// # Параметры
    /// * `provinces` — список провинций для получения цветов
    ///
    /// # Возвращает
    /// Вектор байт длиной `width × height × 4`, готовый для создания изображения.
    ///
    /// # Особенности
    /// - **Эффективность**: маппинг строится один раз перед обработкой всех пикселей
    /// - **Безопасность**: чёрный цвет для неизвестных ID помогает обнаружить ошибки генерации
    /// - **Согласованность**: цвета совпадают с `Province::color` для детерминированности
    ///
    /// # Пример
    /// ```rust
    /// let rgba = map.to_rgba_image(&provinces);
    /// assert_eq!(rgba.len(), (map.width * map.height * 4) as usize);
    /// ```
    #[must_use]
    pub fn to_rgba_image(&self, provinces: &[Province]) -> Vec<u8> {
        // Создаём маппинг ID → цвет для эффективного поиска
        let mut color_map: HashMap<u32, [u8; 4]> = HashMap::new();

        // Добавляем цвета для всех провинций
        for province in provinces {
            // Извлекаем компоненты из HEX-строки "#rrggbb"
            let hex = &province.color[1..]; // убираем '#'
            if hex.len() == 6
                && let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                )
            {
                color_map.insert(province.id, [r, g, b, 255]); // альфа = 255 (непрозрачный)
            }
        }

        // Цвет по умолчанию для неотнесённых пикселей (чёрный — визуальный сигнал ошибки)
        let default_color = [0, 0, 0, 255];

        // Преобразуем каждый пиксель в RGBA
        self.data
            .iter()
            .flat_map(|&pid| color_map.get(&pid).copied().unwrap_or(default_color))
            .collect()
    }

    /// Сохраняет карту провинций в PNG-файл
    ///
    /// # Параметры
    /// * `provinces` — список провинций для получения цветов
    /// * `path` — путь к файлу для сохранения (например, `"output/provinces.png"`)
    ///
    /// # Ошибки
    /// Возвращает ошибку в случае:
    /// - Невозможно создать буфер изображения (некорректные размеры)
    /// - Невозможно записать файл (нет прав, недостаточно места и т.д.)
    ///
    /// # Пример
    /// ```rust
    /// province_map.save_as_png(&provinces, "output/provinces.png")?;
    /// ```
    pub fn save_as_png(
        &self,
        provinces: &[Province],
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let rgba_data = self.to_rgba_image(provinces);
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, rgba_data)
                .ok_or("Failed to create image buffer")?;
        img.save(path)?;
        Ok(())
    }
}
