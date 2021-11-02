use crate::Config;
use std::io;

pub fn perform_thaw(config: Config, matches: &clap::ArgMatches) -> io::Result<()> {
    log::info!("THAW...");
    if config.verbose {
        log::debug!("Printing verbose info...");
    } else if !config.quiet {
        log::debug!("Printing normally...");
    }

    let debug = matches.is_present("debug");
    if debug {
        log::debug!("We debug");
    }
    Ok(())
}
