use crate::cli::{Cli, Freeze};
use crate::config::ConfigFile;
use crate::core::aws;
use crate::core::notify::notify_error;
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
        log::warn!(
            "Configuration {path} does not exist, ignoring configuration updates",
            path = config_path.display()
        );
        None
    };

    log::trace!("Config: {config:#?}");

    let spool = cli.spool.as_ref();
    watcher
        .watch(spool, RecursiveMode::Recursive)
        .map_err(notify_error)?;
    log::trace!("Watching spool {path}", path = spool.display());

    for res in rx {
        event_handler(res).map_err(notify_error)?
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
