// Copyright The Cryophile Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use clap::Parser;
use cryophile::{
    cli::{Cli, CliResult},
    on_clap_error,
};

fn main() -> CliResult {
    let cli = Cli::try_parse().unwrap_or_else(on_clap_error);
    cryophile::setup(cli.debug, cli.quiet)
        .and_then(|_| cryophile::run(cli))
        .map_err(Into::<CliResult>::into)
        .unwrap_or_else(std::convert::identity) // returns contained CliResult value from `Ok` or `Err`
}
