// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::{
    fmt,
    process::{ExitCode, Termination},
};

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum CliResult {
    Ok = 0,
    IoError = 42,
    Usage = 64,
    LogError = 65,
    ConfigError = 78,
    Abort = 255,
}

impl fmt::Display for CliResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(exit code {})", *self as u8)
    }
}

impl Termination for CliResult {
    fn report(self) -> ExitCode {
        match self {
            CliResult::Ok => log::debug!("Terminating without error"),
            _ => log::error!("Terminating with error(s) {self}"),
        };
        ExitCode::from(self as u8)
    }
}
