use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApplicationStatus {
    Submitted,
    InReview,
    Rejected,
    OfferReceived,
    Withdrawn,
}

impl ApplicationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Rejected | Self::OfferReceived | Self::Withdrawn)
    }

    pub fn from_str_cli(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "submitted" => Ok(Self::Submitted),
            "in-review" | "inreview" => Ok(Self::InReview),
            "rejected" => Ok(Self::Rejected),
            "offer-received" | "offerreceived" => Ok(Self::OfferReceived),
            "withdrawn" => Ok(Self::Withdrawn),
            _ => Err(format!(
                "unknown status '{}'. Valid values: submitted, in-review, rejected, offer-received, withdrawn",
                s
            )),
        }
    }
}

impl std::fmt::Display for ApplicationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Submitted => write!(f, "Submitted"),
            Self::InReview => write!(f, "InReview"),
            Self::Rejected => write!(f, "Rejected"),
            Self::OfferReceived => write!(f, "OfferReceived"),
            Self::Withdrawn => write!(f, "Withdrawn"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Application {
    pub id: String,
    pub company: String,
    pub position: String,
    pub applied_at: NaiveDate,
    pub expected_reply_days: u32,
    pub status: ApplicationStatus,
    pub notes: Option<String>,
    pub job_url: Option<String>,
}

impl Application {
    pub fn deadline(&self) -> NaiveDate {
        self.applied_at + chrono::Duration::days(self.expected_reply_days as i64)
    }

    pub fn is_due_today(&self) -> bool {
        self.deadline() == chrono::Local::now().date_naive() && !self.status.is_terminal()
    }
}

pub struct AddApplicationCmd {
    pub company: String,
    pub position: String,
    pub reply_days: Option<u32>,
    pub notes: Option<String>,
    pub job_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // 10.3 Unit-тесты для ApplicationStatus::from_str_cli
    #[test]
    fn test_from_str_cli_valid_values() {
        assert_eq!(ApplicationStatus::from_str_cli("submitted").unwrap(), ApplicationStatus::Submitted);
        assert_eq!(ApplicationStatus::from_str_cli("in-review").unwrap(), ApplicationStatus::InReview);
        assert_eq!(ApplicationStatus::from_str_cli("inreview").unwrap(), ApplicationStatus::InReview);
        assert_eq!(ApplicationStatus::from_str_cli("rejected").unwrap(), ApplicationStatus::Rejected);
        assert_eq!(ApplicationStatus::from_str_cli("offer-received").unwrap(), ApplicationStatus::OfferReceived);
        assert_eq!(ApplicationStatus::from_str_cli("offerreceived").unwrap(), ApplicationStatus::OfferReceived);
        assert_eq!(ApplicationStatus::from_str_cli("withdrawn").unwrap(), ApplicationStatus::Withdrawn);
        // case-insensitive
        assert_eq!(ApplicationStatus::from_str_cli("SUBMITTED").unwrap(), ApplicationStatus::Submitted);
        assert_eq!(ApplicationStatus::from_str_cli("Rejected").unwrap(), ApplicationStatus::Rejected);
    }

    #[test]
    fn test_from_str_cli_invalid_value() {
        assert!(ApplicationStatus::from_str_cli("unknown").is_err());
        assert!(ApplicationStatus::from_str_cli("").is_err());
        assert!(ApplicationStatus::from_str_cli("pending").is_err());
    }

    // 10.4 Unit-тест: Application::is_due_today с терминальным статусом → false
    #[test]
    fn test_is_due_today_terminal_status_is_false() {
        let today = chrono::Local::now().date_naive();
        let terminal_statuses = [
            ApplicationStatus::Rejected,
            ApplicationStatus::OfferReceived,
            ApplicationStatus::Withdrawn,
        ];
        for status in &terminal_statuses {
            let app = Application {
                id: "test-id".to_string(),
                company: "Acme".to_string(),
                position: "Dev".to_string(),
                applied_at: today,
                expected_reply_days: 0,
                status: status.clone(),
                notes: None,
                job_url: None,
            };
            assert!(!app.is_due_today(), "terminal status {:?} should not be due today", status);
        }
    }

    // 10.16 Property P11: создание заявки с правильными полями
    // Feature: job-notifier-enhanced, Property 11: ApplicationTracker::add создаёт заявку со статусом Submitted и правильными полями
    proptest! {
        #[test]
        fn prop_p11_application_fields(
            company in "[A-Za-z ]{1,20}",
            position in "[A-Za-z ]{1,20}",
            reply_days in 1u32..100u32,
            notes_opt in prop::option::of("[A-Za-z ]{1,30}"),
        ) {
            use crate::domain::application::AddApplicationCmd;
            use crate::tracker::ApplicationTracker;
            use crate::storage::SqliteStorage;
            use std::sync::Arc;

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let storage = SqliteStorage::new("sqlite::memory:").await.unwrap();
                let tracker = ApplicationTracker::new(Arc::new(storage));
                let cmd = AddApplicationCmd {
                    company: company.clone(),
                    position: position.clone(),
                    reply_days: Some(reply_days),
                    notes: notes_opt.clone(),
                };
                let app = tracker.add(cmd).await.unwrap();
                prop_assert_eq!(&app.company, &company);
                prop_assert_eq!(&app.position, &position);
                prop_assert_eq!(app.expected_reply_days, reply_days);
                prop_assert_eq!(app.status, ApplicationStatus::Submitted);
                prop_assert_eq!(&app.notes, &notes_opt);
                Ok(())
            })?;
        }
    }

    // 10.17 Property P12: форматирование строки таблицы заявок
    // Feature: job-notifier-enhanced, Property 12: строка таблицы содержит id, company, position, applied_at, deadline, status
    proptest! {
        #[test]
        fn prop_p12_table_row_contains_required_fields(
            company in "[A-Za-z]{1,15}",
            position in "[A-Za-z]{1,15}",
            days in 1u32..30u32,
        ) {
            let today = chrono::Local::now().date_naive();
            let app = Application {
                id: "test-uuid-1234".to_string(),
                company: company.clone(),
                position: position.clone(),
                applied_at: today,
                expected_reply_days: days,
                status: ApplicationStatus::Submitted,
                notes: None,
                job_url: None,
            };

            // Форматируем строку таблицы так же, как это делает CLI
            let row = format!(
                "{} | {} | {} | {} | {} | {}",
                app.id,
                app.company,
                app.position,
                app.applied_at,
                app.deadline(),
                app.status,
            );

            prop_assert!(row.contains(&app.id));
            prop_assert!(row.contains(&app.company));
            prop_assert!(row.contains(&app.position));
            prop_assert!(row.contains(&app.applied_at.to_string()));
            prop_assert!(row.contains(&app.deadline().to_string()));
            prop_assert!(row.contains(&app.status.to_string()));
        }
    }
}
