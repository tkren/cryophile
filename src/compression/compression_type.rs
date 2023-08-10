// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum CompressionType {
    #[default]
    None,
    Lz4,
    Zstd,
}
