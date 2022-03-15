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

#[derive(thiserror::Error, fmt::Debug)]
pub enum CliError {
    #[error("BaseDirError: {0} (exit {1})")]
    BaseDirError(xdg::BaseDirectoriesError, exitcode::ExitCode),
    #[error("EnvError: {0} (exit {1})")]
    EnvError(env::VarError, exitcode::ExitCode),
    #[error("IoError: {0} (exit {1})")]
    IoError(io::Error, exitcode::ExitCode),
    #[error("LogError: Cannot call set_logger more than once (exit {1})")]
    LogError(log::SetLoggerError, exitcode::ExitCode),
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        if let Some(raw_os_error) = error.raw_os_error() {
            return CliError::IoError(error, raw_os_error);
        }
        CliError::IoError(error, exitcode::IOERR)
    }
}

impl From<log::SetLoggerError> for CliError {
    fn from(error: log::SetLoggerError) -> Self {
        CliError::LogError(error, 1)
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
        Err(err) => Err(CliError::BaseDirError(err, exitcode::CONFIG)),
    }
}

pub fn run(config: Config, command: &str, matches: &'_ clap::ArgMatches) -> Result<(), CliError> {
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
