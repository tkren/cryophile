// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::path::{Component, Path};
use std::{fmt, path::PathBuf};

use ulid::Ulid;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub struct BackupId<'a> {
    vault: Uuid,
    prefix: Option<&'a str>,
    ulid: Option<Ulid>,
}

fn canonical_relative_path(path: &Path) -> PathBuf {
    let mut canonical_dir_path = PathBuf::new();
    // create canonical representation
    for component in path.components() {
        match component {
            Component::Normal(subpath) => {
                canonical_dir_path.push(subpath);
            }
            Component::ParentDir => {
                // for every ../ just pop to parent
                canonical_dir_path.pop();
            }
            _ => {
                // Component::CurDir / Component::RootDir / Component::Prefix
                // ignore components that start with
                // - current directory ./ (other CurDir are already normalized),
                // - the root /
                // - a prefix
            }
        }
    }
    canonical_dir_path
}

fn canonical_prefix(prefix: &str) -> String {
    String::from(
        canonical_relative_path(&PathBuf::from(prefix))
            .to_str()
            .unwrap_or_default(),
    )
}

impl<'a> BackupId<'a> {
    pub fn new(vault: Uuid, prefix: Option<&'a str>, ulid: Ulid) -> Self {
        Self {
            vault,
            prefix,
            ulid: Some(ulid),
        }
    }

    pub fn from_prefix(vault: Uuid, prefix: &'a str) -> Self {
        Self {
            vault,
            prefix: Some(prefix),
            ulid: None,
        }
    }

    pub fn with_vault(self, vault: Uuid) -> Self {
        Self {
            vault,
            prefix: self.prefix,
            ulid: self.ulid,
        }
    }

    pub fn with_prefix(self, prefix: &'a str) -> Self {
        Self {
            vault: self.vault,
            prefix: Some(prefix),
            ulid: self.ulid,
        }
    }

    pub fn with_ulid(self, ulid: Ulid) -> Self {
        Self {
            vault: self.vault,
            prefix: self.prefix,
            ulid: Some(ulid),
        }
    }

    pub fn to_path_buf(&self) -> PathBuf {
        let mut path = PathBuf::new();
        path.push(self.vault.to_string());
        if let Some(pfx) = self.prefix {
            let canonical_prefix = canonical_relative_path(&PathBuf::from(pfx.to_string()));
            if !canonical_prefix.as_path().as_os_str().is_empty() {
                path.push(canonical_prefix)
            }
        };
        if let Some(ulid) = self.ulid {
            path.push(ulid.to_string())
        };
        path
    }

    pub fn to_vault_key(&self, delimiter: char) -> String {
        let mut vault_key = String::new();
        if let Some(prefix) = self.prefix {
            vault_key.push_str(&canonical_prefix(prefix));
            if let Some(ulid) = self.ulid {
                vault_key.push(delimiter);
                vault_key.push_str(&ulid.to_string());
            };
        } else if let Some(ulid) = self.ulid {
            vault_key.push_str(&ulid.to_string());
        };
        vault_key
    }

    pub fn to_delimited_string(&self, delimiter: char) -> String {
        let mut backup_id = String::new();
        backup_id.push_str(&self.vault.to_string());
        if self.prefix.is_some() || self.ulid.is_some() {
            backup_id.push(delimiter);
            backup_id.push_str(&self.to_vault_key(delimiter));
        }
        backup_id
    }
}

impl<'a> fmt::Display for BackupId<'a> {
    // used in to_string()
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{vault}", vault = self.vault)?;
        if self.prefix.is_some() || self.ulid.is_some() {
            write!(f, "/{vault_key}", vault_key = self.to_vault_key('/'))?;
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_backup_id() {
        let backup_id = BackupId::new(uuid::Uuid::nil(), None, ulid::Ulid::nil());
        let backup_id_path = backup_id.to_path_buf();
        assert_eq!(
            backup_id_path,
            PathBuf::from("00000000-0000-0000-0000-000000000000/00000000000000000000000000")
        );
        let backup_id_string = backup_id.to_string();
        assert_eq!(
            backup_id_string,
            String::from("00000000-0000-0000-0000-000000000000/00000000000000000000000000")
        );
        let vault_key = backup_id.to_vault_key('/');
        assert_eq!(vault_key, String::from("00000000000000000000000000"));

        let prefix = String::from("some/prefix");
        let backup_id = backup_id.with_prefix(&prefix);
        let backup_id_path = backup_id.to_path_buf();
        assert_eq!(
            backup_id_path,
            PathBuf::from(
                "00000000-0000-0000-0000-000000000000/some/prefix/00000000000000000000000000"
            )
        );
        let backup_id_string = backup_id.to_string();
        assert_eq!(
            backup_id_string,
            String::from(
                "00000000-0000-0000-0000-000000000000/some/prefix/00000000000000000000000000"
            )
        );
        let backup_id_string = backup_id.to_delimited_string('+');
        assert_eq!(
            backup_id_string,
            String::from(
                "00000000-0000-0000-0000-000000000000+some/prefix+00000000000000000000000000"
            )
        );
        let vault_key = backup_id.to_vault_key('+');
        assert_eq!(
            vault_key,
            String::from("some/prefix+00000000000000000000000000")
        );

        let mut prefix = String::new();
        prefix.push('a');
        let backup_id = backup_id
            .with_vault(Uuid::max())
            .with_prefix(&prefix)
            .with_ulid(Ulid::from_parts(u64::MAX, u128::MAX));
        let backup_id_path = backup_id.to_path_buf();
        assert_eq!(
            backup_id_path,
            PathBuf::from("ffffffff-ffff-ffff-ffff-ffffffffffff/a/7ZZZZZZZZZZZZZZZZZZZZZZZZZ")
        );
        let vault_key = backup_id.to_vault_key('+');
        assert_eq!(vault_key, String::from("a+7ZZZZZZZZZZZZZZZZZZZZZZZZZ"));
    }

    #[test]
    fn weird_prefix_backup_id() {
        let prefix = String::from("/..//some/../prefix/");
        let backup_id = BackupId::new(uuid::Uuid::nil(), Some(&prefix), ulid::Ulid::nil());
        let backup_id_path = backup_id.to_path_buf();
        assert_eq!(
            backup_id_path,
            PathBuf::from("00000000-0000-0000-0000-000000000000/prefix/00000000000000000000000000")
        );
        let backup_id_string = backup_id.to_string();
        assert_eq!(
            backup_id_string,
            String::from("00000000-0000-0000-0000-000000000000/prefix/00000000000000000000000000")
        );
        let backup_id_string = backup_id.to_delimited_string('+');
        assert_eq!(
            backup_id_string,
            String::from("00000000-0000-0000-0000-000000000000+prefix+00000000000000000000000000")
        );
        let vault_key = backup_id.to_vault_key('+');
        assert_eq!(vault_key, String::from("prefix+00000000000000000000000000"));

        let backup_id = BackupId::from_prefix(uuid::Uuid::nil(), &prefix);
        let backup_id_path = backup_id.to_path_buf();
        assert_eq!(
            backup_id_path,
            PathBuf::from("00000000-0000-0000-0000-000000000000/prefix")
        );
        let backup_id_string = backup_id.to_string();
        assert_eq!(
            backup_id_string,
            String::from("00000000-0000-0000-0000-000000000000/prefix")
        );
        let backup_id_string = backup_id.to_delimited_string('+');
        assert_eq!(
            backup_id_string,
            String::from("00000000-0000-0000-0000-000000000000+prefix")
        );
        let vault_key = backup_id.to_vault_key('+');
        assert_eq!(vault_key, String::from("prefix"));
    }
}
