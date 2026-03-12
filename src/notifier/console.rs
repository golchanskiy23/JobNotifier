use async_trait::async_trait;
use crate::domain::Job;
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
}
