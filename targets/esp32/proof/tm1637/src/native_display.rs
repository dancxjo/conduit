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
#[cfg(feature = "display-ready-root")]
#[path = "native_display_assignment.rs"]
mod assignment;
#[cfg(feature = "display-ready-root")]
#[path = "native_display_kernel.rs"]
mod kernel;
#[cfg(feature = "display-ready-root")]
#[path = "display_layout.rs"]
mod layout;
mod software {
    include!(concat!(env!("OUT_DIR"), "/display-software.rs"));
}
pub struct NativeDisplay {
    #[cfg(feature = "display-ready-root")]
    staged: Option<assignment::StagedDisplayAssignment>,
    #[cfg(feature = "display-ready-root")]
    kernel: Option<kernel::Prepared>,
    ready: ReadyWriteBase<DisplayPins<'static>>,
    #[cfg(feature = "display-ready-root")]
    software: Result<
        Box<conduit_environment_runtime::appliance::software::ReadySoftwareBacks>,
        &'static str,
    >,
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
        // Validate the immutable definition at bootstrap, but do not retain its
        // decoded schema alongside the actual GPIO owner. Projection/binding
        // preparation can reconstruct it inside admitted control headroom.
        WriteContract::prepare().map_err(|_| "display-contract")?;
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
        #[cfg(feature = "display-ready-root")]
        let software = {
            // Exact trusted artifact, separate diagnostic Boot profile. Keep
            // 8 KiB control reserve beyond the conservative 96 KiB preparation
            // allowance; refuse without initialization when it is unavailable.
            let stats = esp_alloc::HEAP.stats();
            if stats.size.saturating_sub(stats.current_usage) < (96 + 8) * 1024 {
                Err("display-preparation-capacity")
            } else {
                let id = ready.native_identity();
                software::prepare(id.host_id.clone(), id.boot_id.clone())
                    .map(Box::new)
                    .map_err(|_| "display-software-initialization")
            }
        };
        Ok(Box::new(Self {
            #[cfg(feature = "display-ready-root")]
            staged: None,
            #[cfg(feature = "display-ready-root")]
            kernel: None,
            ready,
            #[cfg(feature = "display-ready-root")]
            software,
        }))
    }
    pub fn native_identity(&self) -> &WriteNativeIdentity {
        self.ready.native_identity()
    }
    pub fn software_ready(&self) -> bool {
        #[cfg(feature = "display-ready-root")]
        {
            self.software.is_ok()
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            false
        }
    }
    pub fn software_refusal(&self) -> Option<&'static str> {
        #[cfg(feature = "display-ready-root")]
        {
            self.software.as_ref().err().copied()
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            None
        }
    }
    pub fn current_software_offers(
        &mut self,
        host: &c::HostId,
        boot: &c::BootId,
        generation: c::OfferGeneration,
    ) -> Result<conduit_environment_runtime::appliance::software::ReadySoftwareOffers<'_>, ()> {
        #[cfg(feature = "display-ready-root")]
        {
            self.software
                .as_mut()
                .map_err(|_| ())?
                .current_offers(host, boot, generation)
                .map_err(|_| ())
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            let _ = (host, boot, generation);
            Err(())
        }
    }
    pub fn append_to_advertisement(&self, host: &mut c::HostAdvertisement) -> Result<(), ()> {
        let contract = WriteContract::prepare().map_err(|_| ())?;
        self.ready
            .append_to_advertisement(host, &contract)
            .map_err(|_| ())
    }
}

#[cfg(feature = "display-ready-root")]
impl NativeDisplay {
    pub fn stage_assignment(
        &mut self,
        wire: &[u8],
        scope: conduit_environment_runtime::display_output::AssignedDisplayScope<'_>,
        available: usize,
    ) -> Result<c::AssignedPlanView, &'static str> {
        if self.staged.is_some() {
            return Err("display-already-staged");
        }
        if !self.software_ready() {
            return Err("display-software-unavailable");
        }
        let source = layout::source();
        let fore = layout::fore();
        let profile = layout::profile(&source, &fore).map_err(|_| "display-provisioning")?;
        let staged = assignment::StagedDisplayAssignment::stage(
            wire,
            &profile,
            &layout::configurations(),
            scope,
            available,
        )?;
        let identity = staged.admitted.identity();
        self.staged = Some(staged);
        Ok(identity)
    }
    pub fn assignment(&self) -> Option<&assignment::StagedDisplayAssignment> {
        self.staged.as_ref()
    }
}

impl NativeDisplay {
    pub fn kernel_prepared(&self) -> bool {
        #[cfg(feature = "display-ready-root")]
        {
            self.kernel.is_some()
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            false
        }
    }
}
#[cfg(feature = "display-ready-root")]
impl NativeDisplay {
    pub fn prepare_kernel(
        &mut self,
        scope: conduit_environment_runtime::display_output::AssignedDisplayScope<'_>,
        maximum_frames: u32,
        available: usize,
    ) -> Result<(), &'static str> {
        if self.kernel.is_some() {
            return Err("display-already-prepared");
        }
        let staged = self
            .staged
            .as_ref()
            .ok_or("display-assignment-unavailable")?;
        let source = layout::source();
        let fore = layout::fore();
        let profile = layout::profile(&source, &fore).map_err(|_| "display-provisioning")?;
        self.kernel = Some(kernel::prepare(
            staged.wire(),
            &profile,
            &layout::configurations(),
            scope,
            self.software
                .as_mut()
                .map_err(|_| "display-software-unavailable")?,
            maximum_frames,
            available,
        )?);
        Ok(())
    }
}

#[cfg(feature = "display-ready-root")]
pub fn kernel_preparation_bound() -> Result<usize, &'static str> {
    kernel::requirements()
}
