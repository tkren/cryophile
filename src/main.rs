use clap::Parser;
use permafrust::{cli::Cli, CliError, CliResult, Config};

fn on_clap_error(err: clap::error::Error) -> Cli {
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
    let cli = Cli::try_parse().unwrap_or_else(on_clap_error);

    if let Err(err) = permafrust::setup(cli.debug, cli.quiet) {
        eprintln!("Cannot initialize permafrust: {err}");
        return CliResult::Abort;
    };

    permafrust::log_versions();

    let subcommand = cli.command.to_string();
    let base_directories = permafrust::base_directory_profile(&subcommand).unwrap();

    let config = Config {
        base: base_directories,
        cli,
    };

    if let Err(err) = permafrust::run(&config) {
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
