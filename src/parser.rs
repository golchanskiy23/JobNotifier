use crate::domain::{Job, Url};
use chrono::Utc;
use scraper::{Html, Selector};

pub fn parse_jobs(html: &str) -> Vec<Job> {
    let document = Html::parse_document(html);

    let card_sel =
        Selector::parse("div.vacancy-card").expect("valid selector for vacancy card");
    let title_sel =
        Selector::parse("a.vacancy-card__title").expect("valid selector for vacancy title");

    let mut jobs = Vec::new();

    for card in document.select(&card_sel) {
        let title_el = match card.select(&title_sel).next() {
            Some(el) => el,
            None => continue,
        };

        let raw_title = title_el.text().collect::<String>();
        let title = raw_title.trim().to_string();

        let href = match title_el.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        let url_str = if href.starts_with("http://") || href.starts_with("https://") {
            href.to_string()
        } else {
            format!("https://hh.ru{}", href)
        };

        let company = "Unknown".to_string();
        let tech_stack = Vec::new();

        let id = make_job_id(&title, &company);

        let job = Job {
            id,
            title,
            company,
            tech_stack,
            grade: None,
            url: Url::new(url_str),
            salary: None,
            seen_at: Utc::now(),
        };

        jobs.push(job);
    }

    jobs
}

fn make_job_id(title: &str, company: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    title.hash(&mut hasher);
    company.hash(&mut hasher);
    let hash = hasher.finish();

    format!("{:016x}", hash)
}

pub fn dedup(jobs: Vec<Job>) -> Vec<Job> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for job in jobs.into_iter() {
        let key = (job.title.clone(), job.company.clone());

        if seen.insert(key) {
            unique.push(job);
        }
    }

    unique
}


