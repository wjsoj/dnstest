//! Error types module.
//!
//! This module defines the error types used throughout the dnstest application.
//! It uses `thiserror` for structured error handling and provides
//! a custom `Result` type alias for convenience.

use thiserror::Error;

/// A specialized `Result` type for dnstest operations.
///
/// This type is used throughout the crate to handle errors consistently.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error enum for dnstest application.
///
/// Each variant represents a different category of error that can occur
/// during DNS testing operations.
#[derive(Debug, Error)]
pub enum Error {
    /// I/O error (file operations, network sockets, etc.)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error (configuration files, JSON output)
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// DNS resolver error (DNS query failures)
    #[error("DNS resolver error: {0}")]
    Resolver(#[from] trust_dns_resolver::error::ResolveError),

    /// Network-related error (connection failures, timeouts)
    #[error("Network error: {0}")]
    Network(String),

    /// Configuration error (invalid config, missing files)
    #[error("Config error: {0}")]
    Config(String),

    /// TUI (terminal UI) related error
    #[error("TUI error: {0}")]
    Tui(String),

    /// Parse error (invalid input format, malformed data)
    #[error("Parse error: {0}")]
    Parse(String),

    /// Operation timeout
    #[error("Operation timed out")]
    Timeout,
}

impl Error {
    /// Create a new network error with a message.
    #[must_use]
    pub fn network(msg: impl Into<String>) -> Self {
        Self::Network(msg.into())
    }

    /// Create a new configuration error with a message.
    #[must_use]
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a new parse error with a message.
    #[must_use]
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }

    /// Create a new TUI error with a message.
    #[must_use]
    pub fn tui(msg: impl Into<String>) -> Self {
        Self::Tui(msg.into())
    }
}

impl From<color_eyre::Report> for Error {
    fn from(e: color_eyre::Report) -> Self {
        Self::Config(e.to_string())
    }
}
