use crate::Config;
use std::io;

pub fn perform_restore(config: Config, matches: &clap::ArgMatches) -> io::Result<()> {
    log::info!("RESTORE...");
    if config.verbose {
        log::debug!("Printing verbose info...");
    } else if !config.quiet {
        log::debug!("Printing normally...");
    }

    let output = matches.value_of("output").unwrap_or("-");
    log::info!("The output file passed is: {output:?}");
    Ok(())
}
