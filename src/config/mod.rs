// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

mod configfile;

use xdg::BaseDirectories;

use crate::cli::Cli;

pub use self::configfile::ConfigFile;
pub use self::configfile::ParseConfigError;

pub struct Config {
    pub base: xdg::BaseDirectories,
    pub cli: Cli,
    pub file: ConfigFile,
}

impl Config {
    pub fn new(base: BaseDirectories, cli: Cli, file: ConfigFile) -> Self {
        Self { base, cli, file }
    }
}
