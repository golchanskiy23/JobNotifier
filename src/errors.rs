// Единая точка для доменных ошибок (library-уровень).
//
// Идея из индустрии:
//  - доменные компоненты (scraper / notifier / storage) возвращают типизированные ошибки (`thiserror`);
//  - верхний уровень приложения (main/scheduler/CLI) возвращает `anyhow::Result`,
//    добавляя контекст через `.context(...)`.

use thiserror::Error;

/// Ошибки скрейпинга (сетевые + парсинг + rate limiting).
///
/// Пока проект на стадии прототипа, все варианты ещё не используются,
/// поэтому помечаем enum как заглушку, чтобы не засорять вывод варнингами.
#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum ScraperError {
    /// Сетевая ошибка при запросе страницы.
    ///
    /// `#[source]` сохраняет исходную ошибку, чтобы в стеке причин было:
    /// "failed to scrape ..." -> "network error for ..." -> "connection refused".
    #[error("network error for {url}: {source}")]
    Network {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    /// Ошибка парсинга HTML (например, селектор не нашёл обязательный элемент).
    #[error("parse failed: {0}")]
    Parse(String),

    /// Сайт ограничил нас по частоте запросов.
    #[error("rate limited by {site}, retry after {secs}s")]
    RateLimit { site: String, secs: u64 },
}

/// Ошибки отправки уведомлений.
#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum NotifierError {
    #[error("failed to send notification: {0}")]
    Send(String),
}

/// Ошибки хранилища.
#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("failed to save jobs: {0}")]
    Save(String),
    #[error("failed to load jobs: {0}")]
    Load(String),
}


