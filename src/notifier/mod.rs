use async_trait::async_trait;
use crate::domain::Job;
use crate::errors::NotifierError;

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, jobs: &[Job]) -> Result<(), NotifierError>;
}

pub mod console;

pub use console::ConsoleNotifier;
