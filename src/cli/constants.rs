// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::sync::LazyLock;

use regex::Regex;

pub const DEFAULT_CHUNK_SIZE: usize = 512;

pub static DEFAULT_SPOOL_PATH: &str = "/var/spool/permafrust";

pub static DEFAULT_CONFIG_PATH: &str = "/etc/permafrust/permafrust.toml";

pub static UNSAFE_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"[^a-zA-Z0-9[/()!'*._-]]+"#).expect("broken regex"));
