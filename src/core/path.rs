// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::{
    fs, io,
    os::unix::fs::DirBuilderExt,
    path::{Component, PathBuf},
};

use chrono::{DateTime, Utc};
use nix::NixPath;
use ulid::Ulid;
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SpoolNameComponent {
    #[default]
    None,
    Vault(Uuid),
    Prefix(Option<PathBuf>),
    Id(Ulid),
}

impl SpoolNameComponent {
    pub fn canonical_path(&self) -> io::Result<PathBuf> {
        let dir = match self {
            SpoolNameComponent::None => {
                return Err(invalid_path_error(
                    "cannot build unassigned spool name component".to_string(),
                ))
            }
            SpoolNameComponent::Vault(vault) => PathBuf::from(vault.to_string()),
            SpoolNameComponent::Id(ulid) => PathBuf::from(ulid.to_string()),
            SpoolNameComponent::Prefix(prefix) => {
                let prefix = validate_prefix(prefix)?;
                // here we only care about relative paths, needs validate_prefix first
                let mut canonical_dir_path = PathBuf::new();
                // create canonical representation
                for component in prefix.components() {
                    match component {
                        Component::Normal(subpath) => {
                            canonical_dir_path.push(subpath);
                        }
                        Component::CurDir => {
                            // ignore paths that start with ./ (other CurDir are already normalized)
                        }
                        _ => {
                            // ParentDir and RootDir and Prefix are disallowed
                            return Err(invalid_path_error(format!(
                                "Path {prefix:?} must not contain a .. component"
                            )));
                        }
                    }
                }
                canonical_dir_path
            }
        };
        log::trace!("Using {self:?} directory {dir:?}");
        Ok(dir)
    }
}

fn validate_prefix(prefix: &Option<PathBuf>) -> io::Result<PathBuf> {
    let prefix = if let Some(pfx) = prefix {
        if pfx.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "prefix cannot be empty".to_string(),
            ));
        }
        // TODO check if prefix is safe
        if pfx.has_root() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "prefix cannot have a root component or be absolute".to_string(),
            ));
        }
        pfx.to_owned()
    } else {
        // only undefined prefixes can be empty
        PathBuf::new()
    };
    Ok(prefix)
}

#[derive(Clone, Debug, Default)]
pub struct SpoolPathComponents {
    pub spool: PathBuf,
    pub vault: SpoolNameComponent,
    pub prefix: SpoolNameComponent,
    pub id: SpoolNameComponent,
}

impl SpoolPathComponents {
    pub fn new(spool: PathBuf, vault: Uuid, prefix: Option<PathBuf>, id: Ulid) -> Self {
        Self {
            spool,
            vault: SpoolNameComponent::Vault(vault),
            prefix: SpoolNameComponent::Prefix(prefix),
            id: SpoolNameComponent::Id(id),
        }
    }

    pub fn from_prefix(spool: PathBuf, vault: Uuid, prefix: PathBuf) -> Self {
        Self {
            spool,
            vault: SpoolNameComponent::Vault(vault),
            prefix: SpoolNameComponent::Prefix(Some(prefix)),
            id: SpoolNameComponent::None,
        }
    }

    pub fn from_spool(spool: PathBuf) -> Self {
        Self {
            spool,
            vault: SpoolNameComponent::None,
            prefix: SpoolNameComponent::None,
            id: SpoolNameComponent::None,
        }
    }

    pub fn with_vault(self, vault: Uuid) -> Self {
        Self {
            spool: self.spool,
            vault: SpoolNameComponent::Vault(vault),
            prefix: self.prefix,
            id: self.id,
        }
    }

    pub fn with_prefix(self, prefix: PathBuf) -> Self {
        Self {
            spool: self.spool,
            vault: self.vault,
            prefix: SpoolNameComponent::Prefix(Some(prefix)),
            id: self.id,
        }
    }

