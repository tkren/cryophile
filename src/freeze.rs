use crate::Config;
use std::io;

pub fn perform_freeze(config: Config, matches: &clap::ArgMatches) -> io::Result<()> {
    println!("FREEZE...");
    if config.verbose {
        println!("Printing verbose info...");
    } else if !config.quiet {
        println!("Printing normally...");
    }

    let debug = matches.is_present("debug");
    if debug {
        println!("We debug");
    }
    Ok(())
}
