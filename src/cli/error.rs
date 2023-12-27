// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::{env, fmt, io};

use crate::config::ParseConfigError;

use super::CliResult;

#[derive(thiserror::Error, fmt::Debug)]
pub enum CliError {
    #[error("BaseDirError: {0} {1}")]
    BaseDirError(xdg::BaseDirectoriesError, CliResult),
    #[error("ConfigurationError: {0} {1}")]
    ConfigurationError(ParseConfigError, CliResult),
    #[error("EnvError: {0} {1}")]
    EnvError(env::VarError, CliResult),
    #[error("IoError: {0} {1}")]
    IoError(io::Error, CliResult),
    #[error("LogError: Cannot call set_logger more than once {1}")]
    LogError(log::SetLoggerError, CliResult),
}

impl From<ParseConfigError> for CliError {
    fn from(error: ParseConfigError) -> Self {
        match error {
            ParseConfigError::TomlDeError(_) => {
                CliError::ConfigurationError(error, CliResult::ConfigError)
            }
            ParseConfigError::IoError(err) => CliError::IoError(err, CliResult::IoError),
        }
    }
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
