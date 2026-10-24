//! Actual Root-owned output pins. Readiness is not an admitted Mask Play,
//! authority to transmit, or evidence of physical display delivery.
use crate::{boot::ObservedBoot, electrical::DisplayPins};
use alloc::boxed::Box;
use conduit_core as c;
use conduit_tm1637_wire::{
    installation::{ReadyWriteBase, WriteNativeIdentity},
    owner::WriteAttachment,
    WriteContract,
};
use esp_hal::gpio::Pin;
pub struct NativeDisplay {
    ready: ReadyWriteBase<DisplayPins<'static>>,
    contract: WriteContract,
}
impl NativeDisplay {
    /// Consume the two actual initialized MCU peripherals before offering them.
    /// The existing electrical adapter releases both lines on drop. Its finite
    /// synchronous transaction time still needs independent Play admission.
    pub fn prepare(
        clock: impl Pin + 'static,
        data: impl Pin + 'static,
        boot: &ObservedBoot,
    ) -> Result<Box<Self>, &'static str> {
        let (host, boot) = boot.ids();
        let contract = WriteContract::prepare().map_err(|_| "display-contract")?;
        let identity = WriteNativeIdentity {
            host_id: host.into(),
            boot_id: boot.into(),
            base_id: "base/esp32/gpio18-gpio19".into(),
            provider_instance_id: "instance/esp32/tm1637".into(),
            provider_generation: 1,
            resource_pool_id: "pool/esp32/gpio18-gpio19".into(),
            resource_generation_id: c::ResourceGenerationId("generation/tm1637/1".into()),
            envelope_id: "envelope/esp32/tm1637".into(),
            artifact_id: "native/esp32-tm1637-root@1".into(),
            implementation_id: "esp32/tm1637-write@1".into(),
            base_implementation_id: "esp32/tm1637-base@1".into(),
        };
        // SAFETY: the native composition root transfers the actual GPIO18/19
        // peripherals here. DisplayPins owns their open-drain realization and
        // fixed half-period; no probe, received assignment or descriptor creates
        // possession. This initialization performs no packet transmission.
        let ready = unsafe {
            ReadyWriteBase::new(
                identity,
                WriteAttachment {
                    generation: 1,
                    resource_bytes: 64,
                },
                DisplayPins::new(clock, data),
            )
        }
        .map_err(|_| "display-provider")?;
        Ok(Box::new(Self { ready, contract }))
    }
    pub fn append_to_advertisement(&self, host: &mut c::HostAdvertisement) -> Result<(), ()> {
        self.ready
            .append_to_advertisement(host, &self.contract)
            .map_err(|_| ())
    }
}
