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
    path::Path,
    str::FromStr,
};
use thiserror::Error;

use crate::compression::CompressionType;

#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    pub compression: Option<CompressionType>,
    pub vault: Vec<Vault>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Vault {
    pub id: uuid::Uuid,
    pub compression: Option<CompressionType>,
    pub profile: Option<Profile>,
    pub bucket: Option<Bucket>,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Profile {
    pub provider: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Bucket {
    pub name: String,
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
    pub fn new(path: &Path) -> Result<Self, ParseConfigError> {
        let mut file = File::open(path).map_err(ParseConfigError::from)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .map_err(ParseConfigError::from)?;
        log::info!("Reading configuration file {path:?}");
        ConfigFile::from_str(&buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_config_file() {
        let config_str = r#"[[vault]]
id = "797daf41-ba2c-440e-a56a-d0a190403a0b"
    [vault.profile]
    provider = "s3"
    [vault.bucket]
    name = "the-bucket-name"

[[vault]]
id = "23e52b86-7293-4889-824f-50135685c9e4"
compression = "Lz4"
    [vault.profile]
    provider = "s3"
"#;

        let config = ConfigFile::from_str(config_str).expect("should work as is");
        assert_eq!(config.compression, None);
        assert_eq!(config.vault.len(), 2);

        let mut vaults = config.vault.iter();

        let v0 = Vault {
            id: uuid::Uuid::from_str("797daf41-ba2c-440e-a56a-d0a190403a0b").unwrap(),
            profile: Some(Profile {
                provider: "s3".to_owned(),
            }),
            compression: None,
            bucket: Some(Bucket {
                name: "the-bucket-name".to_owned(),
            }),
        };
        assert_eq!(vaults.next().expect(""), &v0);

        let v1 = Vault {
            id: uuid::Uuid::from_str("23e52b86-7293-4889-824f-50135685c9e4").unwrap(),
            profile: Some(Profile {
                provider: "s3".to_owned(),
            }),
            compression: Some(CompressionType::Lz4),
            bucket: None,
        };
        assert_eq!(vaults.next().expect(""), &v1);

        assert_eq!(vaults.next(), None);
    }
}
