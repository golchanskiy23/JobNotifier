use async_trait::async_trait;
use crate::domain::Job;
use crate::errors::ScraperError;

/// Trait для всех скрейперов вакансий
#[async_trait]
pub trait Scraper: Send + Sync {
    /// Асинхронный сбор вакансий с указанного URL
    async fn scrape(&self, url: &str) -> Result<Vec<Job>, ScraperError>;
    
    /// Название скрейпера для логов
    fn name(&self) -> &str;
}

pub mod hh;

pub use hh::HhScraper;
