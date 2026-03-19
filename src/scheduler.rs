use crate::domain::{Job, Filter};
use crate::scraper::Scraper;
use crate::notifier::Notifier;
use crate::storage::Storage;
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::{interval_at, Instant, sleep};
use chrono::Timelike;

pub struct JobScheduler {
    scrapers: Vec<Box<dyn Scraper>>,
    notifiers: Vec<Box<dyn Notifier>>,
    storage: Box<dyn Storage>,
    filter: Box<dyn Filter>,
}

impl JobScheduler {
    pub fn new(
        scrapers: Vec<Box<dyn Scraper>>,
        notifiers: Vec<Box<dyn Notifier>>,
        storage: Box<dyn Storage>,
        filter: Box<dyn Filter>,
    ) -> Self {
        Self {
            scrapers,
            notifiers,
            storage,
            filter,
        }
    }
    
    pub async fn run_once(&mut self, urls: &[String]) -> Result<()> {
        println!("Starting job search...");
        
        let all_jobs = self.scrape_all_urls(urls).await?;
        
        let filtered_jobs: Vec<Job> = all_jobs
            .into_iter()
            .filter(|job| self.filter.matches(job))
            .collect();
        
        let new_jobs = self.deduplicate_and_save(&filtered_jobs).await?;
        
        if !new_jobs.is_empty() {
            for notifier in &self.notifiers {
                notifier.notify(&new_jobs).await
                    .context("Failed to send notification")?;
            }
        } else {
            println!("No new jobs found");
        }
        
        self.show_stats().await?;

        // Check deadline applications
        match self.storage.get_deadline_applications().await {
            Ok(apps) if !apps.is_empty() => {
                for notifier in &self.notifiers {
                    if let Err(e) = notifier.notify_deadlines(&apps).await {
                        eprintln!("Error sending deadline notifications: {}", e);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error checking deadline applications: {}", e);
            }
        }

        println!("Job search completed");
        Ok(())
    }
    
    pub async fn run_scheduler(&mut self, urls: &[String]) -> Result<()> {
        println!("Starting job scheduler...");
        
        let target_hour = 11;
        let target_minute = 02;
        let initial_delay = self.compute_initial_delay(target_hour, target_minute);
        let start = Instant::now() + initial_delay;
        
        let mut tick = interval_at(start, Duration::from_secs(60 * 60 * 24));
        
        println!("Scheduler will start at {}:{}", target_hour, target_minute);
        println!("Press Ctrl+C to stop the scheduler");
        
        loop {
            tokio::select! {
                _ = tick.tick() => {
                    println!("Running scheduled job check...");
                    if let Err(e) = self.run_once(urls).await {
                        eprintln!("Error in scheduled run: {}", e);
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("Ctrl+C received - shutting down scheduler...");
                    break;
                }
            }
        }
        
        println!("Scheduler stopped gracefully");
        Ok(())
    }
    
    async fn scrape_all_urls(&self, urls: &[String]) -> Result<Vec<Job>> {
        let mut all_jobs = Vec::new();
        
        for scraper in &self.scrapers {
            for url in urls {
                println!("Scraping {} with {}", scraper.name(), url);
                
                match scraper.scrape(url).await {
                    Ok(jobs) => {
                        println!("Found {} jobs from {}", jobs.len(), scraper.name());
                        all_jobs.extend(jobs);
                    }
                    Err(e) => {
                        eprintln!("Error scraping {}: {}", url, e);
                    }
                }
                
                sleep(Duration::from_millis(500)).await;
            }
        }
        
        Ok(all_jobs)
    }
    
    async fn deduplicate_and_save(&mut self, jobs: &[Job]) -> Result<Vec<Job>> {
        let mut new_jobs = Vec::new();
        
        for job in jobs {
            if !self.storage.is_job_seen(job).await? {
                self.storage.mark_job_seen(job).await
                    .context("Failed to mark job as seen")?;
                new_jobs.push(job.clone());
            }
        }
        
        Ok(new_jobs)
    }
    
    async fn show_stats(&self) -> Result<()> {
        match self.storage.get_stats().await {
            Ok(stats) => {
                println!("Statistics:");
                println!("Total jobs seen: {}", stats.total_seen);
                println!("Jobs in last 24h: {}", stats.last_24h);
            }
            Err(e) => {
                eprintln!("Failed to get stats: {}", e);
            }
        }
        Ok(())
    }
    
    fn compute_initial_delay(&self, target_hour: u32, target_minute: u32) -> Duration {
        use chrono::Local;
        
        let now = Local::now();
        let today_target = now
            .with_hour(target_hour)
            .and_then(|dt: chrono::DateTime<Local>| dt.with_minute(target_minute))
            .and_then(|dt: chrono::DateTime<Local>| dt.with_second(0))
            .and_then(|dt: chrono::DateTime<Local>| dt.with_nanosecond(0))
            .expect("invalid target time");
        
        let next_run = if today_target > now {
            today_target
        } else {
            today_target + chrono::Duration::days(1)
        };
        
        let diff = next_run - now;
        Duration::from_secs(diff.num_seconds().max(0) as u64)
    }
}
