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
use ulid::Ulid;
use uuid::Uuid;

fn build_canonical_path(dir: &Path) -> Option<PathBuf> {
    let mut canonical_dir_path = PathBuf::new();

    // create canonical representation
    for component in dir.components() {
        match component {
            Component::Normal(subpath) => {
                canonical_dir_path.push(subpath);
            }
            Component::CurDir => {
                // ignore
            }
            _ => {
                return None;
            }
        }
    }

    Some(canonical_dir_path)
}

#[derive(Clone, Debug, Default)]
pub struct SpoolPathComponents {
    pub spool: PathBuf,
    pub vault: Uuid,
    pub prefix: Option<PathBuf>,
    pub id: Option<Ulid>,
}

impl SpoolPathComponents {
    pub fn new(spool: PathBuf, vault: Uuid, prefix: Option<PathBuf>, id: Ulid) -> Self {
        Self {
            spool,
            vault,
            prefix,
            id: Some(id),
        }
    }

    pub fn from_prefix(spool: PathBuf, vault: Uuid, prefix: PathBuf) -> Self {
        Self {
            spool,
            vault,
            prefix: Some(prefix),
            id: None,
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

impl SpoolPathComponents {
    pub fn to_queue_path(&self, queue: Queue) -> Option<PathBuf> {
        let mut backup_dir = PathBuf::new();

        // backup_dir starts with the spool directory
        backup_dir.push(&self.spool);

        // next up: queue path
        backup_dir.push::<PathBuf>(queue.into());

        log::trace!("Base directory for {queue:?} queue: {backup_dir:?}");

        // next we add a vault as lower-case hyphenated UUID
        let vault_string = &self.vault.to_string();
        let backup_vault_path = Path::new(vault_string);
        let Some(vault_dir) = build_canonical_path(backup_vault_path) else {
            return None;
        };
        log::trace!("Using vault directory {vault_dir:?}");
        backup_dir.push(vault_dir);

        // then the prefix key, potentially containing a path of length >= 1
        let prefix_path = if let Some(prefix) = &self.prefix {
            prefix.as_path()
        } else {
            Path::new("")
        };

        let Some(prefix_dir) = build_canonical_path(prefix_path) else {
            return None;
        };
        log::trace!("Using prefix path {prefix_dir:?}");
        backup_dir.push(prefix_dir);

        // finally, the current ULID path (timestamp + random)
        let Some(id) = &self.id else {
            return Some(backup_dir);
        };
        let ulid_string = id.to_string();
        let ulid_path = Path::new(&ulid_string);

        let Some(ulid_dir) = build_canonical_path(ulid_path) else {
            return None;
        };
        log::trace!(
            "Using ULID directory {ulid_dir:?}: timestamp={ulid_timestamp:?} random={ulid_random:x?}",
            ulid_timestamp=DateTime::<Utc>::from(id.datetime()),
            ulid_random=id.random(),
        );
        backup_dir.push(ulid_dir);

        Some(backup_dir)
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
