use crate::cli::{Cli, Thaw};
use std::io;

pub fn perform_thaw(cli: &Cli, _thaw: &Thaw) -> io::Result<()> {
    log::info!("THAW...");
    if cli.debug > 0 {
        log::debug!("Printing verbose info...");
    } else if !cli.quiet {
        log::debug!("Printing normally...");
    }

    let debug = cli.debug > 0;
    if debug {
        log::debug!("We debug");
    }
    Ok(())
}
