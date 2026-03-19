use std::sync::Arc;

use chrono::Local;
use uuid::Uuid;

use crate::domain::application::{AddApplicationCmd, Application, ApplicationStatus};
use crate::errors::StorageError;
use crate::storage::Storage;

pub struct ApplicationTracker {
    storage: Arc<dyn Storage>,
}

impl ApplicationTracker {
    pub fn new(storage: Arc<dyn Storage>) -> Self {
        Self { storage }
    }

    pub async fn add(&self, cmd: AddApplicationCmd) -> Result<Application, StorageError> {
        let app = Application {
            id: Uuid::new_v4().to_string(),
            company: cmd.company,
            position: cmd.position,
            applied_at: Local::now().date_naive(),
            expected_reply_days: cmd.reply_days.unwrap_or(21),
            status: ApplicationStatus::Submitted,
            notes: cmd.notes,
            job_url: cmd.job_url,
        };
        self.storage.add_application(&app).await?;
        Ok(app)
    }

    pub async fn list(&self) -> Result<Vec<Application>, StorageError> {
        self.storage.list_applications().await
    }

    pub async fn update_status(
        &self,
        id: &str,
        status: ApplicationStatus,
    ) -> Result<(), StorageError> {
        self.storage.update_application_status(id, &status).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), StorageError> {
        self.storage.delete_application(id).await
    }

    pub async fn get_due_today(&self) -> Result<Vec<Application>, StorageError> {
        self.storage.get_deadline_applications().await
    }
}
