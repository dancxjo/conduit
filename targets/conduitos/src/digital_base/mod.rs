//! Generic single-contact digital operations; physical pin selection is Host truth.

pub mod bcm2835;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigitalRequest {
    Observe,
    Drive(bool),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigitalDisposition {
    Unsupported,
    ProviderLost,
    Refused,
}

/// One exclusively selected digital contact, with immutable direction.
/// Success returns the actual observed level, including after a drive operation.
/// Requests contain neither pin numbers nor a way to change direction.
pub trait DigitalProvider {
    fn access(&mut self, request: DigitalRequest) -> Result<bool, DigitalDisposition>;
    fn revoke(&mut self);
}

pub mod contract;
pub mod decode;
mod factory;
pub mod installation;
pub mod owner;
pub mod result;
pub use factory::DigitalOperationFactory;

#[cfg(test)]
mod tests;
