use std::path::{Component, Path, PathBuf};

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
