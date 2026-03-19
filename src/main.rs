mod config;
mod domain;
mod scraper;
mod notifier;
mod storage;
mod scheduler;
mod errors;
mod tracker;

use crate::config::AppConfig;
use crate::domain::Filter;
use crate::domain::application::{AddApplicationCmd, ApplicationStatus};
use crate::scraper::Scraper;
use crate::notifier::Notifier;
use crate::storage::{Storage, SqliteStorage};
use crate::tracker::ApplicationTracker;
use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run(args) => run_main(args).await,
        Commands::AddApplication(args) => {
            let storage: Arc<dyn Storage> = Arc::new(
                SqliteStorage::new("sqlite:job_notifier.db")
                    .await
                    .context("failed to initialize storage")?,
            );
            handle_add_application(storage, args).await
        }
        Commands::ListApplications => {
            let storage: Arc<dyn Storage> = Arc::new(
                SqliteStorage::new("sqlite:job_notifier.db")
                    .await
                    .context("failed to initialize storage")?,
            );
            handle_list_applications(storage).await
        }
        Commands::UpdateStatus(args) => {
            let storage: Arc<dyn Storage> = Arc::new(
                SqliteStorage::new("sqlite:job_notifier.db")
                    .await
                    .context("failed to initialize storage")?,
            );
            handle_update_status(storage, args).await
        }
        Commands::DeleteApplication(args) => {
            let storage: Arc<dyn Storage> = Arc::new(
                SqliteStorage::new("sqlite:job_notifier.db")
                    .await
                    .context("failed to initialize storage")?,
            );
            handle_delete_application(storage, args).await
        }
    }
}

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "job-notifier")]
#[command(about = "Async Rust job notifier with SQLite storage and local notifications")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the job-notifier scheduler (default mode)
    Run(RunArgs),
    /// Add a new job application
    AddApplication(AddApplicationArgs),
    /// List all job applications
    ListApplications,
    /// Update the status of a job application
    UpdateStatus(UpdateStatusArgs),
    /// Delete a job application
    DeleteApplication(DeleteApplicationArgs),
}

#[derive(Parser, Debug)]
struct RunArgs {
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

#[derive(Parser, Debug)]
struct AddApplicationArgs {
    #[arg(long)]
    company: String,

    #[arg(long)]
    position: String,

    #[arg(long)]
    reply_days: Option<u32>,

    #[arg(long)]
    notes: Option<String>,

    #[arg(long)]
    job_url: Option<String>,
}

#[derive(Parser, Debug)]
struct UpdateStatusArgs {
    #[arg(long)]
    id: String,

