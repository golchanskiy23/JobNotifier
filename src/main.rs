// В этом файле мы покажем, как ownership и borrowing работают
// на примере доменной модели вакансий и простого парсера HTML.

// Подключаем модуль с доменными структурами (`Job`, фильтр, конфиг парсера и т.п.)
mod domain;

// Подключаем модуль с функциями парсинга (`parse_jobs`, `dedup` и т.д.)
mod parser;

// Модуль конфигурации: список URL и прочие настройки.
mod config;

// Модуль с trait-архитектурой и планировщиком.
mod scheduler;
mod errors;

use crate::config::AppConfig;
use crate::domain::{Filter, Job, JobFilter, ScraperConfig};
use crate::parser::{dedup, parse_jobs};
use crate::scheduler::{
    AsyncHhScraper, AsyncScheduler, HhScraper, InMemoryStorage, Scheduler, TelegramNotifier,
};
use chrono::{Local, Timelike};
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
    // Имитируем HTML‑страницу с вакансиями как строковый литерал.
    // Тип: `&'static str` — ссылка на строку, зашитую в бинарник, без аллокаций в куче.
    let html: &str = r#"
        <div class="vacancy-card">
            <a class="vacancy-card__title" href="/vacancy/123">
                Junior Rust Developer
            </a>
        </div>
        <div class="vacancy-card">
            <a class="vacancy-card__title" href="/vacancy/456">
                Intern Backend Engineer
            </a>
        </div>
    "#;

    // Пример использования конфига парсера с lifetime.
    // Здесь и `url`, и CSS‑селектор — строковые литералы с `'static` lifetime,
    // поэтому `ScraperConfig<'static>` безопасен.
    let config = ScraperConfig {
        url: "https://hh.ru",
        job_card_selector: "div.job-card",
    };

    println!("Scraping from: {} with selector: {}", config.url, config.job_card_selector);

    // Вызываем парсер, передавая `&str`, а не `String`.
    // HTML уже в памяти — нет смысла копировать его в новый `String`.
    let mut jobs: Vec<Job> = parse_jobs(html);

    // Настраиваем фильтр: ищем вакансии по Rust и company = "Acme Corp".
    // Здесь мы явно создаём `String` — фильтр должен жить дольше, чем временные &str.
    let filter = JobFilter {
        min_grade: Some("junior".to_string()),
        required_tech: vec!["Rust".to_string()],
        company: Some("Acme Corp".to_string()),
    };

    // Используем borrowing: в замыкание попадают ссылки `&Job`, а не владение.
    // `iter()` даёт `&Job`, `filter.matches(job)` получает `&Job`, сами `Job` остаются во владении `Vec`.
    let filtered: Vec<&Job> = jobs
        .iter()
        // `Filter` — это trait, реализованный для `JobFilter`.
        // Благодаря этому `scheduler` и другой код могут работать
        // через обобщённый интерфейс, а не через конкретный тип.
        .filter(|job| filter.matches(job))
        .collect();

    // Печатаем отфильтрованные вакансии.
    // Здесь мы по-прежнему работаем только с заимствованными данными (`&Job`), не двигая владение.
    for job in &filtered {
        println!("Matched job: {} at {}", job.title, job.company);
    }

    // Функция `dedup` принимает владение `Vec<Job>`, так как будет перестраивать коллекцию.
    // После вызова `dedup(jobs)` переменная `jobs` больше недоступна — она была move'нута.
    jobs = dedup(jobs);

    // Теперь можно снова итерироваться по `jobs` — это уже новый вектор без дубликатов.
    for job in &jobs {
        println!("Unique job: {} at {}", job.title, job.company);
    }

    // --- Демонстрация trait-архитектуры с dyn Scraper / Notifier / Storage ---

    // Создаём dyn-объекты scrapers / notifiers / storage.
    // Типы (`HhScraper`, `TelegramNotifier`, `InMemoryStorage`) спрятаны за trait'ами —
    // scheduler видит только `dyn Scraper`, `dyn Notifier`, `dyn Storage`, `dyn Filter`.
    let scrapers: Vec<Box<dyn scheduler::Scraper>> = vec![Box::new(HhScraper)];
    let notifiers: Vec<Box<dyn scheduler::Notifier>> = vec![Box::new(TelegramNotifier)];
    let storage: Box<dyn scheduler::Storage> = Box::new(InMemoryStorage::new());

    // Для scheduler'а фильтр тоже выступает как `dyn Filter`.
    let filter_box: Box<dyn Filter> = Box::new(filter.clone());

    let mut scheduler = Scheduler::new(scrapers, notifiers, storage, filter_box);
    scheduler.run().context("sync scheduler failed")?;

    // --- Загрузка конфигурации списка URL из TOML-файла ---

    // Конфиг лежит рядом с бинарём / в корне проекта.
    // Если файл не найден или содержит ошибку, мы логируем проблему и
    // продолжаем с пустым списком URL (демо-режим).
    let cfg = match AppConfig::load_from_file("Config.toml") {
        Ok(cfg) => cfg,
        Err(err) => {
            // Здесь мы используем реализацию Display для ConfigError,
            // что "прочитывает" внутренние поля и устраняет предупреждения о dead_code.
            eprintln!(
                "Failed to load Config.toml: {}. Running with empty URL list.",
                err
            );
            AppConfig {
                scraping: config::ScrapingConfig { urls: Vec::new() },
            }
        }
    };

    // Набор URL теперь задаётся пользователем в `Config.toml` и может содержать
    // любые сайты: hh.ru, lamoda, avito, linkedin и т.д.
    let urls = cfg.scraping.urls.clone();

    // Async-скрейпер и async-scheduler работают через те же trait'ы Filter/Notifier/Storage,
    // но сами операции scraping выполняются конкурентно внутри Tokio.
    let async_scrapers: Vec<Box<dyn scheduler::AsyncScraper>> = vec![Box::new(AsyncHhScraper)];
    let async_notifiers: Vec<Box<dyn scheduler::Notifier + Send + Sync>> =
        vec![Box::new(TelegramNotifier)];
    let async_storage: Box<dyn scheduler::Storage + Send + Sync> = Box::new(InMemoryStorage::new());
    let async_filter: Box<dyn Filter + Send + Sync> = Box::new(filter);

    let mut async_scheduler =
        AsyncScheduler::new(async_scrapers, async_notifiers, async_storage, async_filter);

    // --- Главный цикл планировщика: один запуск в день в заданное время ---

    // Ежедневное время запуска (локальное): 19:00.
    let target_hour = 19;
    let target_minute = 0;

    // Вычисляем, через сколько времени нужно запустить первый цикл,
    // чтобы он попал на "следующие 19:00" (сегодня или завтра).
    let initial_delay = compute_initial_delay(target_hour, target_minute);
    let start = Instant::now() + initial_delay;

    // Интервал 24 часа: после первого тика в 19:00 последующие будут каждый день
    // в это же время.
    let mut tick: Interval = interval_at(start, Duration::from_secs(24 * 60 * 60));
    // Токен для "мягкой" остановки (graceful shutdown).
    let token = CancellationToken::new();

    loop {
        tokio::select! {
            // Каждое "тиканье" интервала запускает полный async-цикл scraping.
            _ = tick.tick() => {
                async_scheduler
                    .run_async(&urls)
                    .await
                    .context("failed to run scraping cycle")?;
            }
            // Ожидаем отмену токена (например, по сигналу).
            _ = token.cancelled() => {
                println!("Shutdown requested via CancellationToken");
                break;
            }
        }
    }

    Ok(())
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

