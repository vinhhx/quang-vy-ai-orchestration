//! Logic fixbug: lấy bug từ Trello / Google Sheet vào hàng chờ, ghi ngược Trello (Plan fixbug-sources-engine).

pub mod config;
pub mod envfile;
pub mod gsheet;
pub mod pull;
pub mod trello;

use crate::error::{BrainError, EXIT_ERROR};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixbugError {
    NotConfigured(String),
    Config(String),
    Trello { status: u16, message: String },
    Sheet(String),
    Io(String),
    Network(String),
    Other(String),
}

impl std::fmt::Display for FixbugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotConfigured(msg) => write!(f, "Chưa cấu hình fixbug: {msg}"),
            Self::Config(msg) => write!(f, "Lỗi cấu hình fixbug: {msg}"),
            Self::Trello { status, message } => write!(f, "Lỗi Trello ({status}): {message}"),
            Self::Sheet(msg) => write!(f, "Lỗi Google Sheet: {msg}"),
            Self::Io(msg) => write!(f, "Lỗi I/O: {msg}"),
            Self::Network(msg) => write!(f, "Lỗi mạng: {msg}"),
            Self::Other(msg) => write!(f, "Lỗi: {msg}"),
        }
    }
}

impl std::error::Error for FixbugError {}

impl From<FixbugError> for BrainError {
    fn from(e: FixbugError) -> Self {
        BrainError::new(e.to_string(), EXIT_ERROR)
    }
}

impl From<std::io::Error> for FixbugError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
