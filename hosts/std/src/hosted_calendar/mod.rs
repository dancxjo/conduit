//! Hosted calendar provider mechanisms below the portable calendar seam.

mod google_protocol;
mod google_response;
mod https_transport;

#[cfg(test)]
mod tests;

pub use google_protocol::*;
pub use google_response::{GoogleEventPage, GoogleFreeBusyPage, GoogleWriteReceipt};
pub use https_transport::GoogleHttpsTransport;
