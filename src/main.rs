use clap::{Arg, Command};
use permafrust::{
    base_directory_profile,
    constants::{DEFAULT_COMPRESSION, DEFAULT_SPOOL_PATH},
};
use std::{path::PathBuf, process};

fn main() {
    let matches = clap::command!()
        .subcommand_required(true)
        .arg_required_else_help(true)
        .long_about(clap::crate_description!())
        .subcommand(
            Command::new("backup")
                .about("Schedules input for backup")
                .arg(
                    Arg::new("input")
                        .short('i')
                        .long("input")
                        .takes_value(true)
                        .help("input file (default: stdin)"),
                )
                .arg(
                    Arg::new("vault")
                        .short('v')
                        .long("vault")
                        .takes_value(true)
                        .help("vault directory (under spool directory)"),
                )
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .takes_value(true)
                        .required(true)
                        .help("output directory (under vault directory)"),
                )
                .arg(
                    Arg::new("compression")
                        .short('C')
                        .long("compression")
                        .takes_value(true)
                        .required(false)
                        .default_value(DEFAULT_COMPRESSION.into())
                        .help("compress output using a supported algorithm (lz4, none, zstd)"),
                ),
        )
        .subcommand(
            Command::new("freeze")
                .about("Uploads backup chunks to glacier")
                .arg(
                    Arg::new("debug")
                        .short('d')
                        .long("debug")
                        .help("print debug information verbosely"),
                ),
        )
        .subcommand(
            Command::new("thaw")
                .about("Downloads backup chunks from glacier")
                .arg(
                    Arg::new("debug")
                        .short('d')
                        .long("debug")
                        .help("print debug information verbosely"),
                ),
        )
        .subcommand(
            Command::new("restore").about("Restores backup chunks").arg(
                Arg::new("output")
                    .short('o')
                    .long("output")
                    .takes_value(true)
                    .help("output file (default: stdout)"),
            ),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Verbose mode"),
        )
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Quiet mode"),
        )
        .arg(
            Arg::new("base")
                .short('b')
                .long("base")
                .takes_value(true)
                .default_value(DEFAULT_SPOOL_PATH)
                .help("Base directory containing all backup and restore state"),
        )
        .get_matches();

    let (subcommand, submatches) = match matches.subcommand() {
        Some((sc, m)) => (sc, m),
        _ => unreachable!(),
    };

    let base_directories = base_directory_profile(subcommand).unwrap();

    let config = permafrust::Config {
        base: base_directories,
        spool: PathBuf::from(matches.value_of("base").unwrap()),
        verbose: matches.is_present("verbose"),
        quiet: matches.is_present("quiet"),
    };

    if let Err(err) = permafrust::run(config, subcommand, submatches) {
        log::error!("{err}");
        let code = match err {
            permafrust::CliError::BaseDirError(_e, _code) => exitcode::CONFIG,
            permafrust::CliError::EnvError(_e, _code) => exitcode::CONFIG,
            permafrust::CliError::IoError(_e, _code) => exitcode::IOERR,
            permafrust::CliError::LogError(_e, _code) => exitcode::SOFTWARE,
        };
        process::exit(code);
    }

    process::exit(exitcode::OK);
}
