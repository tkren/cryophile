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
