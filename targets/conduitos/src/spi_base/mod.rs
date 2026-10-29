//! Peripheral-agnostic SPI mechanisms and explicitly admitted operation backs.

pub mod bcm2835;

pub mod contract;
pub mod transaction;
pub use transaction::{SpiDisposition, SpiProvider, SpiTransaction};
pub mod decode;
pub mod result;

#[cfg(test)]
mod tests;

mod factory;
pub mod installation;
pub mod owner;
pub use factory::SpiOperationFactory;

impl<P: SpiProvider + ?Sized> SpiProvider for alloc::boxed::Box<P> {
    fn exchange(
        &mut self,
        request: &SpiTransaction<'_>,
        input: &mut [u8],
    ) -> Result<(), SpiDisposition> {
        (**self).exchange(request, input)
    }
    fn revoke(&mut self) {
        (**self).revoke();
    }
}

mod provider_guard;
