//! Hosted calendar provider mechanisms below the portable calendar seam.

mod google_protocol;
mod https_transport;

#[cfg(test)]
mod tests;

pub use google_protocol::*;
pub use https_transport::GoogleHttpsTransport;
