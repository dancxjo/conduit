//! Classify errors at the actual byte carrier boundary. A filesystem timeout,
//! rejected command or invalid signature is not a reconnectable transport loss.
use std::{error::Error, fmt, io};
#[derive(Debug)]
struct CarrierError(io::Error);
impl fmt::Display for CarrierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for CarrierError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}
pub(super) fn io(error: io::Error) -> Box<dyn Error> {
    Box::new(CarrierError(error))
}
pub(super) fn closed(message: &'static str) -> Box<dyn Error> {
    io(io::Error::new(io::ErrorKind::UnexpectedEof, message))
}
pub(super) fn timeout() -> Box<dyn Error> {
    io(io::Error::new(
        io::ErrorKind::TimedOut,
        "owner carrier deadline",
    ))
}
pub fn retryable(error: &(dyn Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if super::fault::is_mcu_fault(error) {
            return false;
        }
        if let Some(error) = error.downcast_ref::<CarrierError>() {
            use io::ErrorKind::*;
            return matches!(
                error.0.kind(),
                ConnectionRefused
                    | ConnectionReset
                    | ConnectionAborted
                    | NotConnected
                    | BrokenPipe
                    | TimedOut
                    | UnexpectedEof
                    | WouldBlock
                    | WriteZero
                    | NetworkDown
                    | NetworkUnreachable
                    | HostUnreachable
                    | AddrNotAvailable
            );
        }
        current = error.source();
    }
    false
}
