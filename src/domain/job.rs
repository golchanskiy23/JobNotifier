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
pub struct Job {
    pub id: String,
    pub title: String,
    pub company: String,
    pub url: Url,
}

impl Job {
    pub fn dedup_key(&self) -> String {
        self.url.0.clone()
    }
}

pub trait Filter: Send + Sync {
    fn matches(&self, job: &Job) -> bool;
}
