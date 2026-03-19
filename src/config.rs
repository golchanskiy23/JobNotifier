use serde::Deserialize;
use thiserror::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct ScrapingConfig {
    pub urls: Vec<String>,
    pub interval_minutes: u64,
    pub timeout_secs: u64,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    /// Использовать headless Chromium для всех URL (устаревший флаг, используй browser_urls)
    #[serde(default)]
    pub use_browser: bool,
    /// Список URL которые нужно скрейпить через headless Chromium (SPA-сайты)
    /// Если пустой и use_browser=true — браузер используется для всех URL
    #[serde(default)]
    pub browser_urls: Vec<String>,
    /// Путь к исполняемому файлу Chromium (None = автопоиск)
    #[serde(default)]
    pub chrome_path: Option<String>,
    /// Время ожидания JS-рендеринга в миллисекундах (по умолчанию 3000)
    #[serde(default)]
    pub browser_wait_ms: Option<u64>,
}

impl ScrapingConfig {
    /// Нужно ли использовать браузер для данного URL
    pub fn needs_browser(&self, url: &str) -> bool {
        if !self.browser_urls.is_empty() {
            // Точное совпадение или URL начинается с одного из browser_urls
            self.browser_urls.iter().any(|b| url == b || url.starts_with(b.trim_end_matches('/')))
        } else {
            self.use_browser
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub scraping: ScrapingConfig,
    /// Маппинг домен → название компании (например "careers.kaspersky.ru" = "Kaspersky")
    #[serde(default)]
    pub companies: std::collections::HashMap<String, String>,
}

impl AppConfig {
    /// Возвращает название компании по URL вакансии
    pub fn company_for_url(&self, url: &str) -> String {
        let host = if let Some(pos) = url.find("://") {
            let after = &url[pos + 3..];
            let end = after.find('/').unwrap_or(after.len());
            after[..end].to_string()
        } else {
            return String::new();
        };
        // Точное совпадение
        if let Some(name) = self.companies.get(&host) {
            return name.clone();
        }
        // Совпадение по суффиксу домена (например "kaspersky.ru" матчит "careers.kaspersky.ru")
        for (domain, name) in &self.companies {
            if host.ends_with(domain) {
                return name.clone();
            }
        }
        // Fallback: второй уровень домена (kaspersky из careers.kaspersky.ru)
        let parts: Vec<&str> = host.split('.').collect();
        if parts.len() >= 2 {
            parts[parts.len() - 2].to_string()
        } else {
            host
        }
    }
}

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
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path)?;
        let cfg: AppConfig = toml::from_str(&contents)?;
        Ok(cfg)
    }

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



#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // 10.2 Unit-тест: AppConfig без поля keywords → keywords == []
    #[test]
    fn test_config_without_keywords_defaults_to_empty() {
        let toml_str = r#"
[scraping]
urls = ["https://example.com"]
interval_minutes = 60
timeout_secs = 10
"#;
        let cfg: AppConfig = toml::from_str(toml_str).expect("should parse");
        assert!(cfg.scraping.keywords.is_empty());
    }

    // 10.10 Property P5: десериализация keywords из TOML
    // Feature: job-notifier-enhanced, Property 5: AppConfig десериализует keywords без потерь
    proptest! {
        #[test]
        fn prop_p5_keywords_deserialization(
            keywords in prop::collection::vec("[a-zA-Z]{1,10}", 0..10),
        ) {
            let kw_toml = keywords.iter()
                .map(|k| format!("\"{}\"", k))
                .collect::<Vec<_>>()
                .join(", ");

            let toml_str = format!(
                r#"
[scraping]
urls = ["https://example.com"]
interval_minutes = 60
timeout_secs = 10
keywords = [{}]
"#,
                kw_toml
            );

            let cfg: AppConfig = toml::from_str(&toml_str).expect("should parse");
            prop_assert_eq!(cfg.scraping.keywords, keywords);
        }
    }
}
