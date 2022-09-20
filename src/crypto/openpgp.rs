use sequoia_openpgp as openpgp;
use std::io;

use openpgp::{
    cert::prelude::ValidKeyAmalgamation,
    packet::key::{PublicParts, UnspecifiedRole},
};

pub type Keyring<'a> = Vec<ValidKeyAmalgamation<'a, PublicParts, UnspecifiedRole, bool>>;

pub fn openpgp_error(e: anyhow::Error) -> io::Error {
    let e = match e.downcast::<io::Error>() {
        Ok(e) => return e,
        Err(e) => e,
    };
    io::Error::new(io::ErrorKind::Other, format!("OpenPGP error: {e}"))
}
