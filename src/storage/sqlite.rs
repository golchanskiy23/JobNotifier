use sqlx::{sqlite::SqlitePool, Sqlite, Pool};
use chrono::Utc;
use serde_json;
use crate::domain::Job;
use crate::errors::StorageError;
use crate::storage::{Storage, JobStats};
use async_trait::async_trait;

/// SQLite хранилище вакансий
pub struct SqliteStorage {
    pool: Pool<Sqlite>,
}

impl SqliteStorage {
    /// Создает новое SQLite хранилище
    pub async fn new(database_url: &str) -> Result<Self, StorageError> {
        // Создаем директорию для базы данных если она не существует
        if database_url.starts_with("sqlite:") {
            let db_path = database_url.strip_prefix("sqlite:").unwrap_or(database_url);
            if let Some(parent) = std::path::Path::new(db_path).parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| StorageError::Connection(format!("Failed to create database directory: {}", e)))?;
            }
        }
        
        let pool = SqlitePool::connect(database_url)
            .await
            .map_err(|e| StorageError::Connection(format!("Failed to connect to database: {}", e)))?;
        
        // Создаем таблицы если их нет
        self::migrations::run_migrations(&pool).await?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    
    /// Проверяет, была ли вакансия уже видена
    async fn is_job_seen(&self, job: &Job) -> Result<bool, StorageError> {
        let dedup_key = job.dedup_key();
        
        let result: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM seen_jobs WHERE dedup_key = ?"
        )
        .bind(&dedup_key)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::Query(format!("Failed to check if job is seen: {}", e)))?;
        
        Ok(result > 0)
    }
    
    /// Отмечает вакансию как виденную
    async fn mark_job_seen(&self, job: &Job) -> Result<(), StorageError> {
        let dedup_key = job.dedup_key();
        let job_json = serde_json::to_string(job)
            .map_err(|e| StorageError::Serialization(format!("Failed to serialize job: {}", e)))?;
        
        sqlx::query(
            "INSERT OR IGNORE INTO seen_jobs (dedup_key, job_data, seen_at) VALUES (?, ?, ?)"
        )
        .bind(&dedup_key)
        .bind(&job_json)
        .bind(job.seen_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Insert(format!("Failed to mark job as seen: {}", e)))?;
        
        Ok(())
    }
    
    /// Получает все виденные вакансии
    async fn get_seen_jobs(&self, limit: Option<i64>) -> Result<Vec<Job>, StorageError> {
        let rows = if let Some(limit) = limit {
            sqlx::query_as::<_, (String,)>(
                "SELECT job_data FROM seen_jobs ORDER BY seen_at DESC LIMIT ?"
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Query(format!("Failed to get seen jobs: {}", e)))?
        } else {
            sqlx::query_as::<_, (String,)>(
                "SELECT job_data FROM seen_jobs ORDER BY seen_at DESC"
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Query(format!("Failed to get seen jobs: {}", e)))?
        };
        
        let mut jobs = Vec::new();
        for (job_json,) in rows {
            let job: Job = serde_json::from_str(&job_json)
                .map_err(|e| StorageError::Deserialization(format!("Failed to deserialize job: {}", e)))?;
            jobs.push(job);
        }
        
        Ok(jobs)
    }
    
    /// Очищает старые записи (старше указанного количества дней)
    async fn cleanup_old_jobs(&self, days_old: i64) -> Result<u64, StorageError> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days_old);
        
        let result = sqlx::query(
            "DELETE FROM seen_jobs WHERE seen_at < ?"
        )
        .bind(cutoff_date)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Delete(format!("Failed to cleanup old jobs: {}", e)))?;
        
        Ok(result.rows_affected())
    }
    
    /// Получает статистику по вакансиям
    async fn get_stats(&self) -> Result<JobStats, StorageError> {
        let total_jobs = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM seen_jobs"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::Query(format!("Failed to get stats: {}", e)))?;
        
        let last_24h = Utc::now() - chrono::Duration::hours(24);
        let recent_jobs = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM seen_jobs WHERE seen_at > ?"
        )
        .bind(last_24h)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StorageError::Query(format!("Failed to get stats: {}", e)))?;
        
        Ok(JobStats {
            total_seen: total_jobs as u64,
            last_24h: recent_jobs as u64,
        })
    }
}

/// Миграции базы данных
mod migrations {
    use sqlx::{Pool, Sqlite};
    
    pub async fn run_migrations(pool: &Pool<Sqlite>) -> Result<(), crate::errors::StorageError> {
        // Создаем таблицу для отслеживания виденных вакансий
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS seen_jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                dedup_key TEXT UNIQUE NOT NULL,
                job_data TEXT NOT NULL,
                seen_at DATETIME NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(pool)
        .await
        .map_err(|e| crate::errors::StorageError::Migration(format!("Failed to create table: {}", e)))?;
        
        // Создаем индекс для быстрого поиска
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_seen_jobs_dedup_key ON seen_jobs(dedup_key)"
        )
        .execute(pool)
        .await
        .map_err(|e| crate::errors::StorageError::Migration(format!("Failed to create index: {}", e)))?;
        
        // Создаем индекс для сортировки по времени
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_seen_jobs_seen_at ON seen_jobs(seen_at DESC)"
        )
        .execute(pool)
        .await
        .map_err(|e| crate::errors::StorageError::Migration(format!("Failed to create index: {}", e)))?;
        
        Ok(())
    }
}
