// Copyright The Permafrust Authors.
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
    path::{Component, Path, PathBuf},
};

use chrono::{DateTime, Utc};
use nix::NixPath;
use ulid::Ulid;
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct SpoolPathComponents {
    pub spool: PathBuf,
    pub vault: Option<Uuid>,
    pub prefix: Option<PathBuf>,
    pub id: Option<Ulid>,
}

impl SpoolPathComponents {
    pub fn new(spool: PathBuf, vault: Uuid, prefix: Option<PathBuf>, id: Ulid) -> Self {
        Self {
            spool,
            vault: Some(vault),
            prefix,
            id: Some(id),
        }
    }

    pub fn from_prefix(spool: PathBuf, vault: Uuid, prefix: PathBuf) -> Self {
        Self {
            spool,
            vault: Some(vault),
            prefix: Some(prefix),
            id: None,
        }
    }

    pub fn from_spool(spool: PathBuf) -> Self {
        Self {
            spool,
            vault: None,
            prefix: None,
            id: None,
        }
    }

    pub fn with_vault(self, vault: Uuid) -> Self {
        Self {
            spool: self.spool,
            vault: Some(vault),
            prefix: self.prefix,
            id: self.id,
        }
    }

    pub fn with_prefix(self, prefix: PathBuf) -> Self {
        Self {
            spool: self.spool,
            vault: self.vault,
            prefix: Some(prefix),
            id: self.id,
        }
    }

    pub fn with_id(self, id: Ulid) -> Self {
        Self {
            spool: self.spool,
            vault: self.vault,
            prefix: self.prefix,
            id: Some(id),
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

fn invalid_path_error(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg)
}

#[derive(Clone, Debug)]
pub enum SpoolNameComponent {
    Vault(Uuid),
    Prefix(PathBuf),
    Id(Ulid),
}

fn build_canonical_path_inner(dir: &PathBuf) -> io::Result<PathBuf> {
    // here we only care about relative paths, needs validate_prefix first
    let mut canonical_dir_path = PathBuf::new();

    // create canonical representation
    for component in dir.components() {
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
                    "Path {dir:?} must not contain a .. component"
                )));
            }
        }
    }
    Ok(canonical_dir_path)
}

fn build_canonical_path(comp: SpoolNameComponent) -> io::Result<PathBuf> {
    let dir = match comp {
        SpoolNameComponent::Vault(vault) => PathBuf::from(vault.to_string()),
        SpoolNameComponent::Prefix(ref prefix) => build_canonical_path_inner(prefix)?,
        SpoolNameComponent::Id(ulid) => PathBuf::from(ulid.to_string()),
    };
    log::trace!("Using {comp:?} directory {dir:?}");
    Ok(dir)
}

fn validate_prefix(prefix: Option<&PathBuf>) -> io::Result<PathBuf> {
    let prefix = if let Some(pfx) = prefix {
        if pfx.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "prefix cannot be empty".to_string(),
            ));
        }
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

impl SpoolPathComponents {
    pub fn to_queue_path(&self, queue: Queue) -> io::Result<PathBuf> {
        let mut path = PathBuf::new();

        // backup_dir starts with the spool directory
        path.push(&self.spool);

        // next up: queue path
        path.push::<PathBuf>(queue.into());

        log::trace!("Base directory for {queue:?} queue: {path:?}");

        // next we add a vault as lower-case hyphenated UUID
        let Some(vault) = self.vault else {
            return Ok(path);
        };
        let vault_dir = build_canonical_path(SpoolNameComponent::Vault(vault))?;
        path.push(vault_dir);

        // then the prefix key, potentially containing a path of length >= 1
        let prefix_path = validate_prefix(self.prefix.as_ref())?;
        let prefix_dir = build_canonical_path(SpoolNameComponent::Prefix(prefix_path))?;
        path.push(prefix_dir);

        // finally, the current ULID path (timestamp + random) if available
        let Some(id) = self.id else {
            return Ok(path);
        };
        log::trace!(
            "Using ULID with timestamp={ulid_timestamp:?} and random={ulid_random:x?}",
            ulid_timestamp = DateTime::<Utc>::from(id.datetime()),
            ulid_random = id.random(),
        );
        let ulid_dir = build_canonical_path(SpoolNameComponent::Id(id))?;
        path.push(ulid_dir);

        Ok(path)
    }
}

#[derive(Debug, Default, PartialEq)]
pub(crate) enum CreateDirectory {
    #[default]
    No,
    NonRecursive,
    Recursive,
}

pub(crate) fn use_dir_atomic_create_maybe(
    dir_path: &Path,
    create_dir: CreateDirectory,
) -> io::Result<()> {
    if create_dir != CreateDirectory::No {
        log::info!("Creating directory {dir_path:?}");
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
