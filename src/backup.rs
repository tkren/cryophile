use crate::Config;
use std::fs;
use std::io;
use std::io::Read;

pub fn perform_backup(config: Config, matches: &clap::ArgMatches) -> io::Result<()> {
    log::info!("BACKUP...");
    if config.verbose {
        log::debug!("Printing verbose info...");
    } else if !config.quiet {
        log::debug!("Printing normally...");
    }

    let input = matches.value_of("input").unwrap_or("-");

    let reader: Box<dyn io::Read> = match input {
        "-" => {
            log::info!("Reading from stdin ...");
            Box::new(io::stdin())
        }
        _ => {
            log::info!("Opening `{}' ...", input);
            Box::new(fs::File::open(input)?)
        }
    };

    splitter(reader)
}

pub fn splitter(reader: Box<dyn io::Read>) -> io::Result<()> {
    let mut buffered_reader = io::BufReader::new(reader);
    let mut buffer = [0; 4096];

    loop {
        let result = buffered_reader.read(&mut buffer);

        match result {
            Ok(n) => {
                if 0 < n && n <= buffer.len() {
                    println!("Got {:?} {:?}", n, &buffer[..n])
                } else if n == 0 {
                    // Eof
                    println!("EOF");
                    break;
                } else {
                    eprintln!("Received {} buff??", n);
                    return Err(io::Error::new(io::ErrorKind::Other, "oh no!"));
                }
            }
            Err(err) => {
                let error_kind = err.kind();
                if error_kind == io::ErrorKind::Interrupted {
                    eprintln!("Retry");
                    continue;
                } else if error_kind == io::ErrorKind::UnexpectedEof {
                    println!("UnexpectedEof");
                    break;
                }
                return Err(err);
            }
        }
    }

    Ok(())
}
