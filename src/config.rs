use serde::Deserialize;
use thiserror::Error;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct ScrapingConfig {
    pub urls: Vec<String>,
    pub interval_minutes: u64,
    pub timeout_secs: u64,
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


