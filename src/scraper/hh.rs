use async_trait::async_trait;
use scraper::{Html, Selector};
use crate::domain::{Job, JobGrade, SalaryRange, Url};
use crate::errors::ScraperError;
use crate::scraper::Scraper;
use chrono::Utc;

/// Скрейпер для HeadHunter (hh.ru)
pub struct HhScraper;

#[async_trait]
impl Scraper for HhScraper {
    async fn scrape(&self, url: &str) -> Result<Vec<Job>, ScraperError> {
        // В реальном приложении здесь был бы HTTP-запрос
        // Для демонстрации используем статический HTML
        let html = r#"
            <div class="vacancy-card">
                <a class="vacancy-card__title" href="/vacancy/123">
                    Junior Rust Developer
                </a>
                <div class="vacancy-card__company">
                    Яндекс
                </div>
                <div class="vacancy-card__salary">
                    150 000 – 200 000 ₽
                </div>
                <div class="vacancy-card__skills">
                    <span>Rust</span>
                    <span>Tokio</span>
                    <span>PostgreSQL</span>
                </div>
            </div>
            <div class="vacancy-card">
                <a class="vacancy-card__title" href="/vacancy/456">
                    Middle Backend Engineer
                </a>
                <div class="vacancy-card__company">
                    Сбер
                </div>
                <div class="vacancy-card__salary">
                    250 000 ₽
                </div>
                <div class="vacancy-card__skills">
                    <span>Go</span>
                    <span>Kubernetes</span>
                </div>
            </div>
        "#;

        self.parse_html(html, url)
    }

    fn name(&self) -> &str {
        "hh.ru"
    }
}

impl HhScraper {
    /// Парсит HTML и извлекает вакансии
    fn parse_html(&self, html: &str, base_url: &str) -> Result<Vec<Job>, ScraperError> {
        let document = Html::parse_document(html);
        
        let card_selector = Selector::parse("div.vacancy-card")
            .map_err(|e| ScraperError::Parse(format!("Invalid selector: {}", e)))?;
        
        let title_selector = Selector::parse("a.vacancy-card__title")
            .map_err(|e| ScraperError::Parse(format!("Invalid selector: {}", e)))?;
        
        let company_selector = Selector::parse("div.vacancy-card__company")
            .map_err(|e| ScraperError::Parse(format!("Invalid selector: {}", e)))?;
        
        let salary_selector = Selector::parse("div.vacancy-card__salary")
            .map_err(|e| ScraperError::Parse(format!("Invalid selector: {}", e)))?;
        
        let skills_selector = Selector::parse("div.vacancy-card__skills span")
            .map_err(|e| ScraperError::Parse(format!("Invalid selector: {}", e)))?;
        
        let mut jobs = Vec::new();
        
        for element in document.select(&card_selector) {
            let title = element
                .select(&title_selector)
                .next()
                .and_then(|el| el.text().next())
                .unwrap_or("Unknown")
                .trim()
                .to_string();
            
            let company = element
                .select(&company_selector)
                .next()
                .and_then(|el| el.text().next())
                .unwrap_or("Unknown")
                .trim()
                .to_string();
            
            let salary_text = element
                .select(&salary_selector)
                .next()
                .and_then(|el| el.text().next())
                .unwrap_or("")
                .trim();
            
            let salary = self.parse_salary(salary_text);
            
            let tech_stack: Vec<String> = element
                .select(&skills_selector)
                .map(|el| el.text().next().unwrap_or("").trim().to_string())
                .collect();
            
            let href = element
                .select(&title_selector)
                .next()
                .and_then(|el| el.value().attr("href"))
                .unwrap_or("#");
            
            let full_url = if href.starts_with("http") {
                href.to_string()
            } else {
                format!("{}{}", base_url.trim_end_matches('/'), href)
            };
            
            let grade = self.detect_grade(&title);
            
            let job = Job {
                id: format!("{}-{}", 
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
                    title.chars().take(10).collect::<String>()
                ),
                title,
                company,
                tech_stack,
                grade,
                url: Url(full_url),
                salary,
                seen_at: Utc::now(),
            };
            
            jobs.push(job);
        }
        
        Ok(jobs)
    }
    
    /// Парсит текст зарплаты в SalaryRange
    fn parse_salary(&self, salary_text: &str) -> Option<SalaryRange> {
        if salary_text.is_empty() {
            return None;
        }
        
        // Удаляем все символы кроме цифр и пробелов
        let clean_text = salary_text
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == ' ')
            .collect::<String>()
            .trim()
            .to_string();
        
        if clean_text.is_empty() {
            return None;
        }
        
        let parts: Vec<&str> = clean_text.split_whitespace().collect();
        if parts.len() == 1 {
            // Фиксированная зарплата
            parts[0].parse::<u64>().ok().map(SalaryRange::Fixed)
        } else if parts.len() >= 2 {
            // Диапазон зарплаты
            let min = parts[0].parse::<u64>().ok()?;
            let max = parts[1].parse::<u64>().ok()?;
            Some(SalaryRange::Range(min, max))
        } else {
            None
        }
    }
    
    /// Определяет грейд из названия вакансии
    fn detect_grade(&self, title: &str) -> Option<JobGrade> {
        let title_lower = title.to_lowercase();
        
        if title_lower.contains("intern") || title_lower.contains("стажер") {
            Some(JobGrade::Intern)
        } else if title_lower.contains("junior") || title_lower.contains("младший") {
            Some(JobGrade::Junior)
        } else if title_lower.contains("middle") || title_lower.contains("middle") {
            Some(JobGrade::Middle)
        } else if title_lower.contains("senior") || title_lower.contains("старший") {
            Some(JobGrade::Senior)
        } else if title_lower.contains("lead") || title_lower.contains("ведущий") {
            Some(JobGrade::Lead)
        } else {
            None
        }
    }
}
