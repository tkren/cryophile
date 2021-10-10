use crate::Config;

pub fn perform_backup(config: Config, matches: &clap::ArgMatches) {
    println!("BACKUP...");
    if config.verbose {
        println!("Printing verbose info...");
    } else if !config.quiet {
        println!("Printing normally...");
    }

    let input = matches.value_of("input").unwrap_or("-");
    println!("The input file passed is: {}", input);
}
