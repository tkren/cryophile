use clap::{Arg, ArgMatches, Command};
use permafrust::{
    base_directory_profile,
    constants::{DEFAULT_CHUNK_SIZE, DEFAULT_COMPRESSION, DEFAULT_SPOOL_PATH},
    CliError, CliResult,
};
use std::path::PathBuf;

fn on_clap_error(err: clap::error::Error) -> ArgMatches {
    err.print().expect("Error writing error");

    let code: CliResult = match err.use_stderr() {
        true => CliResult::Usage,
        false => match err.kind() {
            clap::ErrorKind::DisplayHelp => CliResult::Ok,
            clap::ErrorKind::DisplayVersion => CliResult::Ok,
            clap::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => CliResult::Usage,
            _ => CliResult::Usage,
        },
    };

    // perform clap::util::safe_exit(code)
    use std::io::Write;

    let _ = std::io::stdout().lock().flush();
    let _ = std::io::stderr().lock().flush();

    std::process::exit(code as i32);
}

fn main() -> CliResult {
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

    if let Err(err) = permafrust::setup() {
        eprintln!("Cannot initialize permafrust: {err}");
        return CliResult::Abort;
    };

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
            return CliResult::ConfigError;
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
            CliError::BaseDirError(_e, code) => code,
            CliError::EnvError(_e, code) => code,
            CliError::IoError(_e, code) => code,
            CliError::LogError(_e, code) => code,
        };
        return code;
    }

    CliResult::Ok
}
