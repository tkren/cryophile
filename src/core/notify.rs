use std::io;

pub fn notify_error(e: notify::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, format!("Notify error: {e}"))
}
