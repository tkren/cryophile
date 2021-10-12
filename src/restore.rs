use crate::Config;
use std::io;

pub fn perform_restore(config: Config, matches: &clap::ArgMatches) -> io::Result<()> {
    println!("RESTORE...");
    if config.verbose {
        println!("Printing verbose info...");
    } else if !config.quiet {
        println!("Printing normally...");
    }

    let output = matches.value_of("output").unwrap_or("-");
    println!("The output file passed is: {}", output);
    Ok(())
}
