use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("文件不存在: {0}")]
    FileNotFound(String),

    #[error("JSON 解析失敗: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("IO 錯誤: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum ScrapeError {
    #[error("選擇器解析失敗: {0}")]
    SelectorError(String),

    #[error("JSON 解析失敗: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("無法從頁面抽取資料: {0}")]
    ExtractError(String),

    #[error("URL 無效: {0}")]
    InvalidUrl(String),

    #[error("IO 錯誤: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub type ScrapeResult<T> = std::result::Result<T, ScrapeError>;
