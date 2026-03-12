use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Url(pub String);

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Job {
    pub id: String,
    pub title: String,
    pub company: String,
    pub tech_stack: Vec<String>,
    pub grade: Option<JobGrade>,
    pub url: Url,
    pub salary: Option<SalaryRange>,
    pub seen_at: DateTime<Utc>,
}

impl Job {
    pub fn dedup_key(&self) -> String {
        format!("{}:{}:{}", self.company, self.title, self.url.0)
    }
    
    pub fn is_relevant(&self) -> bool {
        !self.title.is_empty() 
            && !self.company.is_empty() 
            && self.url.0.starts_with("http")
    }
}

pub trait Filter: Send + Sync {
    fn matches(&self, job: &Job) -> bool;
}
