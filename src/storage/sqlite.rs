use sqlx::{sqlite::SqlitePool, Sqlite, Pool};
use chrono::Utc;
use serde_json;
use crate::domain::{Job, Application, ApplicationStatus};
use crate::errors::StorageError;
use crate::storage::{Storage, JobStats};
use async_trait::async_trait;

pub struct SqliteStorage {
    pool: Pool<Sqlite>,
}

impl SqliteStorage {
    pub async fn new(database_url: &str) -> Result<Self, StorageError> {
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
        
        self::migrations::run_migrations(&pool).await?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl Storage for SqliteStorage {
    
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

    async fn add_application(&self, app: &Application) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO applications (id, company, position, applied_at, expected_reply_days, status, notes, job_url) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&app.id)
        .bind(&app.company)
        .bind(&app.position)
        .bind(app.applied_at.to_string())
        .bind(app.expected_reply_days as i64)
        .bind(app.status.to_string())
        .bind(&app.notes)
        .bind(&app.job_url)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Insert(format!("Failed to insert application: {}", e)))?;

        Ok(())
    }

    async fn list_applications(&self) -> Result<Vec<Application>, StorageError> {
        let rows = sqlx::query_as::<_, ApplicationRow>(
            "SELECT id, company, position, applied_at, expected_reply_days, status, notes, job_url \
             FROM applications ORDER BY applied_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Query(format!("Failed to list applications: {}", e)))?;

        rows.into_iter().map(ApplicationRow::into_application).collect()
    }

    async fn update_application_status(&self, id: &str, status: &ApplicationStatus) -> Result<(), StorageError> {
        let result = sqlx::query(
            "UPDATE applications SET status = ? WHERE id = ?"
        )
        .bind(status.to_string())
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Query(format!("Failed to update application status: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("Application with id '{}' not found", id)));
        }

        Ok(())
    }

    async fn delete_application(&self, id: &str) -> Result<(), StorageError> {
        let result = sqlx::query("DELETE FROM applications WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Delete(format!("Failed to delete application: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(StorageError::NotFound(format!("Application with id '{}' not found", id)));
        }

        Ok(())
    }

    async fn get_deadline_applications(&self) -> Result<Vec<Application>, StorageError> {
        let rows = sqlx::query_as::<_, ApplicationRow>(
            "SELECT id, company, position, applied_at, expected_reply_days, status, notes, job_url \
             FROM applications \
             WHERE date(applied_at, '+' || expected_reply_days || ' days') = date('now') \
               AND status IN ('Submitted', 'InReview')"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Query(format!("Failed to get deadline applications: {}", e)))?;

        rows.into_iter().map(ApplicationRow::into_application).collect()
    }
}

#[derive(sqlx::FromRow)]
struct ApplicationRow {
    id: String,
    company: String,
    position: String,
    applied_at: String,
    expected_reply_days: i64,
    status: String,
    notes: Option<String>,
    job_url: Option<String>,
}

impl ApplicationRow {
    fn into_application(self) -> Result<Application, StorageError> {
        use chrono::NaiveDate;

        let applied_at = NaiveDate::parse_from_str(&self.applied_at, "%Y-%m-%d")
            .map_err(|e| StorageError::Deserialization(format!("Invalid applied_at date '{}': {}", self.applied_at, e)))?;

        let status = match self.status.as_str() {
            "Submitted" => ApplicationStatus::Submitted,
            "InReview" => ApplicationStatus::InReview,
            "Rejected" => ApplicationStatus::Rejected,
            "OfferReceived" => ApplicationStatus::OfferReceived,
            "Withdrawn" => ApplicationStatus::Withdrawn,
            other => return Err(StorageError::Deserialization(format!("Unknown application status: {}", other))),
        };

        Ok(Application {
            id: self.id,
            company: self.company,
            position: self.position,
            applied_at,
            expected_reply_days: self.expected_reply_days as u32,
            status,
            notes: self.notes,
            job_url: self.job_url,
        })
    }
}

mod migrations {
    use sqlx::{Pool, Sqlite};
    
    pub async fn run_migrations(pool: &Pool<Sqlite>) -> Result<(), crate::errors::StorageError> {
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
        
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_seen_jobs_dedup_key ON seen_jobs(dedup_key)"
        )
        .execute(pool)
        .await
        .map_err(|e| crate::errors::StorageError::Migration(format!("Failed to create index: {}", e)))?;
        
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_seen_jobs_seen_at ON seen_jobs(seen_at DESC)"
        )
        .execute(pool)
        .await
        .map_err(|e| crate::errors::StorageError::Migration(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS applications (
                id TEXT PRIMARY KEY,
                company TEXT NOT NULL,
                position TEXT NOT NULL,
                applied_at DATE NOT NULL,
                expected_reply_days INTEGER NOT NULL DEFAULT 21,
                status TEXT NOT NULL DEFAULT 'Submitted',
                notes TEXT,
                job_url TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#
        )
        .execute(pool)
        .await
        .map_err(|e| crate::errors::StorageError::Migration(format!("Failed to create applications table: {}", e)))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_applications_status ON applications(status)"
        )
        .execute(pool)
        .await
        .map_err(|e| crate::errors::StorageError::Migration(format!("Failed to create index: {}", e)))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_applications_applied_at ON applications(applied_at DESC)"
        )
        .execute(pool)
        .await
        .map_err(|e| crate::errors::StorageError::Migration(format!("Failed to create index: {}", e)))?;

        // Add job_url column if it doesn't exist (migration for existing databases)
        let _ = sqlx::query("ALTER TABLE applications ADD COLUMN job_url TEXT")
            .execute(pool)
            .await; // ignore error — column already exists
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::application::{Application, ApplicationStatus};
    use chrono::NaiveDate;
    use proptest::prelude::*;

    fn make_app(id: &str, company: &str, position: &str, applied_at: NaiveDate, days: u32, status: ApplicationStatus) -> Application {
        Application {
            id: id.to_string(),
            company: company.to_string(),
            position: position.to_string(),
            applied_at,
            expected_reply_days: days,
            status,
            notes: None,
            job_url: None,
        }
    }

    // 10.11 Property P6: round-trip добавления Application
    // Feature: job-notifier-enhanced, Property 6: после add_application список содержит заявку с теми же полями
    proptest! {
        #[test]
        fn prop_p6_add_application_roundtrip(
            company in "[A-Za-z]{1,15}",
            position in "[A-Za-z]{1,15}",
            days in 1u32..60u32,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();
                let today = chrono::Local::now().date_naive();
                let app = make_app("id-roundtrip", &company, &position, today, days, ApplicationStatus::Submitted);
                storage.add_application(&app).await.unwrap();
                let list = storage.list_applications().await.unwrap();
                let found = list.iter().find(|a| a.id == "id-roundtrip").expect("app must be in list");
                prop_assert_eq!(&found.company, &company);
                prop_assert_eq!(&found.position, &position);
                prop_assert_eq!(found.expected_reply_days, days);
                prop_assert_eq!(&found.status, &ApplicationStatus::Submitted);
                Ok(())
            })?;
        }
    }

    // 10.12 Property P7: сортировка списка заявок по applied_at DESC
    // Feature: job-notifier-enhanced, Property 7: list_applications возвращает заявки в порядке убывания applied_at
    proptest! {
        #[test]
        fn prop_p7_list_applications_sorted_desc(
            offsets in prop::collection::vec(0i64..365i64, 2..6),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();
                let base = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
                for (i, offset) in offsets.iter().enumerate() {
                    let date = base + chrono::Duration::days(*offset);
                    let app = make_app(
                        &format!("id-sort-{}", i),
                        "Co",
                        "Dev",
                        date,
                        21,
                        ApplicationStatus::Submitted,
                    );
                    storage.add_application(&app).await.unwrap();
                }
                let list = storage.list_applications().await.unwrap();
                for window in list.windows(2) {
                    prop_assert!(
                        window[0].applied_at >= window[1].applied_at,
                        "list not sorted: {} < {}", window[0].applied_at, window[1].applied_at
                    );
                }
                Ok(())
            })?;
        }
    }

    // 10.13 Property P8: обновление статуса заявки
    // Feature: job-notifier-enhanced, Property 8: после update_application_status заявка имеет новый статус
    proptest! {
        #[test]
        fn prop_p8_update_application_status(
            new_status_idx in 0usize..5usize,
        ) {
            let statuses = [
                ApplicationStatus::Submitted,
                ApplicationStatus::InReview,
                ApplicationStatus::Rejected,
                ApplicationStatus::OfferReceived,
                ApplicationStatus::Withdrawn,
            ];
            let new_status = statuses[new_status_idx].clone();

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();
                let today = chrono::Local::now().date_naive();
                let app = make_app("id-update", "Co", "Dev", today, 21, ApplicationStatus::Submitted);
                storage.add_application(&app).await.unwrap();
                storage.update_application_status("id-update", &new_status).await.unwrap();
                let list = storage.list_applications().await.unwrap();
                let found = list.iter().find(|a| a.id == "id-update").unwrap();
                prop_assert_eq!(&found.status, &new_status);
                Ok(())
            })?;
        }
    }

    // 10.14 Property P9: удаление заявки
    // Feature: job-notifier-enhanced, Property 9: после delete_application заявка отсутствует в списке
    proptest! {
        #[test]
        fn prop_p9_delete_application(
            company in "[A-Za-z]{1,10}",
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();
                let today = chrono::Local::now().date_naive();
                let app = make_app("id-delete", &company, "Dev", today, 21, ApplicationStatus::Submitted);
                storage.add_application(&app).await.unwrap();
                storage.delete_application("id-delete").await.unwrap();
                let list = storage.list_applications().await.unwrap();
                prop_assert!(!list.iter().any(|a| a.id == "id-delete"));
                Ok(())
            })?;
        }
    }

    // 10.15 Property P10: получение заявок с дедлайном сегодня (исключая терминальные)
    // Feature: job-notifier-enhanced, Property 10: get_deadline_applications возвращает только заявки с deadline==today и нетерминальным статусом
    #[tokio::test]
    async fn prop_p10_deadline_applications_excludes_terminal() {
        let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();

        // Use SQLite's own notion of "today" (UTC) to avoid local/UTC timezone mismatch.
        // We insert applied_at = date('now', '-N days') directly so the deadline lands on date('now').
        let today_utc: String = sqlx::query_scalar("SELECT date('now')")
            .fetch_one(&storage.pool)
            .await
            .unwrap();
        let today = chrono::NaiveDate::parse_from_str(&today_utc, "%Y-%m-%d").unwrap();

        // Заявка с дедлайном сегодня (applied_at = today, days = 0), нетерминальный статус
        let app_active = make_app("id-active", "Co", "Dev", today, 0, ApplicationStatus::Submitted);
        // Заявка с дедлайном сегодня, терминальный статус
        let app_rejected = make_app("id-rejected", "Co", "Dev", today, 0, ApplicationStatus::Rejected);
        // Заявка с дедлайном не сегодня (deadline = today + 30)
        let app_future = make_app("id-future", "Co", "Dev", today, 30, ApplicationStatus::Submitted);

        storage.add_application(&app_active).await.unwrap();
        storage.add_application(&app_rejected).await.unwrap();
        storage.add_application(&app_future).await.unwrap();

        let due = storage.get_deadline_applications().await.unwrap();
        assert!(due.iter().any(|a| a.id == "id-active"), "active app should be in due list");
        assert!(!due.iter().any(|a| a.id == "id-rejected"), "rejected app should NOT be in due list");
        assert!(!due.iter().any(|a| a.id == "id-future"), "future app should NOT be in due list");
    }
}
