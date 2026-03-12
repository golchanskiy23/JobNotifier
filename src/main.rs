mod config;
mod domain;
mod scraper;
mod filter;
mod notifier;
mod storage;
mod scheduler;
mod errors;

use crate::config::AppConfig;
use crate::domain::Filter;
use crate::scraper::Scraper;
use crate::notifier::Notifier;
use crate::storage::{Storage, SqliteStorage};
use chrono::{Local, Timelike};
use clap::Parser;
use std::time::Duration;
use tokio::time::{interval_at, Instant, Interval};
use tokio_util::sync::CancellationToken;
use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();

    let storage: Box<dyn Storage> = Box::new(
        SqliteStorage::new("sqlite:job_notifier.db")
            .await
            .context("failed to initialize storage")?
    );

    if args.stats {
        show_stats(&storage).await?;
        return Ok(());
    }

    if let Some(limit) = args.recent {
        show_recent_jobs(&storage, limit).await?;
        return Ok(());
    }

    if let Some(days) = args.cleanup {
        cleanup_old_jobs(&storage, days).await?;
        return Ok(());
    }

    let cfg = AppConfig::load_from_file(&args.config)
        .with_context(|| format!("failed to load config from {}", args.config))?;
    
    cfg.validate()
        .context("config validation failed")?;
    
    let scrapers: Vec<Box<dyn Scraper>> = vec![
        Box::new(crate::scraper::HhScraper),
    ];
    
    let notifiers: Vec<Box<dyn Notifier>> = vec![
        Box::new(crate::notifier::ConsoleNotifier),
    ];
    
    let mut scheduler = scheduler::JobScheduler::new(
        scrapers,
        notifiers,
        storage,
        create_filter(),
    );
    
    if args.run_once {
        scheduler.run_once(&cfg.scraping.urls).await?;
    } else {
        scheduler.run_scheduler(&cfg.scraping.urls).await?;
    }
    
    Ok(())
}

async fn show_stats(storage: &Box<dyn Storage>) -> Result<()> {
    let stats = storage.get_stats().await
        .context("Failed to get stats")?;
    
    println!("Job Statistics:");
    println!("Total jobs seen: {}", stats.total_seen);
    println!("Jobs in last 24h: {}", stats.last_24h);
    
    Ok(())
}

async fn show_recent_jobs(storage: &Box<dyn Storage>, limit: usize) -> Result<()> {
    let jobs = storage.get_seen_jobs(Some(limit as i64)).await
        .context("Failed to get recent jobs")?;
    
    println!("Last {} jobs:", limit);
    println!("{}", "=".repeat(50));
    
    for (i, job) in jobs.iter().enumerate() {
        println!("Job #{}", i + 1);
        println!("Company: {}", job.company);
        println!("Title: {}", job.title);
        println!("URL: {}", job.url);
        if let Some(grade) = &job.grade {
            println!("Grade: {:?}", grade);
        }
        if !job.tech_stack.is_empty() {
            println!("Tech Stack: {}", job.tech_stack.join(", "));
        }
        if let Some(salary) = &job.salary {
            println!("Salary: {}", salary);
        }
        println!("Found: {}", job.seen_at.format("%Y-%m-%d %H:%M:%S UTC"));
        println!();
    }
    
    Ok(())
}

async fn cleanup_old_jobs(storage: &Box<dyn Storage>, days: u64) -> Result<()> {
    let deleted = storage.cleanup_old_jobs(days as i64).await
        .context("Failed to cleanup old jobs")?;
    
    println!("Cleaned up {} old job records (older than {} days)", deleted, days);
    
    Ok(())
}

fn create_filter() -> Box<dyn Filter> {
    use crate::filter::{GradeFilter, KeywordFilter, TechFilter, AndFilter};
    
    let grade_filter = GradeFilter::new(Some(crate::domain::JobGrade::Junior));
    let keyword_filter = KeywordFilter::new(
        vec!["rust".to_string(), "backend".to_string()],
        vec!["senior".to_string(), "lead".to_string()],
    );
    let tech_filter = TechFilter::new(
        vec!["rust".to_string()],
        vec![],
    );
    
    Box::new(AndFilter::new(
        AndFilter::new(grade_filter, keyword_filter),
        tech_filter,
    ))
}

#[derive(Parser, Debug)]
#[command(name = "job-notifier")]
#[command(about = "Async Rust job notifier with SQLite storage and local notifications")]
struct CliArgs {
    #[arg(long, default_value = "Config.toml")]
    config: String,

    #[arg(long)]
    run_once: bool,

    #[arg(long)]
    stats: bool,

    #[arg(long)]
    recent: Option<usize>,

    #[arg(long)]
    cleanup: Option<u64>,
}

