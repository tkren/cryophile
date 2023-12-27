// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use crate::{cli::Thaw, Config};
use std::io;

pub fn perform_thaw(_config: &Config, _thaw: &Thaw) -> io::Result<()> {
    log::info!("THAW…");

    Ok(())
}
