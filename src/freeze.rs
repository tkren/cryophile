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
        config.to_path_buf()
    } else {
        let default_config = base_directories.get_config_file("permafrust.toml");
        default_config
    };

    if config.exists() {
        if let Err(err) = watcher.watch(&config, RecursiveMode::NonRecursive) {
            log::error!("Cannot setup config watcher {config:?} failed: {err:?}");
            return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
        }
        log::trace!("Watching config {path}", path = config.display());
    } else {
        log::warn!(
            "Configuration {path} does not exist, ignoring configuration updates",
            path = config.display()
        );
    }

    let spool = cli.spool.as_ref();
    if let Err(err) = watcher.watch(spool, RecursiveMode::Recursive) {
        log::error!("notify watcher {spool:?} failed: {err:?}");
        return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
    }
    log::trace!("Watching spool {path}", path = spool.display());

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
