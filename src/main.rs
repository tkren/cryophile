// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use clap::Parser;
use permafrust::{
    cli::{Cli, CliResult},
    on_clap_error,
};

fn main() -> CliResult {
    let cli = Cli::try_parse().unwrap_or_else(on_clap_error);

    if let Err(err) = permafrust::setup(cli.debug, cli.quiet) {
        err.into()
    } else {
        permafrust::run(cli).unwrap_or_else(|err| err.into())
    }
}
