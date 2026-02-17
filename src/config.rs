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
use std::fs;
use std::path::Path;

/// Раздел конфигурации, отвечающий за scraping.
#[derive(Debug, Deserialize)]
pub struct ScrapingConfig {
    /// Список произвольных URL для обхода (hh, lamoda, avito и т.д.).
    pub urls: Vec<String>,
}

/// Корневая структура всего конфига.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub scraping: ScrapingConfig,
}

/// Ошибки при загрузке конфигурации.
#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    ParseToml(toml::de::Error),
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        ConfigError::Io(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError::ParseToml(err)
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "I/O error: {}", e),
            ConfigError::ParseToml(e) => write!(f, "TOML parse error: {}", e),
        }
    }
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
}


