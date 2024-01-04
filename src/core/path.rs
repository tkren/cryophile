// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::{fs, io, os::unix::fs::DirBuilderExt, path::PathBuf};

use super::backup_id::BackupId;

#[derive(Clone, Debug)]
pub struct SpoolPathComponents<'a> {
    pub spool: PathBuf,
    pub backup_id: Option<BackupId<'a>>,
}

impl<'a> SpoolPathComponents<'a> {
    pub fn new(spool: PathBuf, backup_id: BackupId<'a>) -> Self {
        Self {
            spool,
            backup_id: Some(backup_id),
        }
    }

    pub fn from_spool(spool: PathBuf) -> Self {
        Self {
            spool,
            backup_id: None,
        }
    }

    pub fn with_backup_id(self, backup_id: BackupId<'a>) -> Self {
        Self {
            spool: self.spool,
            backup_id: Some(backup_id),
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

impl<'a> SpoolPathComponents<'a> {
    pub fn uri(&self) -> Option<String> {
        // TODO we pretend that we always have an s3 bucket provider here
        let mut uri = String::from("s3://");
        uri.push_str(&self.backup_id?.to_string());
        Some(uri)
    }

    pub fn to_queue_path(&self, queue: Queue) -> io::Result<PathBuf> {
        let mut path = PathBuf::new();

        // backup_dir starts with the spool directory
        path.push(&self.spool);

        // next up: queue path
        path.push::<PathBuf>(queue.into());

        log::trace!("Base directory for {queue:?} queue: {path:?}");

        if let Some(backup_id) = self.backup_id {
            path.push(backup_id.to_path_buf());
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

    pub(crate) fn try_with_queue_path(
        &self,
        queue: Queue,
        create_dir: CreateDirectory,
    ) -> io::Result<(PathBuf, bool)> {
        let (path, created) = match self.with_queue_path(queue, create_dir) {
            Ok(path) => {
                // we could create path, now watch for incoming files
                (path, true)
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                let path = self.to_queue_path(queue)?;
                if !path.is_dir() {
                    return Err(err); // a non-directory is in the way, just bail out
                }
                (path, false) // reuse directory and walk
            }
            Err(err) => {
                return Err(err);
            }
        };
        Ok((path, created))
    }
}

pub(crate) fn use_base_dir(base: &xdg::BaseDirectories) -> io::Result<PathBuf> {
    let config_home = base.get_config_home();
    match fs::metadata(&config_home) {
        Err(_) => {
            log::debug!("Creating config home {config_home:?}");
            base.create_config_directory("")
        }
        Ok(metadata) if !metadata.is_dir() => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Config home {config_home:?} is not an existing directory"),
        )),
        Ok(_) => Ok(config_home),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn basic_spool_path_components() {
        let backup_id = BackupId::new(uuid::Uuid::nil(), None, ulid::Ulid::nil());
        let snc = SpoolPathComponents::new(PathBuf::from("/"), backup_id);
        let queue_path = snc
            .to_queue_path(Queue::Backup)
            .expect("this shouldn't fail");
        assert_eq!(
            queue_path,
            PathBuf::from(
                "/backup/00000000-0000-0000-0000-000000000000/00000000000000000000000000"
            )
        );

        let prefix = String::from("some/prefix");
        let backup_id = backup_id.with_prefix(&prefix);
        let queue_path = snc
            .with_backup_id(backup_id)
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
