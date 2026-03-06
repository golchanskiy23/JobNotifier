use async_trait::async_trait;
use crate::domain::Job;
use crate::errors::NotifierError;

/// Trait для всех систем уведомлений
#[async_trait]
pub trait Notifier: Send + Sync {
    /// Отправить уведомление о одной вакансии
    async fn notify(&self, jobs: &[Job]) -> Result<(), NotifierError>;
}

pub mod console;

pub use console::ConsoleNotifier;
