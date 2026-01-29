//! Error types for the HomeCtrl LED controller.
//!
//! This module provides comprehensive error handling for all operations
//! including network communication, hardware interaction, and state management.

use std::sync::PoisonError;

use actix_web::{http::StatusCode, ResponseError};

/// Result type alias for HomeCtrl operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during HomeCtrl operations.
#[derive(thiserror::Error, Debug, Clone)]
pub enum Error {
    /// LED strip with the specified ID was not found.
    #[error("LED strip {0} not found")]
    StripNotFound(u8),

    /// Requested animation mode does not exist.
    #[error("unknown animation mode: {0}")]
    UnknownMode(String),

    /// Operation requires debug mode to be enabled.
    #[error("debug mode is required for this operation")]
    DebugModeRequired,

    /// Network communication failure.
    #[error("network error: {0}")]
    Network(String),

    /// Failed to lock shared state - indicates thread panic.
    #[error("state lock poisoned")]
    LockPoisoned,

    /// Invalid configuration value.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Color parsing failed.
    #[error("invalid color format: {0}")]
    InvalidColor(String),

    /// JSON serialization/deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Network(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl<T> From<PoisonError<T>> for Error {
    fn from(_: PoisonError<T>) -> Self {
        Self::LockPoisoned
    }
}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::StripNotFound(_) => StatusCode::NOT_FOUND,
            Self::UnknownMode(_) => StatusCode::BAD_REQUEST,
            Self::DebugModeRequired => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// HTTP-specific error responses for the REST API.
#[derive(Debug, serde::Serialize)]
pub struct ErrorResponse {
    /// Human-readable error message.
    pub error: String,
    /// Optional error code for programmatic handling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl ErrorResponse {
    /// Creates a new error response with the given message.
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: None,
        }
    }

    /// Creates a new error response with both message and code.
    pub fn with_code(error: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: Some(code.into()),
        }
    }
}

impl From<Error> for ErrorResponse {
    fn from(err: Error) -> Self {
        Self::new(err.to_string())
    }
}
