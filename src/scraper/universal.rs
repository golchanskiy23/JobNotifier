use async_trait::async_trait;
use scraper::{Html, Selector};
use chrono::Utc;
use regex::Regex;

use crate::domain::{Job, Url};
use crate::errors::ScraperError;
use crate::scraper::Scraper;
use crate::scraper::grade::detect_grade;

pub struct UniversalScraper {
    keywords: Vec<String>,
    user_agent: Option<String>,
    client: reqwest::Client,
}

impl UniversalScraper {
    pub fn new(keywords: Vec<String>, user_agent: Option<String>) -> Self {
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(10));
        if let Some(ref ua) = user_agent {
            builder = builder.user_agent(ua.clone());
        }
        let client = builder.build().unwrap_or_default();
        Self { keywords, user_agent, client }
    }

    fn extract_jobs(&self, html: &str, base_url: &str) -> Vec<Job> {
        let document = Html::parse_document(html);
        let div_selector = match Selector::parse("div") {
            Ok(s) => s,
            Err(_) => return vec![],
        };
        let a_selector = match Selector::parse("a[href]") {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        let mut jobs = Vec::new();

        for element in document.select(&div_selector) {
            let text: String = element.text().collect();

            let matches = self.keywords.iter().any(|kw| {
                let pattern = format!(r"(?i)\b{}\b", regex::escape(kw));
                Regex::new(&pattern)
                    .map(|re| re.is_match(&text))
                    .unwrap_or(false)
            });
            if !matches {
                continue;
            }

            // Первый <a href> как URL вакансии
            let href = match element.select(&a_selector).next()
                .and_then(|a| a.value().attr("href"))
            {
                Some(h) => h,
                None => continue,
            };

            let url = Self::resolve_url(href, base_url);

            // Первая непустая текстовая строка как заголовок
            let title = element.text()
                .map(|t| t.trim().to_string())
                .find(|t| !t.is_empty())
                .unwrap_or_default();

            if title.is_empty() {
                continue;
            }

            let grade = detect_grade(&title);

            let job = Job {
                id: format!(
                    "{}-{}",
                    Utc::now().timestamp_nanos_opt().unwrap_or(0),
                    title.chars().take(10).collect::<String>()
                ),
                title,
                company: String::new(),
                tech_stack: vec![],
                grade,
                url: Url(url),
                salary: None,
                seen_at: Utc::now(),
            };

            jobs.push(job);
        }

        jobs
    }

    fn resolve_url(href: &str, base_url: &str) -> String {
        if href.contains("://") {
            href.to_string()
        } else {
            // Извлекаем origin из base_url
            let origin = if let Some(pos) = base_url.find("://") {
                let after_scheme = &base_url[pos + 3..];
                let end = after_scheme.find('/').unwrap_or(after_scheme.len());
                &base_url[..pos + 3 + end]
            } else {
                base_url.trim_end_matches('/')
            };
            format!("{}{}", origin, if href.starts_with('/') { href.to_string() } else { format!("/{}", href) })
        }
    }
}

#[async_trait]
impl Scraper for UniversalScraper {
    async fn scrape(&self, url: &str) -> Result<Vec<Job>, ScraperError> {
        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| ScraperError::Network { url: url.to_string(), source: e })?;

        let html = response
            .text()
            .await
            .map_err(|e| ScraperError::Network { url: url.to_string(), source: e })?;

