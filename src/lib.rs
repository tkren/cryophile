mod backup;
mod freeze;
mod restore;
mod split;
mod thaw;

pub use encoder::FinalEncoder;
pub use split::Split;
use std::fmt;
use std::io;

pub struct Config {
    pub verbose: bool,
    pub quiet: bool,
}

impl Config {
    pub fn new<'a>(matches: &'a clap::ArgMatches) -> Self {
        Config {
            verbose: matches.is_present("verbose"),
            quiet: matches.is_present("quiet"),
        }
    }
}

pub enum CliError {
    IoError(io::Error, i32),
    LogError(log::SetLoggerError, i32),
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        if let Some(raw_os_error) = error.raw_os_error() {
            return CliError::IoError(error, raw_os_error);
        }
        CliError::IoError(error, 1)
    }
}

impl From<log::SetLoggerError> for CliError {
    fn from(error: log::SetLoggerError) -> Self {
        CliError::LogError(error, 1)
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::IoError(error, code) => write!(f, "{} ({})", error, code),
            CliError::LogError(_error, _code) => write!(f, "Cannot call set_logger more than once"),
        }
    }
}

pub fn run<'a>(config: Config, matches: &'a clap::ArgMatches) -> Result<(), CliError> {
    let env = env_logger::Env::new()
        .filter("PERMAFRUST_LOG")
        .write_style("PERMAFRUST_LOG_STYLE");
    env_logger::try_init_from_env(env)?;

    match matches.subcommand() {
        ("backup", Some(m)) => backup::perform_backup(config, m)?,
        ("freeze", Some(m)) => freeze::perform_freeze(config, m)?,
        ("restore", Some(m)) => restore::perform_restore(config, m)?,
        ("thaw", Some(m)) => thaw::perform_thaw(config, m)?,
        _ => unreachable!(),
    };
    Ok(())
}
