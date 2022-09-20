mod configfile;

use crate::cli::Cli;

pub use self::configfile::ConfigFile;

pub struct Config {
    pub base: xdg::BaseDirectories,
    pub cli: Cli,
}
