use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// URL newtype для безопасности типов
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Url(pub String);

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Уровень должности
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobGrade {
    Intern,
    Junior,
    Middle,
    Senior,
    Lead,
    Principal,
    Staff,
}

impl fmt::Display for JobGrade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobGrade::Intern => write!(f, "Intern"),
            JobGrade::Junior => write!(f, "Junior"),
            JobGrade::Middle => write!(f, "Middle"),
            JobGrade::Senior => write!(f, "Senior"),
            JobGrade::Lead => write!(f, "Lead"),
            JobGrade::Principal => write!(f, "Principal"),
            JobGrade::Staff => write!(f, "Staff"),
        }
    }
}

/// Диапазон зарплаты
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SalaryRange {
    Fixed(u64),
    Range(u64, u64),
}

impl fmt::Display for SalaryRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SalaryRange::Fixed(amount) => write!(f, "{} ₽", amount),
            SalaryRange::Range(min, max) => write!(f, "{} – {} ₽", min, max),
        }
    }
}

/// Основная структура вакансии
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Job {
    /// Уникальный идентификатор вакансии
    pub id: String,
    
    /// Название вакансии
    pub title: String,
    
    /// Название компании
    pub company: String,
    
    /// Технологический стек
    pub tech_stack: Vec<String>,
    
    /// Уровень должности
    pub grade: Option<JobGrade>,
    
    /// Ссылка на вакансию
    pub url: Url,
    
    /// Зарплата
    pub salary: Option<SalaryRange>,
    
    /// Когда вакансия была найдена
    pub seen_at: DateTime<Utc>,
}

impl Job {
    /// Создает ключ для дедупликации
    pub fn dedup_key(&self) -> String {
        format!("{}:{}:{}", self.company, self.title, self.url.0)
    }
    
    /// Проверяет, является ли вакансия релевантной для поиска
    pub fn is_relevant(&self) -> bool {
        // Базовые проверки релевантности
        !self.title.is_empty() 
            && !self.company.is_empty() 
            && self.url.0.starts_with("http")
    }
}

/// Фильтр вакансий
pub trait Filter: Send + Sync {
    /// Проверяет, соответствует ли вакансия условиям фильтра
    fn matches(&self, job: &Job) -> bool;
}
