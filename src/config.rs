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
}

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub scraping: ScrapingConfig,
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
