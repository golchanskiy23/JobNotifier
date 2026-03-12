use crate::domain::Job;
use crate::errors::StorageError;
use async_trait::async_trait;

#[async_trait]
pub trait Storage: Send + Sync {
    async fn is_job_seen(&self, job: &Job) -> Result<bool, StorageError>;
    
    async fn mark_job_seen(&self, job: &Job) -> Result<(), StorageError>;
    
    async fn get_seen_jobs(&self, limit: Option<i64>) -> Result<Vec<Job>, StorageError>;
    
    async fn cleanup_old_jobs(&self, days_old: i64) -> Result<u64, StorageError>;
    
    async fn get_stats(&self) -> Result<JobStats, StorageError>;
}

#[derive(Debug)]
pub struct JobStats {
    pub total_seen: u64,
    pub last_24h: u64,
}

pub mod sqlite;

pub use sqlite::SqliteStorage;
