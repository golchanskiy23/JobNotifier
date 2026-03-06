use crate::domain::{Job, JobGrade, Filter};

/// Фильтр по уровню грейда
#[derive(Debug, Clone)]
pub struct GradeFilter {
    pub min_grade: Option<JobGrade>,
}

impl GradeFilter {
    pub fn new(min_grade: Option<JobGrade>) -> Self {
        Self { min_grade }
    }
}

impl Filter for GradeFilter {
    fn matches(&self, job: &Job) -> bool {
        if let Some(ref min_grade) = self.min_grade {
            if let Some(ref job_grade) = job.grade {
                // Сравниваем грейды по уровню
                self.grade_to_number(job_grade) >= self.grade_to_number(min_grade)
            } else {
                false // Если у вакансии нет грейда, не проходит фильтр
            }
        } else {
            true // Фильтр не задан, пропускаем все
        }
    }
}

impl GradeFilter {
    fn grade_to_number(&self, grade: &JobGrade) -> u8 {
        match grade {
            JobGrade::Intern => 0,
            JobGrade::Junior => 1,
            JobGrade::Middle => 2,
            JobGrade::Senior => 3,
            JobGrade::Lead => 4,
            JobGrade::Principal => 5,
            JobGrade::Staff => 6,
        }
    }
}

/// Фильтр по ключевым словам в названии
#[derive(Debug, Clone)]
pub struct KeywordFilter {
    pub keywords: Vec<String>,
    pub exclude: Vec<String>,
}

impl KeywordFilter {
    pub fn new(keywords: Vec<String>, exclude: Vec<String>) -> Self {
        Self { keywords, exclude }
    }
}

impl Filter for KeywordFilter {
    fn matches(&self, job: &Job) -> bool {
        let title_lower = job.title.to_lowercase();
        
        // Проверяем обязательные ключевые слова
        if !self.keywords.is_empty() {
            let has_required = self.keywords.iter().any(|keyword| {
                title_lower.contains(&keyword.to_lowercase())
            });
            if !has_required {
                return false;
            }
        }
        
        // Исключаем вакансии с нежелательными словами
        for exclude_word in &self.exclude {
            if title_lower.contains(&exclude_word.to_lowercase()) {
                return false;
            }
        }
        
        true
    }
}

/// Фильтр по компании
#[derive(Debug, Clone)]
pub struct CompanyFilter {
    pub companies: Vec<String>,
}

impl CompanyFilter {
    pub fn new(companies: Vec<String>) -> Self {
        Self { companies }
    }
}

impl Filter for CompanyFilter {
    fn matches(&self, job: &Job) -> bool {
        if self.companies.is_empty() {
            return true;
        }
        
        self.companies.iter().any(|company| {
            job.company.to_lowercase().contains(&company.to_lowercase())
        })
    }
}

/// Фильтр по технологическому стеку
#[derive(Debug, Clone)]
pub struct TechFilter {
    pub required_tech: Vec<String>,
    pub exclude_tech: Vec<String>,
}

impl TechFilter {
    pub fn new(required_tech: Vec<String>, exclude_tech: Vec<String>) -> Self {
        Self { required_tech, exclude_tech }
    }
}

impl Filter for TechFilter {
    fn matches(&self, job: &Job) -> bool {
        let job_tech: Vec<String> = job.tech_stack.iter()
            .map(|tech| tech.to_lowercase())
            .collect();
        
        // Проверяем обязательные технологии
        for required in &self.required_tech {
            if !job_tech.contains(&required.to_lowercase()) {
                return false;
            }
        }
        
        // Исключаем вакансии с нежелательными технологиями
        for exclude in &self.exclude_tech {
            if job_tech.contains(&exclude.to_lowercase()) {
                return false;
            }
        }
        
        true
    }
}

/// Композиционный фильтр "И" - все фильтры должны вернуть true
#[derive(Debug, Clone)]
pub struct AndFilter<F1, F2> {
    pub first: F1,
    pub second: F2,
}

impl<F1, F2> AndFilter<F1, F2> 
where 
    F1: Filter,
    F2: Filter,
{
    pub fn new(first: F1, second: F2) -> Self {
        Self { first, second }
    }
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

/// Композиционный фильтр "ИЛИ" - хотя бы один фильтр должен вернуть true
#[derive(Debug, Clone)]
pub struct OrFilter<F1, F2> {
    pub first: F1,
    pub second: F2,
}

impl<F1, F2> OrFilter<F1, F2>
where 
    F1: Filter,
    F2: Filter,
{
    pub fn new(first: F1, second: F2) -> Self {
        Self { first, second }
    }
}

impl<F1, F2> Filter for OrFilter<F1, F2>
where
    F1: Filter,
    F2: Filter,
{
    fn matches(&self, job: &Job) -> bool {
        self.first.matches(job) || self.second.matches(job)
    }
}
