use thiserror::Error;

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum ScraperError {
    #[error("network error for {url}: {source}")]
    Network {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("parse failed: {0}")]
    Parse(String),

    #[error("rate limited by {site}, retry after {secs}s")]
    RateLimit { site: String, secs: u64 },
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum NotifierError {
    #[error("failed to send notification: {0}")]
    Send(String),
}

#[allow(dead_code)]
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("connection failed: {0}")]
    Connection(String),
    
    #[error("migration failed: {0}")]
    Migration(String),
    
    #[error("query failed: {0}")]
    Query(String),
    
    #[error("insert failed: {0}")]
    Insert(String),
    
    #[error("delete failed: {0}")]
    Delete(String),
    
    #[error("serialization failed: {0}")]
    Serialization(String),
    
    #[error("deserialization failed: {0}")]
    Deserialization(String),
    
    #[error("failed to save jobs: {0}")]
    Save(String),
    
    #[error("failed to load jobs: {0}")]
    Load(String),
}


