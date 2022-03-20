use clap::{Arg, ArgMatches, Command};
use permafrust::{
    base_directory_profile,
    constants::{DEFAULT_CHUNK_SIZE, DEFAULT_COMPRESSION, DEFAULT_SPOOL_PATH},
};
use std::{path::PathBuf, process};

fn on_clap_error(err: clap::error::Error) -> ArgMatches {
    err.print().expect("Error writing error");

    let code = match err.use_stderr() {
        true => exitcode::USAGE,
        false => match err.kind() {
            clap::ErrorKind::DisplayHelp => exitcode::OK,
            clap::ErrorKind::DisplayVersion => exitcode::OK,
            clap::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => exitcode::USAGE,
            _ => exitcode::USAGE,
        },
    };

    // perform clap::util::safe_exit(code)
    use std::io::Write;

    let _ = std::io::stdout().lock().flush();
    let _ = std::io::stderr().lock().flush();

    std::process::exit(code);
}

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
                    Arg::new("size")
                        .short('s')
                        .long("size")
                        .takes_value(true)
                        .required(false)
                        .default_value(DEFAULT_CHUNK_SIZE)
                        .help("default chunk size"),
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
        .try_get_matches()
        .unwrap_or_else(on_clap_error);

    let (subcommand, submatches) = match matches.subcommand() {
        Some((sc, m)) => (sc, m),
        _ => unreachable!(),
    };

    let base_directories = base_directory_profile(subcommand).unwrap();

    let parse_config = parse_size::Config::new()
        .with_binary()
        .with_byte_suffix(parse_size::ByteSuffix::Deny);

    let chunk_size: &str = if subcommand == "backup" {
        submatches.value_of("size").unwrap()
    } else {
        DEFAULT_CHUNK_SIZE
    };

    let parse_size_result = parse_config.parse_size(chunk_size);

    let chunk_size = match parse_size_result {
        Ok(n) => usize::try_from(n).expect("size exceeds usize"),
        Err(err) => {
            log::error!("Cannot parse chunk size option: {err}");
            use std::io::Write;
            let _ = std::io::stderr().lock().flush();
            process::exit(exitcode::CONFIG);
        }
    };

    log::trace!("Setting backup chunk size to {chunk_size}");

    let config = permafrust::Config {
        base: base_directories,
        chunk_size,
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
