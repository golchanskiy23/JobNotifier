// В этом файле мы покажем, как ownership и borrowing работают
// на примере доменной модели вакансий и простого парсера HTML.

// Подключаем модуль с доменными структурами (`Job`, фильтр, конфиг парсера и т.п.)
mod domain;

// Подключаем модуль с функциями парсинга (`parse_jobs`, `dedup` и т.д.)
mod parser;

// Модуль с trait-архитектурой и планировщиком.
mod scheduler;

use crate::domain::{Filter, Job, JobFilter, ScraperConfig};
use crate::parser::{dedup, parse_jobs};
use crate::scheduler::{
    AsyncHhScraper, AsyncScheduler, HhScraper, InMemoryStorage, Scheduler, TelegramNotifier,
};

/// Точка входа в программу.
///
/// Атрибут `#[tokio::main]` поднимает runtime Tokio:
///  - создаётся event loop;
///  - подготавливается thread pool;
///  - запускается `main` как асинхронная задача.
#[tokio::main]
async fn main() {
    // Имитируем HTML‑страницу с вакансиями как строковый литерал.
    // Тип: `&'static str` — ссылка на строку, зашитую в бинарник, без аллокаций в куче.
    let html: &str = r#"
        <div class="job-card">
            <div class="title">Junior Rust Developer</div>
            <div class="company">Acme Corp</div>
            <div class="tech">Rust, Tokio, SQL</div>
        </div>
        <div class="job-card">
            <div class="title">Intern Backend Engineer</div>
            <div class="company">Startup X</div>
            <div class="tech">Rust, Actix, Postgres</div>
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
    scheduler.run();

    // --- Асинхронный цикл scraping через Tokio и join_all ---

    // Набор URL (в реальности пришли бы из конфига).
    let urls = vec![
        "https://hh.ru/search/vacancy?text=rust".to_string(),
        "https://hh.ru/search/vacancy?text=backend".to_string(),
    ];

    // Async-скрейпер и async-scheduler работают через те же trait'ы Filter/Notifier/Storage,
    // но сами операции scraping выполняются конкурентно внутри Tokio.
    let async_scrapers: Vec<Box<dyn scheduler::AsyncScraper>> = vec![Box::new(AsyncHhScraper)];
    let async_notifiers: Vec<Box<dyn scheduler::Notifier + Send + Sync>> =
        vec![Box::new(TelegramNotifier)];
    let async_storage: Box<dyn scheduler::Storage + Send + Sync> = Box::new(InMemoryStorage::new());
    let async_filter: Box<dyn Filter + Send + Sync> = Box::new(filter);

    let mut async_scheduler =
        AsyncScheduler::new(async_scrapers, async_notifiers, async_storage, async_filter);

    // Здесь мы реально "входим" в async‑мир:
    //  - для каждого URL создаётся асинхронная задача scraping;
    //  - Tokio не блокирует поток, пока ожидает сетевые ответы;
    //  - все URL обрабатываются параллельно, а не по очереди.
    async_scheduler.run_async(&urls).await;
}

