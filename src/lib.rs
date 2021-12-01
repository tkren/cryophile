mod backup;
pub mod constants;
pub mod encoder;
mod freeze;
mod restore;
mod split;
mod thaw;

pub use encoder::FinalEncoder;
pub use split::Split;
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;

pub struct Config {
    pub base: PathBuf,
    pub verbose: bool,
    pub quiet: bool,
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

fn use_base_dir(base: &str) -> io::Result<PathBuf> {
    match fs::metadata(base) {
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!("Base {} does not exist", base),
            ));
        }
        Ok(metadata) => {
            if !metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Base {} is not an existing directory", base),
                ));
            }
            Ok(PathBuf::from(base))
        }
    }
}

pub fn run<'a>(matches: &'a clap::ArgMatches) -> Result<(), CliError> {
    // setup logger using environment
    let env = env_logger::Env::new()
        .filter("PERMAFRUST_LOG")
        .write_style("PERMAFRUST_LOG_STYLE");
    env_logger::try_init_from_env(env)?;

    // parse global arguments and create Config
    let base_path = matches.value_of("base").unwrap_or("/tmp");
    let base_pathbuf: PathBuf = use_base_dir(base_path)?;
    log::trace!("Using base directory {:?}", base_path);

    let config = Config {
        base: base_pathbuf,
        verbose: matches.is_present("verbose"),
        quiet: matches.is_present("quiet"),
    };

    // perform requested subcommand
    match matches.subcommand() {
        ("backup", Some(m)) => backup::perform_backup(config, m)?,
        ("freeze", Some(m)) => freeze::perform_freeze(config, m)?,
        ("restore", Some(m)) => restore::perform_restore(config, m)?,
        ("thaw", Some(m)) => thaw::perform_thaw(config, m)?,
        _ => unreachable!(),
    };
    Ok(())
}
