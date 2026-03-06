// В этом файле мы покажем, как ownership и borrowing работают
// на примере доменной модели вакансий и простого парсера HTML.

// Подключаем модули новой модульной архитектуры
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

/// Точка входа в программу.
///
/// Атрибут `#[tokio::main]` поднимает runtime Tokio:
///  - создаётся event loop;
///  - подготавливается thread pool;
///  - запускается `main` как асинхронная задача.
#[tokio::main]
async fn main() -> Result<()> {
    // --- Парсим CLI-аргументы ---
    let args = CliArgs::parse();
    
    // --- Загрузка и валидация конфигурации ---
    let cfg = AppConfig::load_from_file(&args.config)
        .with_context(|| format!("failed to load config from {}", args.config))?;
    
    cfg.validate()
        .context("config validation failed")?;
    
    // --- Инициализация хранилища ---
    let storage: Box<dyn Storage> = Box::new(
        SqliteStorage::new("sqlite:job_notifier.db")
            .await
            .context("failed to initialize storage")?
    );
    
    // --- Инициализация скрейперов ---
    let scrapers: Vec<Box<dyn Scraper>> = vec![
        Box::new(crate::scraper::HhScraper),
    ];
    
    // --- Инициализация нотификаторов ---
    let notifiers: Vec<Box<dyn Notifier>> = vec![
        Box::new(crate::notifier::ConsoleNotifier),
    ];
    
    // --- Создание и запуск планировщика ---
    let mut scheduler = scheduler::JobScheduler::new(
        scrapers,
        notifiers,
        storage,
        create_filter(),
    );
    
    // --- Запуск в режиме однократного выполнения или планировщика ---
    if args.run_once {
        scheduler.run_once(&cfg.scraping.urls).await?;
    } else {
        scheduler.run_scheduler(&cfg.scraping.urls).await?;
    }
    
    Ok(())
}

/// Создает комбинированный фильтр вакансий
fn create_filter() -> Box<dyn Filter> {
    use crate::filter::{GradeFilter, KeywordFilter, TechFilter, AndFilter};
    
    let grade_filter = GradeFilter::new(Some(crate::domain::JobGrade::Junior));
    let keyword_filter = KeywordFilter::new(
        vec!["rust".to_string(), "backend".to_string()],
        vec!["senior".to_string(), "lead".to_string()], // Исключаем senior позиции
    );
    let tech_filter = TechFilter::new(
        vec!["rust".to_string()], // Обязательно Rust
        vec![],
    );
    
    // Комбинируем фильтры: все условия должны выполняться
    Box::new(AndFilter::new(
        AndFilter::new(grade_filter, keyword_filter),
        tech_filter,
    ))
}

/// CLI-параметры приложения.
#[derive(Parser, Debug)]
#[command(name = "job-notifier")]
#[command(about = "Async Rust job notifier with SQLite storage and local notifications")]
struct CliArgs {
    /// Путь к TOML-конфигу. По умолчанию — `Config.toml` в текущем каталоге.
    #[arg(long, default_value = "Config.toml")]
    config: String,

    /// Запустить один раз и выйти
    #[arg(long)]
    run_once: bool,
}

/// Считает задержку до ближайшего запуска в локальном времени `HH:MM`.
///
/// Пример: сейчас 18:30, `target_hour = 19`, `target_minute = 0` → ~30 минут.
///         сейчас 20:10, `target_hour = 19`, `target_minute = 0` → до 19:00 завтрашнего дня.
fn compute_initial_delay(target_hour: u32, target_minute: u32) -> Duration {
    let now = Local::now();

    // Конструируем "сегодняшнюю" цель в 19:00 локального времени.
    let today_target = now
        .with_hour(target_hour)
        .and_then(|dt| dt.with_minute(target_minute))
        .and_then(|dt| dt.with_second(0))
        .and_then(|dt| dt.with_nanosecond(0))
        .expect("invalid target time");

    let next_run = if today_target > now {
        // Если 19:00 ещё не наступило — запускаемся сегодня.
        today_target
    } else {
        // Иначе переносим цель на завтра в то же время.
        today_target + chrono::Duration::days(1)
    };

    let diff = next_run - now;
    // Переводим chrono::Duration в std::time::Duration.
    Duration::from_secs(diff.num_seconds().max(0) as u64)
}

