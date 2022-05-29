use crate::cli::{Cli, Freeze};
use notify::{RecursiveMode, Watcher};
use std::io;
use std::sync::mpsc;
use xdg::BaseDirectories;

pub fn perform_freeze(
    cli: &Cli,
    _freeze: &Freeze,
    base_directories: &BaseDirectories,
) -> io::Result<()> {
    log::info!("FREEZE...");
    if cli.debug > 0 {
        log::debug!("Printing verbose info...");
    } else if !cli.quiet {
        log::debug!("Printing normally...");
    }

    let debug = cli.debug > 0;
    if debug {
        log::debug!("We debug");
    }

    let state_home = base_directories.get_state_home();

    let (tx, rx) = mpsc::channel();

    let mut watcher = match notify::raw_watcher(tx) {
        Ok(notify_watcher) => notify_watcher,
        Err(err) => {
            log::error!("notify watcher failed: {err:?}");
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Cannot create notify watcher",
            ));
        }
    };

    if let Err(err) = watcher.watch(state_home, RecursiveMode::Recursive) {
        log::error!("notify watcher failed: {err:?}");
        return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
    }

    loop {
        let result = rx.recv();
        if let Err(err) = event_handler(result) {
            return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
        }
    }
}

fn event_handler(result: Result<notify::RawEvent, mpsc::RecvError>) -> Result<(), mpsc::RecvError> {
    match result {
        Ok(notify::RawEvent {
            path: Some(path),
            op: Ok(notify::Op::CREATE),
            cookie,
        }) => {
            log::debug!("CREATE event: {path:?} {cookie:?}")
        }
        Ok(notify::RawEvent {
            path: Some(path),
            op: Ok(notify::Op::RENAME),
            cookie,
        }) => {
            log::debug!("RENAME event: {path:?} {cookie:?}")
        }
        Ok(notify::RawEvent {
            path: Some(path),
            op: Ok(notify::Op::CLOSE_WRITE),
            cookie,
        }) => {
            log::debug!("CLOSE_WRITE event: {path:?} {cookie:?}")
        }
        Ok(notify::RawEvent {
            path: Some(path),
            op: Ok(op),
            cookie,
        }) => {
            log::trace!("ignored event: {op:?}({path:?}) {cookie:?}")
        }
        Ok(broken_event) => {
            log::error!("broken event: {broken_event:?}");
        }
        Err(err) => {
            log::error!("watch error: {err:?}");
            return Err(err);
        }
    }
    Ok(())
}