        Ok(self.extract_jobs(&html, url))
    }

    fn name(&self) -> &str {
        "universal"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_scraper(keywords: Vec<&str>) -> UniversalScraper {
        UniversalScraper::new(keywords.into_iter().map(|s| s.to_string()).collect(), None)
    }

    // 10.1 Unit-тесты для extract_jobs

    #[test]
    fn test_extract_jobs_empty_html() {
        let scraper = make_scraper(vec!["rust"]);
        let jobs = scraper.extract_jobs("", "https://example.com");
        assert!(jobs.is_empty());
    }

    #[test]
    fn test_extract_jobs_no_matches() {
        let scraper = make_scraper(vec!["rust"]);
        let html = r#"<html><body><div><a href="/job/1">Python Developer</a></div></body></html>"#;
        let jobs = scraper.extract_jobs(html, "https://example.com");
        assert!(jobs.is_empty());
    }

    #[test]
    fn test_extract_jobs_with_matches() {
        let scraper = make_scraper(vec!["rust"]);
        let html = r#"<html><body>
            <div>Rust Developer <a href="/job/42">Apply</a></div>
            <div>Python Developer <a href="/job/43">Apply</a></div>
        </body></html>"#;
        let jobs = scraper.extract_jobs(html, "https://example.com");
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].title.to_lowercase().contains("rust"));
        assert!(jobs[0].url.0.starts_with("https://example.com"));
    }

    // 10.6 Property P1: фильтрация div по ключевым словам
    // Feature: job-notifier-enhanced, Property 1: все Job происходят из div-элементов, содержащих хотя бы одно ключевое слово
    proptest! {
        #[test]
        fn prop_p1_filter_divs_by_keywords(
            keyword in "[a-z]{4,8}",
        ) {
            // div с ключевым словом как отдельное слово — должен совпасть
            let html_match = format!(
                r#"<html><body><div>{} developer <a href="/job/1">Link</a></div></body></html>"#,
                keyword
            );
            // div где keyword является подстрокой другого слова — не должен совпасть
            let non_match_word = format!("{}xyz", keyword);
            let html_no_match = format!(
                r#"<html><body><div>{} <a href="/job/2">Link</a></div></body></html>"#,
                non_match_word
            );

            let scraper = make_scraper(vec![&keyword]);

            let jobs_match = scraper.extract_jobs(&html_match, "https://example.com");
            prop_assert!(!jobs_match.is_empty(), "keyword '{}' as whole word should match", keyword);

            let jobs_no_match = scraper.extract_jobs(&html_no_match, "https://example.com");
            prop_assert!(jobs_no_match.is_empty(), "keyword '{}' as substring of '{}' should NOT match", keyword, non_match_word);
        }
    }

    // 10.7 Property P2: извлечение полей из совпадающего div
    // Feature: job-notifier-enhanced, Property 2: извлечённый Job имеет непустой title и url начинающийся с http
    proptest! {
        #[test]
        fn prop_p2_extracted_job_has_title_and_url(
            keyword in "[a-z]{3,8}",
            title_prefix in "[A-Za-z ]{1,10}",
            path in "/[a-z]{1,10}",
        ) {
            let html = format!(
                r#"<html><body><div>{} {} <a href="{}">Link</a></div></body></html>"#,
                title_prefix, keyword, path
            );
            let scraper = make_scraper(vec![&keyword]);
            let jobs = scraper.extract_jobs(&html, "https://example.com");

            for job in &jobs {
                prop_assert!(!job.title.is_empty(), "title must not be empty");
                prop_assert!(job.url.0.starts_with("http"), "url must start with http, got: {}", job.url.0);
            }
        }
    }

    // 10.8 Property P3: преобразование относительного URL в абсолютный
    // Feature: job-notifier-enhanced, Property 3: resolve_url возвращает строку начинающуюся с origin базового URL
    proptest! {
        #[test]
        fn prop_p3_resolve_relative_url(
            scheme in "(https|http)",
            host in "[a-z]{3,10}\\.[a-z]{2,4}",
            path in "/[a-z]{1,10}",
            rel_path in "/[a-z]{1,10}",
        ) {
            let base_url = format!("{}://{}{}", scheme, host, path);
            let resolved = UniversalScraper::resolve_url(&rel_path, &base_url);
            let expected_origin = format!("{}://{}", scheme, host);
            prop_assert!(
                resolved.starts_with(&expected_origin),
                "resolved '{}' should start with origin '{}'", resolved, expected_origin
            );
        }
    }
}
