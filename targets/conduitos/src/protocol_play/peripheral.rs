//! Native SPI and logical contacts use the ordinary prepared protocol kernel.
use super::*;
use crate::{
    digital_base::{DigitalProvider, installation::ReadyDigitalBase},
    spi_base::{SpiProvider, installation::ReadySpiBase},
};
use alloc::{boxed::Box, vec::Vec};

pub struct SpiAdmission {
    pub placement: PlacementId,
    pub ready: ReadySpiBase<Box<dyn SpiProvider>>,
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
}
pub struct DigitalAdmission {
    pub placement: PlacementId,
    pub ready: ReadyDigitalBase<Box<dyn DigitalProvider>>,
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
}
pub struct PeripheralAdmission {
    pub spi: Option<SpiAdmission>,
    pub digital: Vec<DigitalAdmission>,
}
/// An absent I2C provider type for a protocol that selects only SPI and contacts.
/// No instance or I2C offer is constructed by this path.
pub enum NoI2cProvider {}
impl I2cProvider for NoI2cProvider {
    fn transact(
        &mut self,
        _: &crate::i2c_base::I2cTransaction<'_>,
        _: &mut [u8],
    ) -> Result<usize, crate::i2c_base::I2cDisposition> {
        match *self {}
    }
    fn revoke(&mut self) {
        match *self {}
    }
}
pub type PreparedPeripheralPlay = PreparedProtocolPlay<NoI2cProvider>;

impl<P: I2cProvider> PreparedProtocolPlay<P> {
    pub fn prepare_peripheral(
        definition: KernelCompositeDefinition,
        admission: PeripheralAdmission,
        operations: crate::protocol_operations::ProtocolOperations,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_peripheral_mode(
            definition,
            admission,
            operations,
            false,
            conduit_composite::KernelCompositeSignStorage::default(),
        )
    }
    pub(super) fn prepare_peripheral_mode(
        definition: KernelCompositeDefinition,
        admission: PeripheralAdmission,
        operations: crate::protocol_operations::ProtocolOperations,
        allow_clock: bool,
        sign_storage: conduit_composite::KernelCompositeSignStorage,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_selected(
            definition,
            operations,
            allow_clock,
            PhysicalMode::Peripheral,
            sign_storage,
            |plan, zip, states| {
                PreparedProtocolCalls::prepare_peripheral(plan, admission, zip, states)
            },
        )
    }
}
