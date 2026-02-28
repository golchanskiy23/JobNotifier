// Модуль конфигурации: откуда брать список URL и какие скрейперы использовать.
//
// Для простоты сейчас конфиг задаёт только список URL, а маппинг
// "домен → конкретный Scraper" реализуется в коде (можно будет
// вынести в конфиг на следующих фазах).
//
// Формат TOML (файл `Config.toml` в корне проекта):
//
// ```toml
// [scraping]
// urls = [
//   "https://hh.ru/search/vacancy?text=rust",
//   "https://www.lamoda.ru/c/men-home/rabota/",
//   "https://www.avito.ru/moskva/vakansii/programmist",
// ]
// ```

use serde::Deserialize;
use thiserror::Error;
use std::fs;
use std::path::Path;

/// Раздел конфигурации, отвечающий за scraping.
#[derive(Debug, Deserialize)]
pub struct ScrapingConfig {
    /// Список произвольных URL для обхода (hh, lamoda, avito и т.д.).
    pub urls: Vec<String>,
    /// Интервал между запусками scraping в минутах.
    pub interval_minutes: u64,
    /// Таймаут HTTP-запроса/парсинга в секундах.
    pub timeout_secs: u64,
}

/// Корневая структура всего конфига.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub scraping: ScrapingConfig,
}

/// Ошибки при загрузке и валидации конфигурации.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    ParseToml(#[from] toml::de::Error),
    #[error("missing config field: {0}")]
    MissingField(&'static str),
    #[error("invalid config value: {0}")]
    InvalidValue(&'static str),
}

impl AppConfig {
    /// Загружает конфиг из TOML-файла.
    ///
    /// Почему `&str`, а не `String`:
    ///  - путь к файлу у нас уже есть (например, литерал `"Config.toml"`),
    ///  - функция лишь "читает" этот путь, не владея им.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        // Читаем весь файл в память. Конфиг небольшой, это дешёвая операция.
        let contents = fs::read_to_string(path)?;
        // Парсим TOML в строго типизированную структуру.
        let cfg: AppConfig = toml::from_str(&contents)?;
        Ok(cfg)
    }

    /// Бизнес-валидация конфига.
    ///
    /// Здесь мы проверяем семантику значений (пустые списки, нулевые интервалы и т.п.),
    /// а не синтаксис TOML (это делает парсер).
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.scraping.urls.is_empty() {
            return Err(ConfigError::MissingField("scraping.urls"));
        }

        if self.scraping.interval_minutes == 0 {
            return Err(ConfigError::InvalidValue(
                "scraping.interval_minutes must be > 0",
            ));
        }

        if self.scraping.timeout_secs == 0 {
            return Err(ConfigError::InvalidValue(
                "scraping.timeout_secs must be > 0",
            ));
        }

        Ok(())
    }
}


