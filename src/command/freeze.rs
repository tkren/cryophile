use crate::cli::{Cli, Freeze};
use crate::config::ConfigFile;
use crate::core::aws;
use crate::core::notify::notify_error;
use notify::event::{AccessKind, AccessMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::{fs, io};
use xdg::BaseDirectories;

pub fn perform_freeze(
    cli: &Cli,
    freeze: &Freeze,
    base_directories: &BaseDirectories,
) -> io::Result<()> {
    log::info!("FREEZE…");

    let aws_config_future = aws::aws_config(None);
    let aws_config = futures::executor::block_on(aws_config_future);
    log::trace!(
        "Using AWS config region {region:?}",
        region = aws_config.region()
    );

    let aws_client_future = aws::aws_client(&aws_config);
    let aws_client = futures::executor::block_on(aws_client_future);
    log::trace!("Using AWS client {aws_client:?}");

    let (tx, rx) = mpsc::channel();

    let mut watcher =
        RecommendedWatcher::new(tx, notify::Config::default()).map_err(notify_error)?;

    let config_path = if let Some(config) = &freeze.config {
        config.to_path_buf()
    } else {
        base_directories.get_config_file("permafrust.toml")
    };

    let config = if config_path.exists() {
        let config_file = match ConfigFile::new(&config_path) {
            Err(err) => {
                log::error!("Cannot parse config file {config_path:?}: {err}");
                return Err(io::Error::new(io::ErrorKind::Other, err));
            }
            Ok(config_file) => config_file,
        };
        Some(config_file)
    } else {
        log::warn!("Configuration {config_path:?} does not exist, ignoring configuration updates");
        None
    };

    log::trace!("Config: {config:#?}");

    let spool = cli.spool.as_ref();
    watch_read_dir(&mut watcher, spool)?;
    log::debug!("Watching spool {spool:?}");

    for res in rx {
        event_handler(res, &mut watcher).map_err(notify_error)?;
    }

    Ok(())
}

fn watch_read_dir(watcher: &mut notify::RecommendedWatcher, path: &Path) -> io::Result<()> {
    if !path.is_dir() {
        log::warn!("Ignoring non-directory: {path:?}");
        return Ok(());
    }

    watcher
        .watch(path, RecursiveMode::NonRecursive)
        .map_err(notify_error)?;
    log::debug!("Watching path {path:?}");

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let sub_path = entry.path();
        if sub_path.is_dir() {
            // TODO only watch configured vaults
            watcher
                .watch(sub_path.as_path(), RecursiveMode::NonRecursive)
                .map_err(notify_error)?;
            log::trace!("Watching subdirectory: {sub_path:?}")
            // TODO read_dir vault
        } else {
            log::warn!("Ignoring non-directory: {sub_path:?}")
        }
    }

    Ok(())
}

fn event_handler(
    result: Result<notify::Event, notify::Error>,
    watcher: &mut notify::RecommendedWatcher,
) -> Result<(), notify::Error> {
    // TODO check which level in spool causes the event
    // TODO inside vault: new backup dirs arrive, add them if they are not yet uploaded, if uploaded unwatch backup_dir
    // TODO if chunk.0 file arrives, backup is done, do another read_dir for files that are not in "w" mode
    match result {
        Ok(notify::Event { kind, paths, attrs })
            if kind == EventKind::Access(AccessKind::Close(AccessMode::Write)) =>
        {
            log::info!("Closed file event: {kind:?} {paths:?} {attrs:?}");
            for path in paths {
                watch_read_dir(watcher, &path)?;
            }
        }
        Ok(notify::Event { kind, paths, attrs }) => {
            log::debug!("Event: {kind:?} {paths:?} {attrs:?}");
        }
        Err(err) => {
            log::error!("watch error: {err:?}");
            return Err(err);
        }
    };
    Ok(())
}
