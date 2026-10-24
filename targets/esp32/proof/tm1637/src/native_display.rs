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
#[path = "native_display_binding.rs"]
mod binding;
#[cfg(feature = "display-ready-root")]
#[path = "native_display_execution.rs"]
mod execution;
#[cfg(feature = "display-ready-root")]
#[path = "native_display_lifecycle.rs"]
mod lifecycle;
#[cfg(feature = "display-ready-root")]
#[path = "native_display_task.rs"]
mod task;
#[cfg(feature = "display-ready-root")]
pub(crate) use task::Status as DisplayTaskStatus;
#[cfg(feature = "display-ready-root")]
#[path = "native_display_kernel.rs"]
mod kernel;
#[cfg(feature = "display-ready-root")]
#[path = "display_layout.rs"]
mod layout;
#[cfg(feature = "display-ready-root")]
#[path = "native_display_service.rs"]
mod service;
mod software {
    include!(concat!(env!("OUT_DIR"), "/display-software.rs"));
}
pub struct NativeDisplay {
    #[cfg(feature = "display-ready-root")]
    staged: Option<assignment::StagedDisplayAssignment>,
    #[cfg(feature = "display-ready-root")]
    kernel: Option<kernel::Prepared>,
    #[cfg(feature = "display-ready-root")]
    phase: &'static str,
    #[cfg(feature = "display-ready-root")]
    worker: Option<task::Handle>,
    #[cfg(feature = "display-ready-root")]
    execution: Option<Box<execution::Execution<DisplayPins<'static>>>>,
    #[cfg(feature = "display-ready-root")]
    retired: Option<crate::native_retirement::RetiredDisplay<DisplayPins<'static>>>,
    ready: Option<ReadyWriteBase<DisplayPins<'static>>>,
    #[cfg(feature = "display-ready-root")]
    bound: Option<Box<conduit_tm1637_wire::owner::WriteHostCall<DisplayPins<'static>>>>,
    #[cfg(feature = "display-ready-root")]
    bound_write: Option<conduit_kernel::NodeId>,
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
            #[cfg(feature = "display-ready-root")]
            phase: "unprepared",
            #[cfg(feature = "display-ready-root")]
            worker: None,
            #[cfg(feature = "display-ready-root")]
            execution: None,
            #[cfg(feature = "display-ready-root")]
            retired: None,
            ready: Some(ready),
            #[cfg(feature = "display-ready-root")]
            bound: None,
            #[cfg(feature = "display-ready-root")]
            bound_write: None,
            #[cfg(feature = "display-ready-root")]
            software,
        }))
    }
    pub fn native_identity(&self) -> Option<&WriteNativeIdentity> {
        self.ready.as_ref().map(|owner| owner.native_identity())
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
        let Some(ready) = self.ready.as_ref() else {
            return Ok(());
        };
        let contract = WriteContract::prepare().map_err(|_| ())?;
        ready
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
            self.kernel.is_some() || self.context_bytes().is_some()
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
    kernel::requirements(layout::MAXIMUM_VALUE_BYTES)
}

impl NativeDisplay {
    pub fn native_bound(&self) -> bool {
        #[cfg(feature = "display-ready-root")]
        {
            self.bound.is_some()
                || self.execution.is_some()
                || self
                    .worker
                    .as_ref()
                    .is_some_and(|w| w.status().native_custody)
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            false
        }
    }
}
#[cfg(feature = "display-ready-root")]
impl NativeDisplay {
    pub fn bind_native(
        &mut self,
        issuer: &mut crate::native_authority::RootIssuer,
        reservation: &crate::native_owner_reservation::OwnerReservation,
        maximum_frames: u32,
        available: usize,
    ) -> Result<(), &'static str> {
        if self.bound.is_some() || self.execution.is_some() || self.worker.is_some() {
            return Err("display-binding-state");
        }
        let staged = self
            .staged
            .as_ref()
            .ok_or("display-assignment-unavailable")?;
        if staged.seal() != reservation.assignment_seal {
            return Err("assignment-seal");
        }
        let source = layout::source();
        let fore = layout::fore();
        let profile = layout::profile(&source, &fore).map_err(|_| "display-provisioning")?;
        self.bound = Some(binding::bind(
            issuer,
            &mut self.ready,
            binding::BindingInput {
                assignment: &staged.admitted,
                wire: staged.wire(),
                profile: &profile,
                resident: &layout::configurations(),
                fragment: &reservation.fragment,
                active: &reservation.active,
                maximum_frames,
                available_bytes: available,
            },
        )?);
        self.bound_write = Some(profile.write);
        self.phase = "ready-held";
        Ok(())
    }
}

impl NativeDisplay {
    pub fn context_bytes(&self) -> Option<usize> {
        #[cfg(feature = "display-ready-root")]
        {
            self.execution
                .as_ref()
                .map(|_| core::mem::size_of::<execution::Execution<DisplayPins<'static>>>())
                .or_else(|| {
                    self.worker
                        .as_ref()
                        .filter(|w| w.status().native_custody)
                        .map(|w| w.status().context_bytes)
                })
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            None
        }
    }
}
#[cfg(feature = "display-ready-root")]
impl NativeDisplay {
    pub fn prepare_context(&mut self, available: usize) -> Result<(), &'static str> {
        if self.execution.is_some() {
            return Err("display-context-already-prepared");
        }
        if available < core::mem::size_of::<execution::Execution<DisplayPins<'static>>>() + 8192 {
            return Err("display-context-capacity");
        }
        self.execution = Some(execution::Execution::boxed_retained(
            &mut self.kernel,
            &mut self.bound,
        )?);
        self.bound_write = None;
        Ok(())
    }
}

impl NativeDisplay {
    pub fn task_registered(&self) -> bool {
        #[cfg(feature = "display-ready-root")]
        {
            self.worker.is_some()
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            false
        }
    }
}
#[cfg(feature = "display-ready-root")]
impl NativeDisplay {
    pub fn task_status(&self) -> Option<DisplayTaskStatus> {
        self.worker.as_ref().map(|w| w.status())
    }
    pub async fn register_task(
        &mut self,
        spawner: embassy_executor::Spawner,
    ) -> Result<(), &'static str> {
        if self.worker.is_some() {
            return Err("display-task-already-registered");
        }
        self.worker = Some(task::prepare(&mut self.execution, spawner)?);
        let worker = self.worker.as_ref().expect("registered display task");
        if embassy_time::with_timeout(embassy_time::Duration::from_secs(2), worker.wait_ready())
            .await
            .is_err()
        {
            let _ = worker.cancel();
            return Err("display-task-ready-timeout");
        }
        if worker.status().phase != "ready-held" {
            return Err("display-task-not-ready");
        }
        self.phase = "task-owned";
        Ok(())
    }
}

impl NativeDisplay {
    pub fn assignment_staged(&self) -> bool {
        #[cfg(feature = "display-ready-root")]
        {
            self.staged.is_some()
        }
        #[cfg(not(feature = "display-ready-root"))]
        {
            false
        }
    }
}
