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

#[cfg(test)]
mod tests {
    use super::{AppError, AppResult};
    use std::error::Error;
    use std::io;

    #[test]
    fn io_error_display_wraps_the_source() {
        let error = AppError::Io(io::Error::new(io::ErrorKind::NotFound, "no such file"));
        assert_eq!(error.to_string(), "I/O error: no such file");
    }

    #[test]
    fn config_error_display_prefixes_the_message() {
        assert_eq!(
            AppError::Config("broken path".to_string()).to_string(),
            "Configuration error: broken path"
        );
    }

    #[test]
    fn validation_error_display_lists_every_bullet_on_its_own_line() {
        let error = AppError::Validation(vec!["first".to_string(), "second".to_string()]);

        assert_eq!(
            error.to_string(),
            "Validation errors:\n  - first\n  - second\n"
        );
    }

    #[test]
    fn validation_error_display_handles_a_single_and_empty_list() {
        assert_eq!(
            AppError::Validation(vec!["only".to_string()]).to_string(),
            "Validation errors:\n  - only\n"
        );
        // writeln! still emits the header newline.
        assert_eq!(
            AppError::Validation(vec![]).to_string(),
            "Validation errors:\n"
        );
    }

    #[test]
    fn monitor_and_display_errors_are_plainly_labelled() {
        assert_eq!(
            AppError::Monitor("no hyprctl".to_string()).to_string(),
            "Monitor error: no hyprctl"
        );
        assert_eq!(
            AppError::Display("too small".to_string()).to_string(),
            "Display error: too small"
        );
    }

    #[test]
    fn io_errors_convert_through_from() {
        let error: AppError = io::Error::new(io::ErrorKind::PermissionDenied, "denied").into();

        match &error {
            AppError::Io(inner) => {
                assert_eq!(inner.kind(), io::ErrorKind::PermissionDenied);
                assert_eq!(inner.to_string(), "denied");
            }
            other => panic!("expected an Io error, got {:?}", other),
        }
        assert!(error.to_string().contains("denied"));
    }

    #[test]
    fn source_is_exposed_only_for_io_errors() {
        let io_error = AppError::Io(io::Error::new(io::ErrorKind::Other, "root cause"));
        let source = io_error.source().expect("Io errors must expose a source");
        assert_eq!(source.to_string(), "root cause");

        let others = [
            AppError::Config("c".to_string()),
            AppError::Validation(vec![]),
            AppError::Monitor("m".to_string()),
            AppError::Display("d".to_string()),
        ];
        for error in others {
            assert!(
                error.source().is_none(),
                "{:?} should not expose a source",
                error
            );
        }
    }

    #[test]
    fn app_error_satisfies_the_std_error_bounds() {
        // Compile-time contract: usable anywhere a boxed error is expected.
        fn assert_error<T: std::error::Error + Send + Sync + 'static>(_: &T) {}
        assert_error(&AppError::Config("x".to_string()));
        assert_error(&AppError::Validation(vec!["x".to_string()]));
        assert_error(&AppError::Io(io::Error::new(io::ErrorKind::Other, "x")));
    }

    #[test]
    fn app_result_alias_propagates_both_outcomes() {
        // Wrapped in helpers so the outcomes are not statically known.
        fn produce_ok() -> AppResult<i32> {
            Ok(7)
        }
        fn produce_err() -> AppResult<i32> {
            Err(AppError::Monitor("boom".to_string()))
        }

        assert_eq!(produce_ok().unwrap(), 7);
        assert_eq!(
            produce_err().unwrap_err().to_string(),
            "Monitor error: boom"
        );
    }
}
