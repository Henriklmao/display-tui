//! Error handling for the display-tui application.
//!
//! This module provides a centralized error type for all operations
//! in the application, along with convenience type aliases.

use std::fmt;
use std::io;

// The primary error type for the application.
//
// This enum represents all possible error conditions that can occur
// during the execution of the display-tui application.
#[derive(Debug)]
pub enum AppError {
    // An I/O error occurred.
    Io(io::Error),
    // A configuration error with a descriptive message.
    Config(String),
    // A validation error with a list of error messages.
    Validation(Vec<String>),
    // A monitor-related error with a descriptive message.
    Monitor(String),
    // A display-related error with a descriptive message.
    Display(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(err) => write!(f, "I/O error: {}", err),
            AppError::Config(msg) => write!(f, "Configuration error: {}", msg),
            AppError::Validation(errors) => {
                writeln!(f, "Validation errors:")?;
                for error in errors {
                    writeln!(f, "  - {}", error)?;
                }
                Ok(())
            }
            AppError::Monitor(msg) => write!(f, "Monitor error: {}", msg),
            AppError::Display(msg) => write!(f, "Display error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(err: io::Error) -> Self {
        AppError::Io(err)
    }
}

// A specialized Result type for application operations.
//
// This is a convenience type alias that uses AppError as the error type.
pub type AppResult<T> = Result<T, AppError>;
