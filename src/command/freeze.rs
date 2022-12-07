use crate::cli::{Cli, Freeze};
use crate::config::ConfigFile;
use crate::core::aws;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::io;
use std::sync::mpsc;
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

    let (tx, rx) = mpsc::channel();

    let mut watcher = match RecommendedWatcher::new(tx, notify::Config::default()) {
        Ok(notify_watcher) => notify_watcher,
        Err(err) => {
            log::error!("notify watcher failed: {err:?}");
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot create notify watcher",
            ));
        }
    };

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
        log::warn!(
            "Configuration {path} does not exist, ignoring configuration updates",
            path = config_path.display()
        );
        None
    };

    log::trace!("Config: {config:#?}");

    let spool = cli.spool.as_ref();
    if let Err(err) = watcher.watch(spool, RecursiveMode::Recursive) {
        log::error!("notify watcher {spool:?} failed: {err:?}");
        return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
    }
    log::trace!("Watching spool {path}", path = spool.display());

    for res in rx {
        if let Err(err) = event_handler(res, &config) {
            return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
        }
    }

    Ok(())
}

fn event_handler(
    result: Result<notify::Event, notify::Error>,
    config: &Option<ConfigFile>,
) -> Result<(), notify::Error> {
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
