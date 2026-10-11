//! Install the exact finite physical operations selected during preparation.
use super::{I2cOperationFactory, KernelOperationRegistry, PhysicalMode, ProtocolCallRefusal};

pub(super) fn install_physical(
    registry: &mut KernelOperationRegistry,
    mode: PhysicalMode,
) -> Result<(), ProtocolCallRefusal> {
    match mode {
        PhysicalMode::I2c => {
            registry
                .install(
                    I2cOperationFactory::prepare_contract()
                        .map_err(|_| ProtocolCallRefusal::InvalidPlan)?,
                )
                .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        }
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
        ))]
        PhysicalMode::UsbControl
        | PhysicalMode::UsbProtocol
        | PhysicalMode::UsbDuplex
        | PhysicalMode::StorageUsbDuplex
        | PhysicalMode::UsbIsochronous => {
            registry
                .install(
                    crate::usb_base::control_factory::ControlOperationFactory::prepare_contract()
                        .map_err(|_| ProtocolCallRefusal::InvalidPlan)?,
                )
                .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
            if matches!(
                mode,
                PhysicalMode::UsbProtocol
                    | PhysicalMode::UsbDuplex
                    | PhysicalMode::StorageUsbDuplex
            ) {
                registry.install(crate::usb_base::endpoint_read_factory::EndpointReadOperationFactory::prepare_contract().map_err(|_| ProtocolCallRefusal::InvalidPlan)?).map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
            }
            if matches!(mode, PhysicalMode::UsbIsochronous) {
                registry.install(crate::usb_base::isochronous::factory::IsochronousOutOperationFactory::prepare_contract().map_err(|_| ProtocolCallRefusal::InvalidPlan)?).map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
            }
            if matches!(
                mode,
                PhysicalMode::UsbDuplex | PhysicalMode::StorageUsbDuplex
            ) {
                registry.install(crate::usb_base::endpoint_write_factory::EndpointWriteOperationFactory::prepare_contract().map_err(|_| ProtocolCallRefusal::InvalidPlan)?).map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
            }
        }
    }
    registry
        .install(SelectorOperationFactory::default())
        .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    if matches!(mode, PhysicalMode::StorageUsbDuplex) {
        use crate::storage_base::transaction_factory::*;
        for role in [
            StorageTransactionRole::Authorize,
            StorageTransactionRole::Settle,
        ] {
            registry
                .install(
                    StorageTransactionOperationFactory::prepare(role)
                        .map_err(|_| ProtocolCallRefusal::InvalidPlan)?,
                )
                .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
        }
    }
    Ok(())
}
