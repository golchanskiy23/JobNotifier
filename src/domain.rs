// Доменные типы проекта: вакансии, фильтры и конфиг парсера.
// Здесь особенно хорошо видны решения по владению (String) и заимствованиям (&str).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Обёртка над строкой-URL.
///
/// Почему не `String` напрямую:
///  - типовая безопасность: компилятор отличает `Url` от "просто строки";
///  - в будущем сюда легко добавить валидацию/парсинг без изменения сигнатур.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Url(pub String);

impl Url {
    /// Конструктор, явно показывающий намерение: мы создаём URL, а не любую строку.
    pub fn new(value: String) -> Self {
        Self(value)
    }
}

/// Диапазон зарплаты.
///
/// Типобезопасное представление вместо "строки с текстом":
///  - `Fixed(120_000)` — фиксированная ставка;
///  - `Range(100_000, 150_000)` — "от/до";
///  - в будущем можно добавить `Unknown` / `Negotiable` и т.д.
#[allow(dead_code)] // заглушка: пока не конструируем SalaryRange, но фиксируем API и семантику
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SalaryRange {
    Fixed(u64),
    Range(u64, u64),
}

/// Описание вакансии.
///
/// Обратите внимание:
///  - все "долгоживущие" строковые поля — это `String`;
///  - `Job` должен переживать момент парсинга HTML, поэтому он не может хранить `&str` на буфер HTML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Job {
    /// Уникальный идентификатор вакансии.
    ///
    /// В проекте его можно получить как хеш от `(title + company)` — этого достаточно
    /// для дедупликации без отдельной зависимости на UUID.
    #[serde(rename = "jobId")]
    pub id: String,
    /// Заголовок вакансии, например "Junior Rust Developer".
    #[serde(rename = "jobTitle")]
    pub title: String,
    /// Компания-работодатель.
    pub company: String,
    /// Технологический стек — вектор owned-строк.
    /// При `push` сюда мы передаём `String` по move, без лишних копий.
    pub tech_stack: Vec<String>,
    /// Грейд (Junior/Middle/Senior/Lead). Может отсутствовать, если не указан явно.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade: Option<JobGrade>,
    /// URL страницы вакансии (обёртка над строкой).
    pub url: Url,
    /// Диапазон зарплаты, если он указан.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salary: Option<SalaryRange>,
    /// Время, когда вакансия была "увидена" парсером.
    /// Храним как `DateTime<Utc>`, чтобы можно было сортировать и сравнивать.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub seen_at: DateTime<Utc>,
}

/// Грейд (уровень) вакансии.
///
/// Этот enum используется там, где мы хотим типобезопасно оперировать грейдом:
/// `match grade { JobGrade::Junior => ..., JobGrade::Senior => ... }`.
/// Если позже добавить новый вариант (например, `Principal`), компилятор потребует
/// обновить все `match` — это защищает от "забытых" кейсов.
#[allow(dead_code)] // заглушка: enum пока не используется, но описывает грейд для будущей логики
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobGrade {
    Junior,
    Middle,
    Senior,
    Lead,
}

/// Trait-фильтра по вакансиям.
///
/// Благодаря этому trait'у можно комбинировать разные реализации фильтра:
///  - `JobFilter` для грейда/технологий/компании;
///  - отдельный `KeywordFilter` по ключевым словам;
///  - композиции фильтров вида `AndFilter<A, B>`.
pub trait Filter {
    /// Возвращает `true`, если вакансия удовлетворяет условиям фильтра.
    fn matches(&self, job: &Job) -> bool;
}

/// Фильтр по вакансиям.
/// 
/// Здесь мы также используем `String`, а не `&str`, потому что фильтр
/// может жить долго (например, весь запуск программы) и не должен зависеть
/// от конкретного HTML-ответа.
#[derive(Debug, Clone)]
pub struct JobFilter {
    // Простейший пример: минимальный грейд, например "junior", "middle", "senior".
    // `Option<String>`: фильтр может быть задан или отсутствовать.
    pub min_grade: Option<String>,
    // Список технологий, которые обязательно должны присутствовать.
    pub required_tech: Vec<String>,
    // Фильтр по компании: если `Some`, оставляем только вакансии этой компании.
    pub company: Option<String>,
}

impl Job {
    /// Строит ключ для дедупликации на основе сериализации `Job` в JSON.
    ///
    /// Стратегия из описания:
    ///  - `Job` → `serde_json::to_string()` → `hash` → строковый ключ;
    ///  - если структура `Job` изменится, хеш тоже изменится, но это приемлемо
    ///    для кеша `seen_jobs` (он просто будет перепостроен).
    pub fn dedup_key(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // В нормальной ситуации сериализация не должна падать, потому что все поля
        // поддерживают `Serialize`. На всякий случай, при ошибке используем title
        // как базу для хеша, чтобы не паниковать в прод-пути.
        let json = serde_json::to_string(self).unwrap_or_else(|_| self.title.clone());

        let mut hasher = DefaultHasher::new();
        json.hash(&mut hasher);
        let hash = hasher.finish();
        format!("{:016x}", hash)
    }
}

impl JobFilter {
    /// Проверяет, подходит ли вакансия под критерии фильтра.
    ///
    /// Обратите внимание на сигнатуру: `&self` и `&Job`.
    /// Функция лишь читает данные и ничего не забирает во владение.
    ///
    /// Это "базовая" реализация логики фильтрации, на которую будет
    /// опираться trait `Filter`.
    pub fn matches_job(&self, job: &Job) -> bool {
        // Если задана минимальная "ступень" (грейд), делаем очень примитивную проверку:
        // смотрим, содержит ли заголовок вакансии это слово.
        if let Some(ref grade) = self.min_grade {
            // Здесь мы берём `&String` (grade) и сравниваем с `&str`, полученным из `job.title`.
            // Метод `contains` использует borrowing, не создавая новых строк.
            if !job.title.to_lowercase().contains(&grade.to_lowercase()) {
                return false;
            }
        }

        // Проверяем, что все требуемые технологии присутствуют в `job.tech_stack`.
        for required in &self.required_tech {
            // `iter()` даёт `&String`, `any` замыкает по ссылке `&String`,
            // вся проверка работает только на заимствованиях.
            let has_tech = job
                .tech_stack
                .iter()
                .any(|t| t.eq_ignore_ascii_case(required));

            if !has_tech {
                return false;
            }
        }

        // Если фильтр по компании задан, сравниваем с полем `company`.
        if let Some(ref company) = self.company {
            if !job.company.eq_ignore_ascii_case(company) {
                return false;
            }
        }

        true
    }
}

/// Реализация обобщённого trait'а `Filter` для нашего конкретного `JobFilter`.
///
/// Теперь любой код, который работает через `dyn Filter`, может прозрачно
/// использовать `JobFilter` как одну из реализаций.
impl Filter for JobFilter {
    fn matches(&self, job: &Job) -> bool {
        self.matches_job(job)
    }
}

/// Конфигурация парсера, демонстрирующая lifetimes.
///
/// Здесь мы намеренно используем `&'a str`, потому что:
///  - URL обычно задаётся как строковый литерал (`"https://..."`), ему можно дать `'static`.
///  - Конфиг парсера не должен владеть строкой, он лишь "ссылается" на неё.
pub struct ScraperConfig<'a> {
    // Lifetime `'a` означает: `ScraperConfig<'a>` не может жить дольше, чем сама строка `url`.
    pub url: &'a str,
    // CSS‑селектор мы тоже храним как `&'a str`, потому что он почти всегда либо литерал,
    // либо строка, чей владелец живёт дольше конфига.
    pub job_card_selector: &'a str,
}



