// Template module: data types shared across the engine + schema-driven Template.

use serde::{Deserialize, Serialize};

pub mod built_in;
pub mod schema;

pub use schema::Template;

/// Aggregate metadata for one novel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NovelInfo {
    pub title: String,
    #[serde(default)]
    pub author: String,
    pub introduction: String,
    pub tags: Vec<String>,
    pub chapters: Vec<Chapter>,
}

/// One chapter in the index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub title: String,
    pub url: String,
    pub index: usize,
    /// From index `chapters.item.paid_gate` extractor (non-empty) or index gates.
    #[serde(default)]
    pub requires_login: bool,
}

/// Fetched chapter body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterContent {
    pub title: String,
    pub content: String,
}
