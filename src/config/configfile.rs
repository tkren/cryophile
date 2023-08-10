// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use serde_derive::Deserialize;
use std::{
    fs::File,
    io::{self, Read},
    path::PathBuf,
    str::FromStr,
};
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    pub vault: Option<Vec<Vault>>,
}

#[derive(Debug, Deserialize)]
pub struct Vault {
    pub id: uuid::Uuid,
    pub profile: Profile,
}

#[derive(Debug, Deserialize)]
pub struct Profile {
    pub provider: String,
}

#[derive(Error, Debug)]
pub enum ParseConfigError {
    #[error("TOML deserialization error: {0}")]
    TomlDeError(#[from] toml::de::Error),
    #[error("IoError")]
    IoError(#[from] io::Error),
}

impl FromStr for ConfigFile {
    type Err = ParseConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let config = match toml::from_str::<ConfigFile>(s) {
            Ok(config) => config,
            Err(err) => {
                return Err(ParseConfigError::from(err));
            }
        };
        Ok(config)
    }
}

impl ConfigFile {
    pub fn new(path: &PathBuf) -> Result<Self, ParseConfigError> {
        let mut file = File::open(path).map_err(ParseConfigError::from)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .map_err(ParseConfigError::from)?;
        ConfigFile::from_str(&buf)
    }
}
