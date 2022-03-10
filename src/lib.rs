mod backup;
pub mod constants;
pub mod encoder;
mod freeze;
mod restore;
mod split;
mod thaw;

pub use encoder::FinalEncoder;
pub use split::Split;
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;

pub struct Config {
    pub base: xdg::BaseDirectories,
    pub spool: PathBuf,
    pub verbose: bool,
    pub quiet: bool,
}

pub enum CliError {
    BaseDirError(xdg::BaseDirectoriesError, i32),
    EnvError(env::VarError, i32),
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
            CliError::BaseDirError(error, code) => write!(f, "{error} ({code})"),
            CliError::EnvError(error, code) => write!(f, "{error} ({code})"),
            CliError::IoError(error, code) => write!(f, "{error} ({code})"),
            CliError::LogError(_error, _code) => write!(f, "Cannot call set_logger more than once"),
        }
    }
}

impl fmt::Debug for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaseDirError(error, code) => f
                .debug_tuple("BaseDirError")
                .field(error)
                .field(code)
                .finish(),
            Self::EnvError(error, code) => {
                f.debug_tuple("EnvError").field(error).field(code).finish()
            }
            Self::IoError(error, code) => {
                f.debug_tuple("IoError").field(error).field(code).finish()
            }
            Self::LogError(error, code) => {
                f.debug_tuple("LogError").field(error).field(code).finish()
            }
        }
    }
}

fn use_base_dir(base: &xdg::BaseDirectories) -> io::Result<PathBuf> {
    let state_home = base.get_state_home();
    match fs::metadata(&state_home) {
        Err(_err) => {
            log::info!("Creating state directory {state_home:?}");
            match base.create_state_directory("") {
                Ok(state_path) => Ok(state_path),
                Err(err) => Err(err),
            }
        }
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Base state home {state_home:?} is not an existing directory"),
                ));
            }
            Ok(state_home)
        }
    }
}

pub fn base_directory_profile(subcommand: &str) -> Result<xdg::BaseDirectories, CliError> {
    match xdg::BaseDirectories::with_profile(clap::crate_name!(), subcommand) {
        Ok(base_dirs) => Ok(base_dirs),
        Err(err) => Err(CliError::BaseDirError(err, 1)),
    }
}

pub fn run<'a>(
    config: Config,
    command: &str,
    matches: &'a clap::ArgMatches,
) -> Result<(), CliError> {
    // setup logger using environment
    let env = env_logger::Env::new()
        .filter("PERMAFRUST_LOG")
        .write_style("PERMAFRUST_LOG_STYLE");
    env_logger::try_init_from_env(env)?;

    // setup base directory
    let base_pathbuf: PathBuf = use_base_dir(&config.base)?;
    log::trace!("Using base state directory {base_pathbuf:?}");

    // perform requested command
    match command {
        "backup" => backup::perform_backup(config, matches)?,
        "freeze" => freeze::perform_freeze(config, matches)?,
        "restore" => restore::perform_restore(config, matches)?,
        "thaw" => thaw::perform_thaw(config, matches)?,
        _ => unreachable!(),
    };
    Ok(())
}
