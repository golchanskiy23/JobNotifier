// В этом файле мы покажем, как ownership и borrowing работают
// на примере доменной модели вакансий и простого парсера HTML.

// Подключаем модуль с доменными структурами (`Job`, фильтр, конфиг парсера и т.п.)
mod domain;

// Подключаем модуль с функциями парсинга (`parse_jobs`, `dedup` и т.д.)
mod parser;

use crate::domain::{Job, JobFilter, ScraperConfig};
use crate::parser::{dedup, parse_jobs};

fn main() {
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
}

