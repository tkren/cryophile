use std::{
    fs, io,
    os::unix::fs::DirBuilderExt,
    path::{Component, Path, PathBuf},
};

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
pub struct BackupPathComponents {
    pub spool: PathBuf,
    pub vault: Uuid,
    pub output: Option<PathBuf>,
    pub timestamp: Option<time::OffsetDateTime>,
}

impl From<(PathBuf, Uuid, Option<PathBuf>)> for BackupPathComponents {
    fn from((spool, vault, output): (PathBuf, Uuid, Option<PathBuf>)) -> Self {
        Self {
            spool,
            vault,
            output,
            ..Self::default()
        }
    }
}

impl From<(PathBuf, Uuid, Option<PathBuf>, time::OffsetDateTime)> for BackupPathComponents {
    fn from(
        (spool, vault, output, timestamp): (PathBuf, Uuid, Option<PathBuf>, time::OffsetDateTime),
    ) -> Self {
        Self {
            spool,
            vault,
            output,
            timestamp: Some(timestamp),
        }
    }
}

impl From<&BackupPathComponents> for Option<PathBuf> {
    fn from(backup_components: &BackupPathComponents) -> Self {
        let mut backup_dir = PathBuf::new();

        // backup_dir starts with the spool directory
        backup_dir.push(&backup_components.spool);

        // next we add a vault as lower-case hyphenated UUID
        let vault_string = &backup_components.vault.to_string();
        let backup_vault_path = Path::new(vault_string);
        let Some(vault_dir) = build_canonical_path(backup_vault_path) else {return None;};
        log::trace!("Using vault directory {vault_dir:?}");
        backup_dir.push(vault_dir);

        // then the output key, potentially containing a path of length >= 1
        let output = match &backup_components.output {
            Some(output) => output.as_path(),
            _ => Path::new(""),
        };

        let Some(output_dir) = build_canonical_path(output) else {return None;};
        log::trace!("Using output directory {output_dir:?}");
        backup_dir.push(output_dir);

        // finally, the current UTC timestamp
        let Some(ts) = backup_components.timestamp else {return Some(backup_dir);};
        let utc_string = ts.unix_timestamp().to_string();
        let utc_timestamp = Path::new(&utc_string);

        let Some(timestamp_dir) = build_canonical_path(utc_timestamp) else {return None;};
        log::trace!("Using timestamp directory {timestamp_dir:?}");
        backup_dir.push(timestamp_dir);

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
