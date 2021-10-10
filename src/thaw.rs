use crate::Config;

pub fn perform_thaw(config: Config, matches: &clap::ArgMatches) {
    println!("THAW...");
    if config.verbose {
        println!("Printing verbose info...");
    } else if !config.quiet {
        println!("Printing normally...");
    }

    let debug = matches.is_present("debug");
    if debug {
        println!("We debug");
    }
}
