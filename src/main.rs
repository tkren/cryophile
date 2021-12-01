use clap::{App, AppSettings, Arg, SubCommand};
use permafrust::constants::{DEFAULT_COMPRESSION, VERSION};
use std::process;

fn main() {
    let matches = App::new("Permafrust")
        .setting(AppSettings::ArgRequiredElseHelp)
        .version(env!("CARGO_PKG_VERSION"))
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
                )
                .arg(
                    Arg::with_name("vault")
                        .short("v")
                        .long("vault")
                        .takes_value(true)
                        .help("vault directory (under spool directory)"),
                )
                .arg(
                    Arg::with_name("output")
                        .short("o")
                        .long("output")
                        .takes_value(true)
                        .required(true)
                        .help("output directory (under vault directory)"),
                )
                .arg(
                    Arg::with_name("compression")
                        .short("C")
                        .long("compression")
                        .takes_value(true)
                        .required(false)
                        .default_value(DEFAULT_COMPRESSION.into())
                        .help("compress output using a supported algorithm (lz4, none, zstd)"),
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
        .arg(
            Arg::with_name("base")
                .short("b")
                .long("base")
                .takes_value(true)
                .help("Base directory"),
        )
        .get_matches();

    if let Err(err) = permafrust::run(&matches) {
        let code = match err {
            permafrust::CliError::IoError(ref e, code) => {
                log::error!("I/O Error: {}", e);
                code
            }
            permafrust::CliError::LogError(ref e, code) => {
                log::error!("Log Error: {}", e);
                code
            }
        };

        process::exit(code);
    }
}
