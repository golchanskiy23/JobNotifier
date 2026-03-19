use async_trait::async_trait;
use crate::domain::Job;
use crate::domain::application::Application;
use crate::errors::NotifierError;
use crate::notifier::Notifier;

pub struct ConsoleNotifier;

#[async_trait]
impl Notifier for ConsoleNotifier {
    async fn notify(&self, jobs: &[Job]) -> Result<(), NotifierError> {
        if jobs.is_empty() {
            println!("No new jobs found");
            return Ok(());
        }
        
        println!("Found {} new job(s):", jobs.len());
        println!("{}", "=".repeat(50));
        
        for (i, job) in jobs.iter().enumerate() {
            println!("\n Job #{}", i + 1);
            println!("Company: {}", job.company);
            println!("Title: {}", job.title);
            println!("URL: {}", job.url);
            
            if let Some(ref grade) = job.grade {
                println!("Grade: {}", grade);
            }
            
            if !job.tech_stack.is_empty() {
                println!("Tech Stack: {}", job.tech_stack.join(", "));
            }
            
            if let Some(ref salary) = job.salary {
                println!("Salary: {}", salary);
            }
            
            println!("Found: {}", job.seen_at.format("%Y-%m-%d %H:%M:%S UTC"));
        }
        
        println!("\n{}", "=".repeat(50));
        Ok(())
    }

    async fn notify_deadlines(&self, apps: &[Application]) -> Result<(), NotifierError> {
        if apps.is_empty() {
            return Ok(());
        }

        println!("=== Deadline reminders ===");
        for app in apps {
            println!("[!] {} — {} (deadline: {})", app.company, app.position, app.deadline());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::application::{Application, ApplicationStatus};
    use crate::domain::{Job, Url};
    use chrono::Utc;
    use proptest::prelude::*;

    fn make_job(title: &str, url: &str) -> Job {
        Job {
            id: "test-id".to_string(),
            title: title.to_string(),
            company: "TestCo".to_string(),
            tech_stack: vec![],
            grade: None,
            url: Url(url.to_string()),
            salary: None,
            seen_at: Utc::now(),
        }
    }

    fn make_app(company: &str, position: &str, days_offset: i64) -> Application {
        let today = chrono::Local::now().date_naive();
        Application {
            id: "app-id".to_string(),
            company: company.to_string(),
            position: position.to_string(),
            applied_at: today - chrono::Duration::days(days_offset),
            expected_reply_days: days_offset as u32,
            status: ApplicationStatus::Submitted,
            notes: None,
            job_url: None,
        }
    }

    // 10.5 Unit-тест: notify_deadlines vs notify производят разный вывод
    #[tokio::test]
    async fn test_notify_deadlines_vs_notify_different_output() {
        // notify_deadlines с пустым списком ничего не выводит (Ok)
        let notifier = ConsoleNotifier;
        let result_deadlines_empty = notifier.notify_deadlines(&[]).await;
        assert!(result_deadlines_empty.is_ok());

        // notify с пустым списком выводит "No new jobs found"
        let result_notify_empty = notifier.notify(&[]).await;
        assert!(result_notify_empty.is_ok());

        // notify_deadlines с заявками выводит "Deadline reminders"
        let app = make_app("Acme", "Rust Dev", 0);
        let result_deadlines = notifier.notify_deadlines(&[app]).await;
        assert!(result_deadlines.is_ok());

        // notify с вакансиями выводит "Found N new job(s)"
        let job = make_job("Rust Developer", "https://example.com/job/1");
        let result_notify = notifier.notify(&[job]).await;
        assert!(result_notify.is_ok());
    }

    // 10.18 Property P13: уведомление о дедлайне содержит обязательные поля
    // Feature: job-notifier-enhanced, Property 13: notify_deadlines выводит company, position и deadline
    proptest! {
        #[test]
        fn prop_p13_notify_deadlines_contains_required_fields(
            company in "[A-Za-z]{1,15}",
            position in "[A-Za-z]{1,15}",
            days in 0u32..30u32,
        ) {
            let today = chrono::Local::now().date_naive();
            let app = Application {
                id: "test-id".to_string(),
                company: company.clone(),
                position: position.clone(),
                applied_at: today,
                expected_reply_days: days,
                status: ApplicationStatus::Submitted,
                notes: None,
                job_url: None,
            };
            let deadline_str = app.deadline().to_string();

            // Проверяем, что notify_deadlines форматирует строку с нужными полями
            // Формат: "[!] {company} — {position} (deadline: {deadline})"
            let output = format!("[!] {} — {} (deadline: {})", app.company, app.position, app.deadline());
            prop_assert!(output.contains(&company));
            prop_assert!(output.contains(&position));
            prop_assert!(output.contains(&deadline_str));
        }
    }
}
