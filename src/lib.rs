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
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

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

fn use_dir_atomic_create_maybe(
    dir_path: &Path,
    create_dir: Option<bool>,
    recursive: Option<bool>,
) -> io::Result<()> {
    if create_dir.unwrap_or(false) {
        log::info!("Creating directory {dir_path:?}");
        // first mkdir the parent path, ignoring if it exists, and then perfrom
        // atomic creation of the final element in dir_path
        // https://rcrowley.org/2010/01/06/things-unix-can-do-atomically.html
        let mut builder = fs::DirBuilder::new();
        builder.mode(0o755);

        builder.recursive(recursive.unwrap_or(false));
        if let Some(parent) = dir_path.parent() {
            builder.create(parent).map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("Cannot create {path:?}: {err}", path = parent.display()),
                )
            })?;
        }

        // force failure if full dir_path already exists
        builder.recursive(false);
        builder.create(dir_path).map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("Cannot create {path:?}: {err}", path = dir_path.display()),
            )
        })?;
    } else if let Err(err) = fs::read_dir(dir_path) {
        // PermissionDenied, NotADirectory, NotFound, etc.
        log::error!("Cannot use directory {dir_path:?}");
        return Err(err);
    }

    Ok(())
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

    let spool = &config.spool;
    use_dir_atomic_create_maybe(spool, None, None)?;
    log::trace!("Using spool directory {spool:?}");

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
