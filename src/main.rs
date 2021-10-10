mod backup;
mod freeze;
mod restore;
mod thaw;
use clap::{App, AppSettings, Arg, SubCommand};

pub struct Config {
    verbose: bool,
    quiet: bool,
}

fn main() {
    let matches = App::new("Permafrust")
        .setting(AppSettings::ArgRequiredElseHelp)
        .version("0.1.0")
        .author("Thomas Krennwallner <tk@postsubmeta.net>")
        .about("Glacier backup")
        .long_about("A backup and restore tool for AWS Glacier")
        .subcommand(
            SubCommand::with_name("backup")
                .about("Schedules input for backup")
                .arg(
                    Arg::with_name("input")
                        .short("i")
                        .long("input")
                        .takes_value(true)
                        .help("input file (default: stdin)"),
                ),
        )
        .subcommand(
            SubCommand::with_name("freeze")
                .about("Uploads backup chunks to glacier")
                .arg(
                    Arg::with_name("debug")
                        .short("d")
                        .long("debug")
                        .help("print debug information verbosely"),
                ),
        )
        .subcommand(
            SubCommand::with_name("thaw")
                .about("Downloads backup chunks from glacier")
                .arg(
                    Arg::with_name("debug")
                        .short("d")
                        .long("debug")
                        .help("print debug information verbosely"),
                ),
        )
        .subcommand(
            SubCommand::with_name("restore")
                .about("Restores backup chunks")
                .arg(
                    Arg::with_name("output")
                        .short("o")
                        .long("output")
                        .takes_value(true)
                        .help("output file (default: stdout)"),
                ),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .help("Verbose mode"),
        )
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .help("Quiet mode"),
        )
        .get_matches();

    let verbose = matches.is_present("verbose");
    let quiet = matches.is_present("quiet");

    let config = Config { verbose, quiet };

    match matches.subcommand() {
        ("backup", Some(m)) => {
            backup::perform_backup(config, m);
        }
        ("freeze", Some(m)) => {
            freeze::perform_freeze(config, m);
        }
        ("restore", Some(m)) => {
            restore::perform_restore(config, m);
        }
        ("thaw", Some(m)) => {
            thaw::perform_thaw(config, m);
        }
        _ => unreachable!(),
    }
}
