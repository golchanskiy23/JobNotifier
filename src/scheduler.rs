// Trait-архитектура планировщика (scheduler).
//
// Здесь собраны центральные trait'ы проекта и их простая связка:
//  - `Scraper` — источник вакансий (hh.ru, habr, ...).
//  - `Notifier` — канал уведомлений (Telegram, Email, ...).
//  - `Storage` — хранилище (SQLite, InMemory, ...).
//  - `Filter` — фильтрация вакансий по условиям.
//
// Scheduler знает только о trait-объектах (`dyn Scraper`, `dyn Notifier`, ...),
// поэтому добавить новый сайт / новый канал означает просто дописать реализацию trait'а.

use crate::domain::{Filter, Job};
use crate::parser::parse_jobs;

/// Trait-обёртка над любым источником вакансий.
///
/// Примеры реализаций:
///  - `HhScraper` для hh.ru;
///  - `HabrScraper` для Хабр Карьеры;
///  - `LinkedInScraper` для LinkedIn.
pub trait Scraper {
    /// Человекочитаемое имя источника — удобно для логов и отладки.
    #[allow(dead_code)] // заглушка: метод пока не используется, но останется в публичном API trait'а
    fn name(&self) -> &str;

    /// Сбор вакансий из конкретного источника.
    ///
    /// В реальном проекте метод был бы `async` и выполнял HTTP-запросы.
    /// Здесь он синхронный для простоты и наглядности trait-объектов.
    fn fetch_jobs(&self) -> Vec<Job>;
}

/// Trait канала уведомлений.
///
/// Примеры реализаций:
///  - `TelegramNotifier` — отправка в Telegram-бота;
///  - `EmailNotifier` — отправка писем;
///  - `MockNotifier` — заглушка для тестов.
pub trait Notifier {
    /// Отправить уведомление о вакансии.
    ///
    /// Используем `&Job`, а не `Job`, чтобы не забирать владение и не копировать структуру.
    fn send(&self, job: &Job);
}

/// Trait абстракции над хранилищем.
///
/// Примеры реализаций:
///  - `SqliteStorage` — реальная база в проде;
///  - `InMemoryStorage` — простая in-memory версия для тестов.
pub trait Storage {
    /// Сохранить набор вакансий.
    ///
    /// Принимаем `&[Job]`, чтобы не забирать владение и не копировать вектор.
    fn save_jobs(&mut self, jobs: &[Job]);

    /// Загрузить все сохранённые вакансии.
    ///
    /// Возвращаем `Vec<Job>` по значению — вызывающий код становится владельцем данных.
    #[allow(dead_code)] // заглушка: метод будет использоваться позже для инкрементальных обновлений
    fn load_jobs(&self) -> Vec<Job>;
}

/// Scheduler — центральный оркестратор.
///
/// Он ничего не знает о конкретных типах:
///  - хранит `Vec<Box<dyn Scraper>>` — набор источников, известен только в runtime;
///  - хранит `Vec<Box<dyn Notifier>>` — список каналов уведомлений;
///  - хранит `Box<dyn Storage>` — конкретная реализация выбирается через конфиг;
///  - хранит `Box<dyn Filter>` — стратегия фильтрации вакансий.
pub struct Scheduler {
    // Набор скрейперов. `Box<dyn Scraper>` — объект на куче с vtable для вызова методов.
    pub scrapers: Vec<Box<dyn Scraper>>,
    // Каналы уведомлений.
    pub notifiers: Vec<Box<dyn Notifier>>,
    // Хранилище.
    pub storage: Box<dyn Storage>,
    // Фильтр вакансий (может быть композитным).
    pub filter: Box<dyn Filter>,
}

impl Scheduler {
    /// Конструктор, принимающий готовые trait-объекты.
    ///
    /// Благодаря этому верхнеуровневый код может конфигурировать scheduler
    /// как угодно: читать список сайтов и каналов из файла, из env и т.п.
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

