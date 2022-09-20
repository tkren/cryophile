use crate::cli::{Cli, Thaw};
use std::io;

pub fn perform_thaw(_cli: &Cli, _thaw: &Thaw) -> io::Result<()> {
    log::info!("THAW...");

    Ok(())
}
