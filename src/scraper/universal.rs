use async_trait::async_trait;
use scraper::{Html, Selector};
use regex::Regex;

use crate::domain::{Job, Url};
use crate::errors::ScraperError;
use crate::scraper::Scraper;

pub struct UniversalScraper {
    pub keywords: Vec<String>,
    user_agent: Option<String>,
    client: reqwest::Client,
    companies: std::collections::HashMap<String, String>,
}

impl UniversalScraper {
    pub fn new(
        keywords: Vec<String>,
        user_agent: Option<String>,
        companies: std::collections::HashMap<String, String>,
    ) -> Self {
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 10 {
                    attempt.error("too many redirects")
                } else {
                    attempt.follow()
                }
            }));
        if let Some(ref ua) = user_agent {
            builder = builder.user_agent(ua.clone());
        }
        let client = builder.build().unwrap_or_default();
        Self { keywords, user_agent, client, companies }
    }

    pub fn keywords_json(&self) -> String {        let items: Vec<String> = self.keywords.iter()
            .map(|k| format!("\"{}\"", k.replace('"', "\\\"")))
            .collect();
        format!("[{}]", items.join(","))
    }

    pub fn company_for_url(&self, url: &str) -> String {
        let host = Self::extract_host_pub(url);
        if let Some(name) = self.companies.get(&host) {
            return name.clone();
        }
        for (domain, name) in &self.companies {
            if host.ends_with(domain.as_str()) {
                return name.clone();
            }
        }
        // Fallback: второй уровень домена (например "kaspersky" из "careers.kaspersky.ru")
        let parts: Vec<&str> = host.split('.').collect();
        if parts.len() >= 2 {
            let mut s = parts[parts.len() - 2].to_string();
            if let Some(c) = s.get_mut(0..1) { c.make_ascii_uppercase(); }
            s
        } else {
            host
        }
    }

    fn extract_jobs(&self, html: &str, base_url: &str) -> Vec<Job> {
        self.extract_jobs_from_html(html, base_url)
    }

    /// Извлекает вакансии из HTML без привязки к URL-паттернам.
    ///
    /// Алгоритм:
    /// 1. Находим все "листовые" элементы с коротким текстом (потенциальные заголовки)
    /// 2. Проверяем содержат ли они ключевое слово
    /// 3. Ищем ближайшую ссылку: сам элемент → вложенная → родительский контейнер
    /// 4. Принимаем любую ссылку на тот же домен (не только /vacancy/)
    pub fn extract_jobs_from_html(&self, html: &str, base_url: &str) -> Vec<Job> {
        let document = Html::parse_document(html);

        let kw_patterns: Vec<Regex> = self.keywords.iter()
            .filter_map(|kw| Regex::new(&format!(r"(?i)\b{}\b", regex::escape(kw))).ok())
            .collect();

        let base_host = Self::extract_host(base_url);

        let a_sel = Selector::parse("a[href]").unwrap();
        // Листовые элементы — те что обычно содержат заголовок вакансии
        let leaf_sel = Selector::parse(
            "h1, h2, h3, h4, h5, h6, nobr, b, strong, p, span, a[href], li, td"
        ).unwrap();
        // Контейнеры — для поиска ссылки "вверх" по дереву
        let container_sel = Selector::parse("div, section, article, li, td, tr").unwrap();

        // Приоритет тега: меньше = лучше
        fn tag_priority(tag: &str) -> u8 {
            match tag {
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => 0,
                "nobr" | "b" | "strong" => 1,
                "p" | "span" | "li" | "td" => 2,
                "a" => 3,
                _ => 4,
            }
        }

        // url -> (title, tag_priority)
        let mut result: std::collections::HashMap<String, (String, u8)> = std::collections::HashMap::new();

        for elem in document.select(&leaf_sel) {
            let tag = elem.value().name();

            // Берём прямой текст узла (без потомков) — чистый заголовок
            let direct: String = elem.children()
                .filter_map(|n| n.value().as_text())
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ");

            // Если прямой текст пустой — берём полный, но только короткий
            let full: String = elem.text()
                .map(|t| t.trim())
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" ");

            let text = if direct.len() >= 3 {
                direct
            } else if full.len() >= 3 && full.len() <= 100 {
                full
            } else {
                continue;
            };

            if text.starts_with("http") || text.contains('{') || text.contains('@') {
                continue;
            }

            let text_has_kw = kw_patterns.iter().any(|re| re.is_match(&text));
            if !text_has_kw {
                continue;
            }

            // Ищем ссылку: 1) сам элемент, 2) вложенная <a>, 3) ближайший контейнер с <a>
            let href: Option<String> = if tag == "a" {
                elem.value().attr("href").map(|h| h.to_string())
            } else {
                // Вложенная ссылка
                elem.select(&a_sel)
                    .find_map(|a| a.value().attr("href").map(|h| h.to_string()))
                    .or_else(|| {
                        // Ищем в ближайших контейнерах-предках через обход всего документа:
                        // находим контейнер, который содержит наш элемент и имеет ссылку
                        document.select(&container_sel).find_map(|container| {
                            // Контейнер должен содержать текст нашего элемента
                            let container_text: String = container.text().collect();
                            if !container_text.contains(&text) {
                                return None;
                            }
                            // Контейнер не должен быть слишком большим (весь body)
                            if container_text.len() > 500 {
                                return None;
                            }
                            container.select(&a_sel)
                                .find_map(|a| a.value().attr("href").map(|h| h.to_string()))
                        })
                    })
            };

            let href = match href {
                Some(h) if !h.is_empty() && !h.starts_with('#') && !h.starts_with("javascript") => h,
                _ => continue,
            };

            let url = Self::resolve_url(&href, base_url);
            let url_host = Self::extract_host(&url);

            // Только тот же домен
            if !base_host.is_empty() && !url_host.is_empty() && url_host != base_host {
                continue;
            }

            // Не берём ссылки на текущую страницу
            if url.trim_end_matches('/') == base_url.trim_end_matches('/') {
                continue;
            }

            let priority = tag_priority(tag);
            let entry = result.entry(url).or_insert_with(|| (text.clone(), priority));
            let prefer = priority < entry.1
                || (priority == entry.1 && text.len() > entry.0.len());
            if prefer {
                *entry = (text, priority);
            }
        }

        result.into_iter()
            .filter(|(_, (title, _))| title.len() >= 3)
            .map(|(url, (title, _))| {
                let company = self.company_for_url(&url);
                Job {
                    id: url.clone(),
                    title,
                    company,
                    url: Url(url),
                }
            })
            .collect()
    }

    fn resolve_url(href: &str, base_url: &str) -> String {
        if href.contains("://") {
            href.to_string()
        } else {
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

    fn extract_host(url: &str) -> String {
        Self::extract_host_pub(url)
    }

    pub fn extract_host_pub(url: &str) -> String {
        if let Some(pos) = url.find("://") {
            let after_scheme = &url[pos + 3..];
            let end = after_scheme.find('/').unwrap_or(after_scheme.len());
            after_scheme[..end].to_string()
        } else {
            String::new()
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

        // Follow redirect manually if the client stopped (e.g. cross-origin __rr redirects)
        let final_url = response.url().to_string();
        let html = response
            .text()
            .await
            .map_err(|e| ScraperError::Network { url: url.to_string(), source: e })?;

        Ok(self.extract_jobs(&html, &final_url))
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
            <div><a href="/job/42">Rust Developer</a></div>
            <div><a href="/job/43">Python Developer</a></div>
        </body></html>"#;
        let jobs = scraper.extract_jobs(html, "https://example.com");
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].title.to_lowercase().contains("rust"));
        assert!(jobs[0].url.0.starts_with("https://example.com"));
    }

    // 10.6 Property P1: фильтрация элементов по ключевым словам
    // Feature: job-notifier-enhanced, Property 1: все Job происходят из элементов, содержащих хотя бы одно ключевое слово
    proptest! {
        #[test]
        fn prop_p1_filter_divs_by_keywords(
            keyword in "[a-z]{4,8}",
        ) {
            // элемент с ключевым словом как отдельное слово — должен совпасть
            let html_match = format!(
                r#"<html><body><h3><a href="/job/1">{} developer</a></h3></body></html>"#,
                keyword
            );
            // элемент где keyword является подстрокой другого слова — не должен совпасть
            let non_match_word = format!("{}xyz", keyword);
            let html_no_match = format!(
                r#"<html><body><h3><a href="/job/2">{}</a></h3></body></html>"#,
                non_match_word
            );

            let scraper = make_scraper(vec![&keyword]);

            let jobs_match = scraper.extract_jobs(&html_match, "https://example.com");
            prop_assert!(!jobs_match.is_empty(), "keyword '{}' as whole word should match", keyword);

            let jobs_no_match = scraper.extract_jobs(&html_no_match, "https://example.com");
            prop_assert!(jobs_no_match.is_empty(), "keyword '{}' as substring of '{}' should NOT match", keyword, non_match_word);
        }
    }

    // 10.7 Property P2: извлечение полей из совпадающего элемента
    // Feature: job-notifier-enhanced, Property 2: извлечённый Job имеет непустой title и url начинающийся с http
    proptest! {
        #[test]
        fn prop_p2_extracted_job_has_title_and_url(
            keyword in "[a-z]{3,8}",
            title_prefix in "[A-Za-z ]{1,10}",
            path in "/job/[a-z]{1,10}",
        ) {
            // ключевое слово в тексте span, ссылка рядом
            let html = format!(
                r#"<html><body><div><span>{} {}</span><a href="{}">Apply</a></div></body></html>"#,
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
