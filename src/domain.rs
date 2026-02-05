// Доменные типы проекта: вакансии, фильтры и конфиг парсера.
// Здесь особенно хорошо видны решения по владению (String) и заимствованиям (&str).

/// Описание вакансии.
/// 
/// Обратите внимание: все "долгоживущие" строковые поля — это `String`.
/// `Job` должен переживать момент парсинга HTML, поэтому он не может хранить `&str` на буфер HTML.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Job {
    // `String` владеет данными. Это значит, что когда `Job` будет перемещён (move),
    // вместе с ним переместятся и строки, без дополнительных копий.
    pub title: String,
    pub company: String,
    // Технологический стек — вектор строк. Каждый элемент — owned `String`.
    // При `push` будет move строки в вектор.
    pub tech_stack: Vec<String>,
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

impl JobFilter {
    /// Проверяет, подходит ли вакансия под критерии фильтра.
    ///
    /// Обратите внимание на сигнатуру: `&self` и `&Job`.
    /// Функция лишь читает данные и ничего не забирает во владение.
    pub fn matches(&self, job: &Job) -> bool {
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


