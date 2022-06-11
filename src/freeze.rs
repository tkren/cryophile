use crate::cli::{Cli, Freeze};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::io;
use std::sync::mpsc;
use xdg::BaseDirectories;

pub fn perform_freeze(
    cli: &Cli,
    freeze: &Freeze,
    base_directories: &BaseDirectories,
) -> io::Result<()> {
    log::info!("FREEZE...");

    let (tx, rx) = mpsc::channel();

    let mut watcher = match RecommendedWatcher::new(tx) {
        Ok(notify_watcher) => notify_watcher,
        Err(err) => {
            log::error!("notify watcher failed: {err:?}");
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot create notify watcher",
            ));
        }
    };

    let config = if let Some(config) = &freeze.config {
        if config.exists() {
            if let Err(err) = watcher.watch(config, RecursiveMode::NonRecursive) {
                log::error!("Cannot setup config watcher {config:?} failed: {err:?}");
                return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
            }
        }
        config.to_path_buf()
    } else {
        let default_config = base_directories.get_config_file("permafrust");
        if let Err(err) = watcher.watch(&default_config, RecursiveMode::NonRecursive) {
            log::error!("Cannot setup config watcher {default_config:?} failed: {err:?}");
            return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
        }
        default_config
    };

    log::trace!("Watching config {path}", path = config.display());

    let spool_dir = cli.base.as_ref();
    if let Err(err) = watcher.watch(spool_dir, RecursiveMode::Recursive) {
        log::error!("notify watcher {spool_dir:?} failed: {err:?}");
        return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
    }

    log::trace!("Watching spool {path}", path = cli.base.display());

    for res in rx {
        if let Err(err) = event_handler(res) {
            return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
        }
    }

    Ok(())
}

fn event_handler(result: Result<notify::Event, notify::Error>) -> Result<(), notify::Error> {
    match result {
        Ok(notify::Event { kind, paths, attrs }) => {
            log::debug!("Event: {kind:?} {paths:?} {attrs:?}")
        }
        Err(err) => {
            log::error!("watch error: {err:?}");
            return Err(err);
        }
    }
    Ok(())
}
