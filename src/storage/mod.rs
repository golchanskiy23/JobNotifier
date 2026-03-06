use crate::domain::Job;
use crate::errors::StorageError;
use async_trait::async_trait;

/// Trait для всех хранилищ вакансий
#[async_trait]
pub trait Storage: Send + Sync {
    /// Проверяет, была ли вакансия уже видена
    async fn is_job_seen(&self, job: &Job) -> Result<bool, StorageError>;
    
    /// Отмечает вакансию как виденную
    async fn mark_job_seen(&self, job: &Job) -> Result<(), StorageError>;
    
    /// Получает все виденные вакансии
    async fn get_seen_jobs(&self, limit: Option<i64>) -> Result<Vec<Job>, StorageError>;
    
    /// Очищает старые записи (старше указанного количества дней)
    async fn cleanup_old_jobs(&self, days_old: i64) -> Result<u64, StorageError>;
    
    /// Получает статистику по вакансиям
    async fn get_stats(&self) -> Result<JobStats, StorageError>;
}

/// Статистика по вакансиям
#[derive(Debug)]
pub struct JobStats {
    pub total_seen: u64,
    pub last_24h: u64,
}

pub mod sqlite;

pub use sqlite::SqliteStorage;
