// Парсер HTML-страницы с вакансиями.
//
// В реальном проекте здесь используются HTTP-клиент и HTML-парсер (`reqwest` + `scraper`).
// В этой фазе нам важно показать:
//  - как `scraper` работает через CSS-селекторы;
//  - почему `Selector::parse()` вызываем один раз, а не в цикле;
//  - где именно создаются `String` и почему это оправдано с точки зрения ownership.

use crate::domain::{Job, Url};
use chrono::Utc;
use scraper::{Html, Selector};

/// Парсит HTML-страницу hh.ru и возвращает список вакансий.
///
/// Сигнатура подчёркивает владение и borrowing:
///  - `html: &str` — заимствуем уже загруженный HTTP-ответ, не создавая новый `String`;
///  - `Vec<Job>` — возвращаем вектор структур, которые полностью владеют своими данными.
pub fn parse_jobs(html: &str) -> Vec<Job> {
    // 1. Парсим всю страницу в DOM-структуру `Html`.
    //
    // Тип `Html` владеет буфером исходного HTML (обычно это `String` внутри),
    // но мы передаём только `&str`, поэтому никакого лишнего копирования нет.
    let document = Html::parse_document(html);

    // 2. Готовим CSS-селекторы ОДИН РАЗ, до цикла.
    //
    // Парсинг селектора — достаточно дорогая операция, поэтому её нельзя делать
    // внутри каждой итерации по карточкам вакансий.
    let card_sel =
        Selector::parse("div.vacancy-card").expect("valid selector for vacancy card");
    let title_sel =
        Selector::parse("a.vacancy-card__title").expect("valid selector for vacancy title");

    let mut jobs = Vec::new();

    // 3. Итерируемся по карточкам вакансий.
    for card in document.select(&card_sel) {
        // Ищем элемент заголовка внутри карточки.
        let title_el = match card.select(&title_sel).next() {
            Some(el) => el,
            // Если по какой-то причине внутри карточки нет заголовка — пропускаем её.
            None => continue,
        };

        // Собираем весь текст заголовка.
        //
        // `title_el.text()` возвращает итератор по фрагментам `&str`.
        // `collect::<String>()` создаёт НОВЫЙ `String` и конкатенирует все фрагменты.
        // Это как раз тот момент, где копия строки неизбежна и оправдана:
        // нам нужен цельный owned-текст, который будет жить дольше, чем DOM.
        let raw_title = title_el.text().collect::<String>();
        let title = raw_title.trim().to_string();

        // Достаём атрибут `href` из тега `<a>`.
        //
        // `value()` даёт доступ к структуре тега, `attr("href")` возвращает `Option<&str>`.
        let href = match title_el.value().attr("href") {
            Some(h) => h,
            // Если нет ссылки — карточка для нас бесполезна.
            None => continue,
        };

        // В реальном hh.ru ссылки могут быть относительными (`/vacancy/123`),
        // поэтому добавим простую нормализацию: если нет схемы, приклеиваем базовый URL.
        let url_str = if href.starts_with("http://") || href.starts_with("https://") {
            href.to_string()
        } else {
            format!("https://hh.ru{}", href)
        };

        // Пока у нас нет реального парсинга компании/зарплаты — заполним минимальные поля.
        let company = "Unknown".to_string();
        let tech_stack = Vec::new();

        // Уникальный идентификатор вакансии: хеш `(title + company)`.
        let id = make_job_id(&title, &company);

        let job = Job {
            id,
            title,
            company,
            tech_stack,
            grade: None,
            url: Url::new(url_str),
            salary: None,
            // Фиксируем момент, когда мы "увидели" вакансию.
            seen_at: Utc::now(),
        };

        jobs.push(job);
    }

    // Возвращаем `jobs` по move — вызывающая сторона теперь полностью владеет результатом.
    jobs
}

/// Строит детерминированный идентификатор вакансии на основе `(title, company)`.
///
/// Это лёгкий способ сделать уникальный ключ без зависимости от UUID:
///  - если title+company совпадают, то и `id` совпадает;
///  - если хотя бы одно поле отличается — хеш почти наверняка другой.
fn make_job_id(title: &str, company: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    title.hash(&mut hasher);
    company.hash(&mut hasher);
    let hash = hasher.finish();

    // Представляем u64 как шестнадцатеричную строку — компактно и читаемо.
    format!("{:016x}", hash)
}

/// Удаляет дубликаты вакансий.
///
/// Сигнатура подчёркивает владение:
///  - `jobs: Vec<Job>` — функция *забирает* вектор целиком (move).
///  - Возвращаем новый `Vec<Job>`, где все элементы — те же самые `Job`, но без дублей.
pub fn dedup(jobs: Vec<Job>) -> Vec<Job> {
    use std::collections::HashSet;

    // Мы создаём новый вектор и множества для проверки уникальности.
    // Старый `jobs` мы не можем больше использовать (он move'нут в эту функцию).
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    // `into_iter()` *перемещает* каждый `Job` из исходного вектора.
    // После этого элементы принадлежат не вектору `jobs`, а переменной `job` в цикле.
    for job in jobs.into_iter() {
        // Чтобы проверить, встречалась ли вакансия, мы используем кортеж ключевых полей.
        //
        // Важно: мы создаём *владейщую* копию ключа через `String::clone()`.
        // Это нужен, чтобы `HashSet` мог хранить ключ дольше, чем живёт `job`,
        // который мы затем перемещаем в `unique`.
        //
        // Такая копия — осознанный обмен: небольшие аллокации памяти ради простой,
        // безопасной логики dedup. В реальном проекте можно оптимизировать стратегию
        // ключей под конкретные требования.
        let key = (job.title.clone(), job.company.clone());

        if seen.insert(key) {
            // `job` move'ится в новый вектор `unique`.
            // Строки внутри `job` *не* клонируются здесь, мы просто переносим владение.
            unique.push(job);
        }
        // Если `insert` вернул false, job просто дропается, и память освобождается.
    }

    unique
}


