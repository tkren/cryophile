// Copyright The Permafrust Authors.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE> or
// <http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT> or <http://opensource.org/licenses/MIT>, at your option.
//
// This file may not be copied, modified, or distributed except according
// to those terms.

use std::io;
use std::sync::mpsc::{RecvError, SendError};

pub fn channel_send_error<T>(e: SendError<T>) -> io::Error {
    io::Error::other(format!("Channel send error: {e}"))
}

pub fn channel_recv_error(e: RecvError) -> io::Error {
    io::Error::other(format!("Channel recv error: {e}"))
}
