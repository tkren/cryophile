use crate::cli::{Cli, Restore};
use std::{fs, io, path::Path};

pub fn perform_restore(_cli: &Cli, restore: &Restore) -> io::Result<()> {
    log::info!("RESTORE...");

    let _output: Box<dyn io::Write> = match &restore.output {
        Some(p) if p.as_path() == Path::new("-") => {
            log::info!("Writing to stdout ...");
            Box::new(io::stdout())
        }
        None => {
            log::info!("Writing to stdout ...");
            Box::new(io::stdout())
        }
        Some(output) => {
            log::info!("Opening {output:?} ...");
            Box::new(fs::File::open(output)?)
        }
    };

    Ok(())
}