    pub fn with_id(self, id: Ulid) -> Self {
        Self {
            spool: self.spool,
            vault: self.vault,
            prefix: self.prefix,
            id: SpoolNameComponent::Id(id),
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub enum Queue {
    #[default]
    Backup,
    Freeze,
    Thaw,
    Restore,
}

impl From<Queue> for PathBuf {
    fn from(queue: Queue) -> Self {
        PathBuf::from(match queue {
            Queue::Backup => "backup",
            Queue::Freeze => "freeze",
            Queue::Thaw => "thaw",
            Queue::Restore => "restore",
        })
    }
}

#[derive(Debug, Default, PartialEq)]
pub(crate) enum CreateDirectory {
    #[default]
    No,
    NonRecursive,
    Recursive,
}

fn invalid_path_error(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

impl SpoolPathComponents {
    pub fn uri(&self) -> Option<String> {
        // TODO we pretend that we always have an s3 bucket provider here
        let mut uri = String::from("s3://");
        let vault = self.vault.canonical_path().ok()?;
        uri.push_str(vault.to_str()?);
        uri.push('/');
        let prefix = self.prefix.canonical_path().ok()?;
        uri.push_str(prefix.to_str()?);
        uri.push('/');
        let id = self.id.canonical_path().ok()?;
        uri.push_str(id.to_str()?);
        Some(uri)
    }

    pub fn to_queue_path(&self, queue: Queue) -> io::Result<PathBuf> {
        let mut path = PathBuf::new();

        // backup_dir starts with the spool directory
        path.push(&self.spool);

        // next up: queue path
        path.push::<PathBuf>(queue.into());

        log::trace!("Base directory for {queue:?} queue: {path:?}");

        // next we add a vault as lower-case hyphenated UUID
        if self.vault == SpoolNameComponent::None {
            return Ok(path);
        };
        let vault_dir = self.vault.canonical_path()?;
        path.push(vault_dir);

        // then the prefix key, potentially containing a path of length >= 1
        let prefix_dir = self.prefix.canonical_path()?;
        path.push(prefix_dir);

        // finally, the current ULID path (timestamp + random) if available
        match self.id {
            SpoolNameComponent::None => return Ok(path),
            SpoolNameComponent::Id(id) => {
                log::trace!(
                    "Using ULID with timestamp={ulid_timestamp:?} and random={ulid_random:x?}",
                    ulid_timestamp = DateTime::<Utc>::from(id.datetime()),
                    ulid_random = id.random(),
                );

                let ulid_dir = self.id.canonical_path()?;
                path.push(ulid_dir);
            }
            _ => panic!("id must be SpoolNameComponent::Id or SpoolNameComponent::None"),
        };

        Ok(path)
    }

    pub(crate) fn with_queue_path(
        &self,
        queue: Queue,
        create_dir: CreateDirectory,
    ) -> io::Result<PathBuf> {
        let dir_path = self.to_queue_path(queue)?;
        if create_dir != CreateDirectory::No {
            log::trace!("Creating directory {dir_path:?}");
            // first mkdir the parent path, ignoring if it exists, and then perform
            // atomic creation of the final element in dir_path
            // https://rcrowley.org/2010/01/06/things-unix-can-do-atomically.html
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o755);
            if let Some(parent) = dir_path.parent() {
                builder.recursive(create_dir != CreateDirectory::NonRecursive);
                builder.create(parent).map_err(|err| {
                    io::Error::new(
                        err.kind(),
                        format!("Cannot create {path:?}: {err}", path = parent.display()),
                    )
                })?;
            }
            // force failure if full dir_path already exists
            builder.recursive(false);
            builder.create(&dir_path).map_err(|err| {
                io::Error::new(
                    err.kind(),
                    format!("Cannot create {path:?}: {err}", path = dir_path.display()),
                )
            })?;
        } else if let Err(err) = fs::read_dir(&dir_path) {
            // PermissionDenied, NotADirectory, NotFound, etc.
            log::error!("Cannot use directory {dir_path:?}");
            return Err(err);
        }
        Ok(dir_path)
    }
}

pub(crate) fn use_base_dir(base: &xdg::BaseDirectories) -> io::Result<PathBuf> {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn basic_spool_path_components() {
        let snc = SpoolPathComponents::new(
            PathBuf::from("/"),
            uuid::Uuid::nil(),
            None,
            ulid::Ulid::nil(),
        );
        let queue_path = snc
            .to_queue_path(Queue::Backup)
            .expect("this shouldn't fail");
        assert_eq!(
            queue_path,
            PathBuf::from(
                "/backup/00000000-0000-0000-0000-000000000000/00000000000000000000000000"
            )
        );

        let queue_path = snc
            .with_prefix(PathBuf::from("some/prefix"))
            .to_queue_path(Queue::Restore)
            .expect("this shouldn't fail");
        assert_eq!(
            queue_path,
            PathBuf::from(
                "/restore/00000000-0000-0000-0000-000000000000/some/prefix/00000000000000000000000000"
            )
        );
    }
}