    #[arg(long)]
    status: String,
}

#[derive(Parser, Debug)]
struct DeleteApplicationArgs {
    #[arg(long)]
    id: String,
}

// ── Command handlers ──────────────────────────────────────────────────────────

async fn run_main(args: RunArgs) -> Result<()> {
    let storage: Box<dyn Storage> = Box::new(
        SqliteStorage::new("sqlite:job_notifier.db")
            .await
            .context("failed to initialize storage")?,
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

    cfg.validate().context("config validation failed")?;

    let browser_scraper: Option<Box<dyn Scraper>> = if cfg.scraping.use_browser || !cfg.scraping.browser_urls.is_empty() {
        Some(Box::new(crate::scraper::BrowserScraper::new(
            cfg.scraping.keywords.clone(),
            cfg.scraping.user_agent.clone(),
            cfg.scraping.chrome_path.clone(),
            cfg.scraping.browser_wait_ms,
            cfg.companies.clone(),
        )))
    } else {
        None
    };

    let default_scraper: Box<dyn Scraper> = Box::new(crate::scraper::UniversalScraper::new(
        cfg.scraping.keywords.clone(),
        cfg.scraping.user_agent.clone(),
        cfg.companies.clone(),
    ));

    let browser_urls = if cfg.scraping.browser_urls.is_empty() && cfg.scraping.use_browser {
        // Старый режим: use_browser=true без browser_urls → все URL через браузер
        cfg.scraping.urls.clone()
    } else {
        cfg.scraping.browser_urls.clone()
    };

    let notifiers: Vec<Box<dyn Notifier>> = vec![
        Box::new(crate::notifier::ConsoleNotifier),
    ];

    let mut scheduler = scheduler::JobScheduler::new(
        default_scraper,
        browser_scraper,
        browser_urls,
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

async fn handle_add_application(
    storage: Arc<dyn Storage>,
    args: AddApplicationArgs,
) -> Result<()> {
    let tracker = ApplicationTracker::new(storage);
    let cmd = AddApplicationCmd {
        company: args.company,
        position: args.position,
        reply_days: args.reply_days,
        notes: args.notes,
        job_url: args.job_url,
    };
    let app = tracker.add(cmd).await.context("failed to add application")?;
    println!("Application added:");
    println!("  ID:       {}", app.id);
    println!("  Company:  {}", app.company);
    println!("  Position: {}", app.position);
    println!("  Applied:  {}", app.applied_at);
    println!("  Deadline: {}", app.deadline());
    println!("  Status:   {}", app.status);
    if let Some(notes) = &app.notes {
        println!("  Notes:    {}", notes);
    }
    if let Some(url) = &app.job_url {
        println!("  Job URL:  {}", url);
    }
    Ok(())
}

async fn handle_list_applications(storage: Arc<dyn Storage>) -> Result<()> {
    let tracker = ApplicationTracker::new(storage);
    let apps = tracker.list().await.context("failed to list applications")?;

    if apps.is_empty() {
        println!("No applications found.");
        return Ok(());
    }

    // Column widths
    let id_w = 36;
    let company_w = apps.iter().map(|a| a.company.len()).max().unwrap_or(7).max(7);
    let position_w = apps.iter().map(|a| a.position.len()).max().unwrap_or(8).max(8);
    let date_w = 10;
    let status_w = 13;

    println!(
        "{:<id_w$}  {:<company_w$}  {:<position_w$}  {:<date_w$}  {:<date_w$}  {:<status_w$}  {}",
        "ID", "Company", "Position", "Applied", "Deadline", "Status", "Job URL",
        id_w = id_w,
        company_w = company_w,
        position_w = position_w,
        date_w = date_w,
        status_w = status_w,
    );
    let sep_len = id_w + 2 + company_w + 2 + position_w + 2 + date_w + 2 + date_w + 2 + status_w + 2 + 7;
    println!("{}", "-".repeat(sep_len));

    for app in &apps {
        println!(
            "{:<id_w$}  {:<company_w$}  {:<position_w$}  {:<date_w$}  {:<date_w$}  {:<status_w$}  {}",
            app.id,
            app.company,
            app.position,
            app.applied_at.to_string(),
            app.deadline().to_string(),
            app.status.to_string(),
            app.job_url.as_deref().unwrap_or("—"),
            id_w = id_w,
            company_w = company_w,
            position_w = position_w,
            date_w = date_w,
            status_w = status_w,
        );
    }

    Ok(())
}

async fn handle_update_status(
    storage: Arc<dyn Storage>,
    args: UpdateStatusArgs,
) -> Result<()> {
    let status = match ApplicationStatus::from_str_cli(&args.status) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    let tracker = ApplicationTracker::new(storage);
    tracker
        .update_status(&args.id, status)
        .await
        .context("failed to update status")?;
    println!("Status updated for application '{}'.", args.id);
    Ok(())
}

async fn handle_delete_application(
    storage: Arc<dyn Storage>,
    args: DeleteApplicationArgs,
) -> Result<()> {
    let tracker = ApplicationTracker::new(storage);
    tracker
        .delete(&args.id)
        .await
        .context("failed to delete application")?;
    println!("Application '{}' deleted.", args.id);
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn show_stats(storage: &Box<dyn Storage>) -> Result<()> {
    let stats = storage.get_stats().await.context("Failed to get stats")?;
    println!("Job Statistics:");
    println!("Total jobs seen: {}", stats.total_seen);
    println!("Jobs in last 24h: {}", stats.last_24h);
    Ok(())
}

async fn show_recent_jobs(storage: &Box<dyn Storage>, limit: usize) -> Result<()> {
    let jobs = storage
        .get_seen_jobs(Some(limit as i64))
        .await
        .context("Failed to get recent jobs")?;

    println!("Last {} jobs:", limit);
    println!("{}", "=".repeat(50));

    for (i, job) in jobs.iter().enumerate() {
        println!("Job #{}", i + 1);
        println!("Company: {}", job.company);
        println!("Title: {}", job.title);
        println!("URL: {}", job.url);
        println!();
    }

    Ok(())
}

async fn cleanup_old_jobs(storage: &Box<dyn Storage>, days: u64) -> Result<()> {
    let deleted = storage
        .cleanup_old_jobs(days as i64)
        .await
        .context("Failed to cleanup old jobs")?;
    println!(
        "Cleaned up {} old job records (older than {} days)",
        deleted, days
    );
    Ok(())
}

fn create_filter() -> Box<dyn Filter> {
    // Filtering is already done by UniversalScraper via keywords config.
    // Return a pass-through filter that accepts all jobs.
    struct AllowAll;
    impl Filter for AllowAll {
        fn matches(&self, _job: &crate::domain::Job) -> bool { true }
    }
    Box::new(AllowAll)
}