    /// Простейший запуск: обойти все источники, отфильтровать вакансии,
    /// сохранить их и разослать уведомления.
    ///
    /// Обратите внимание: метод принимает `&mut self`, потому что `Storage`
    /// в сигнатуре имеет `&mut self` в `save_jobs`.
    pub fn run(&mut self) {
        for scraper in &self.scrapers {
            // Вызов `scraper.fetch_jobs()` через vtable — это и есть динамическая
            // диспетчеризация (`dyn Scraper`). Стоимость одной vtable-инструкции
            // ничтожна по сравнению с HTTP-запросами.
            let jobs = scraper.fetch_jobs();

            // Применяем фильтр, работающий через trait-объект `dyn Filter`.
            let filtered: Vec<Job> = jobs
                .into_iter()
                .filter(|job| self.filter.matches(job))
                .collect();

            // Сохраняем вакансии в хранилище.
            self.storage.save_jobs(&filtered);

            // Рассылаем уведомления по всем каналам.
            for job in &filtered {
                for notifier in &self.notifiers {
                    notifier.send(job);
                }
            }
        }
    }
}

/// Простейшая реализация `Scraper` для hh.ru.
///
/// В реальном коде она бы выполняла HTTP-запросы и разбирала HTML.
/// Здесь мы используем готовую функцию `parse_jobs` и статический HTML.
pub struct HhScraper;

impl Scraper for HhScraper {
    fn name(&self) -> &str {
        "hh.ru"
    }

    fn fetch_jobs(&self) -> Vec<Job> {
        // Статический HTML — имитация ответа от hh.ru.
        let html = r#"
            <div class="job-card">
                <div class="title">Junior Rust Developer</div>
                <div class="company">Acme Corp</div>
                <div class="tech">Rust, Tokio, SQL</div>
            </div>
        "#;

        // Переиспользуем уже существующую функцию парсера.
        parse_jobs(html)
    }
}

/// Простейший Telegram-notifier.
///
/// Вместо реального HTTP-запроса к Telegram Bot API мы просто печатаем в stdout.
pub struct TelegramNotifier;

impl Notifier for TelegramNotifier {
    fn send(&self, job: &Job) {
        println!("[Telegram] New job: {} at {}", job.title, job.company);
    }
}

/// In-memory хранилище вакансий.
///
/// Полезно для тестов и примеров: не требует ни файлов, ни базы данных.
#[derive(Default)]
pub struct InMemoryStorage {
    jobs: Vec<Job>,
}

impl InMemoryStorage {
    /// Явный конструктор-обёртка над `Default`.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Storage for InMemoryStorage {
    fn save_jobs(&mut self, jobs: &[Job]) {
        // Добавляем копии вакансий в in-memory хранилище.
        //
        // Здесь мы осознанно клонируем `Job`, потому что хранилище должно
        // владеть своими данными независимо от вызывающего кода.
        self.jobs.extend(jobs.iter().cloned());
    }

    fn load_jobs(&self) -> Vec<Job> {
        // Возвращаем копию данных — внешнему коду нельзя дать прямой доступ
        // к внутреннему вектору, чтобы не нарушить инварианты хранилища.
        self.jobs.clone()
    }
}

/// Ещё один пример фильтра: по ключевым словам в заголовке вакансии.
///
/// В комбинации с `JobFilter` можно строить цепочки фильтров
/// (например, через отдельный `AndFilter<F1, F2>`).
#[allow(dead_code)] // заглушка: будет использоваться в более сложных сценариях фильтрации
pub struct KeywordFilter {
    pub keywords: Vec<String>,
}

impl Filter for KeywordFilter {
    fn matches(&self, job: &Job) -> bool {
        // Вакансия подходит, если заголовок содержит хотя бы одно ключевое слово.
        self.keywords.iter().any(|kw| {
            job.title
                .to_lowercase()
                .contains(&kw.to_lowercase())
        })
    }
}

/// Композитный фильтр "логическое И" двух других фильтров.
///
/// Это демонстрация того, что trait-архитектура позволяет легко строить
/// новые комбинации поведения без изменения кода scheduler'а.
#[allow(dead_code)] // заглушка: понадобится, когда добавим композицию фильтров
pub struct AndFilter<F1, F2> {
    pub first: F1,
    pub second: F2,
}

impl<F1, F2> Filter for AndFilter<F1, F2>
where
    F1: Filter,
    F2: Filter,
{
    fn matches(&self, job: &Job) -> bool {
        self.first.matches(job) && self.second.matches(job)
    }
}


