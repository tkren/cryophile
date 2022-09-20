use std::{env, fmt, io};

use super::CliResult;

#[derive(thiserror::Error, fmt::Debug)]
pub enum CliError {
    #[error("BaseDirError: {0} {1}")]
    BaseDirError(xdg::BaseDirectoriesError, CliResult),
    #[error("EnvError: {0} {1}")]
    EnvError(env::VarError, CliResult),
    #[error("IoError: {0} {1}")]
    IoError(io::Error, CliResult),
    #[error("LogError: Cannot call set_logger more than once {1}")]
    LogError(log::SetLoggerError, CliResult),
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        CliError::IoError(error, CliResult::IoError)
    }
}

impl From<log::SetLoggerError> for CliError {
    fn from(error: log::SetLoggerError) -> Self {
        CliError::LogError(error, CliResult::LogError)
    }
}
