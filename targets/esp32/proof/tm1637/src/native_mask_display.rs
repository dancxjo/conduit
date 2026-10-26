//! Root's actual GPIO ownership for the ordinary Mask realization. This replaces
//! the raw output realization; it does not keep its software or kernel alongside.
use crate::{boot::ObservedBoot, electrical::DisplayPins};
use alloc::boxed::Box;
use conduit_core as c;
use conduit_environment_runtime::{display_mask::PreparedDisplayMask, mask_renderer as r};
use conduit_seven_segment_mask::{DigitWiring, SevenSegmentLook, Tm1637Profile, WireBit};
use esp_hal::gpio::Pin;
#[path = "native_mask_session.rs"]
mod session;
#[path = "native_mask_task.rs"]
pub mod task;
#[path = "native_mask_worker.rs"]
mod worker;
pub use session::{FaceScope, FixedKernel, NativeMaskSession, Preparation as LocalPreparation};
pub use worker::{EvidenceSink, Lease, LeaseSource, MAXIMUM_CHILDREN, PERIOD_MS};
pub struct NativeDisplay {
    ready: Option<r::ReadyRenderer<DisplayPins<'static>>>,
    local: Option<Box<NativeMaskSession>>,
    local_issued: bool,
    registered: bool,
    local_context_bytes: Option<usize>,
    context: Option<Box<conduit_face::PreparedMaskShowContext>>,
    kernel: Option<conduit_environment_runtime::mask_kernel::PreparedMaskKernel>,
}
impl NativeDisplay {
    pub fn prepare(
        clock: impl Pin + 'static,
        data: impl Pin + 'static,
        boot: &ObservedBoot,
    ) -> Result<Box<Self>, &'static str> {
        let (host, boot) = boot.ids();
        let identity = r::RendererIdentity {
            host_id: host.into(),
            boot_id: boot.into(),
            base_id: "base/esp32/gpio18-gpio19".into(),
            provider_instance_id: "instance/esp32/tm1637-mask".into(),
            provider_generation: 1,
            resource_pool_id: "pool/esp32/gpio18-gpio19".into(),
            resource_generation_id: c::ResourceGenerationId("generation/tm1637/1".into()),
            envelope_id: "envelope/esp32/tm1637-mask".into(),
            artifact_id: "native/esp32-seven-segment-root@1".into(),
            implementation_id: "esp32/seven-segment-renderer@1".into(),
            base_implementation_id: "esp32/tm1637-base@1".into(),
        };
        let mask = PreparedDisplayMask::prepare(
            Tm1637Profile {
                digits: core::array::from_fn(|i| DigitWiring {
                    address: i as u8,
                    segment_bits: [0, 1, 2, 3, 4, 5, 6],
                }),
                decimal_points: [None; 4],
                colon: Some(WireBit { address: 1, bit: 7 }),
            },
            SevenSegmentLook::fahrenheit().with_colon_fraction(),
            3000,
            120000,
            3,
        )
        .map_err(|_| "mask-look")?;
        // SAFETY: composition root transfers the actual GPIO18/19 here exactly
        // once; DisplayPins releases open-drain lines on Drop. This artifact's
        // immutable Look/profile and finite Source packet material are fixed
        // above. No received descriptor constructs provider possession.
        let ready = unsafe {
            r::ReadyRenderer::new(
                identity,
                r::Bounds {
                    face_bytes: 8192,
                    show_bytes: 8192,
                    resource_bytes: 64,
                },
                DisplayPins::new(clock, data),
                mask,
            )
        }?;
        Ok(Box::new(Self {
            ready: Some(ready),
            local: None,
            local_issued: false,
            registered: false,
            local_context_bytes: None,
            context: None,
            kernel: None,
        }))
    }
    pub fn native_bound(&self) -> bool {
        self.local_issued
    }
    pub fn retain_local(&mut self, preparation: LocalPreparation) -> Result<(), &'static str> {
        if self.local_issued
            || self.local.is_some()
            || self.context.is_some()
            || self.kernel.is_some()
        {
            return Err("mask-already-prepared");
        }
        let bytes = preparation.context.plan_wire().len();
        self.local = Some(NativeMaskSession::prepare(&mut self.ready, preparation)?);
        self.local_context_bytes = Some(bytes);
        self.local_issued = true;
        Ok(())
    }
    pub fn take_local(&mut self) -> Result<Box<NativeMaskSession>, &'static str> {
        self.local.take().ok_or("mask-local-unavailable")
    }
    pub fn mark_local_task_registered(&mut self) -> Result<(), &'static str> {
        if !self.local_issued || self.local.is_some() || self.registered {
            return Err("mask-task-registration");
        }
        self.registered = true;
        Ok(())
    }
    pub fn task_registered(&self) -> bool {
        self.registered
    }
    pub fn context_bytes(&self) -> Option<usize> {
        self.local_context_bytes.or_else(|| {
            self.context
                .as_ref()
                .map(|context| context.plan_wire().len())
        })
    }
    pub fn retain_preparation(
        &mut self,
        context: conduit_face::PreparedMaskShowContext,
        kernel: conduit_environment_runtime::mask_kernel::PreparedMaskKernel,
    ) -> Result<(), &'static str> {
        if self.context.is_some() || self.kernel.is_some() {
            return Err("mask-already-prepared");
        }
        self.context = Some(Box::new(context));
        self.kernel = Some(kernel);
        Ok(())
    }
    pub fn retire_preparation(&mut self) -> Result<(), &'static str> {
        use conduit_kernel::{SignSink, ValueStorage};
        let kernel = self.kernel.as_ref().ok_or("mask-kernel-unavailable")?;
        if kernel.kernel.decisions() != 0
            || kernel.kernel.logical_invocations() != 0
            || kernel.kernel.pending_host_call_count() != 0
            || kernel.kernel.values().used_items() != 0
            || !kernel.kernel.signs().is_empty()
        {
            return Err("mask-not-dormant");
        }
        self.kernel = None;
        self.context = None;
        Ok(())
    }
    pub fn renderer(&self) -> Option<&r::ReadyRenderer<DisplayPins<'static>>> {
        self.ready.as_ref()
    }
    pub fn append_to_advertisement(&self, host: &mut c::HostAdvertisement) -> Result<(), ()> {
        if self.kernel.is_some() || self.local_issued {
            return Ok(());
        }
        if let Some(ready) = self.ready.as_ref() {
            ready.append_to_advertisement(host).map_err(|_| ())?;
        }
        Ok(())
    }
    pub fn append_software_offers(&self, host: &mut c::HostAdvertisement) {
        if self.kernel.is_some() || self.local_issued {
            return;
        }
        // These stateless routing/closure factories are the same compiled Backs
        // admitted by PreparedAssignedMaskKernel, not packet-provided software.
        let limits = c::CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8192,
        };
        let implementation = || c::ImplementationOffer {
            execution_profile_id: "environment/mask-routing@1".into(),
            implementation_id: "environment/mask-routing@1".into(),
            artifact_id: "native/environment-mask-routing@1".into(),
        };
        host.capabilities.push(conduit_face::face_tee_offer(
            "environment/mask-tee@1".into(),
            implementation(),
            limits.clone(),
        ));
        host.capabilities
            .push(conduit_face::face_interaction_close_offer(
                "environment/mask-close@1".into(),
                implementation(),
                limits,
            ));
    }
    pub fn context_staged(&self) -> bool {
        self.context.is_some() || self.local_issued
    }
    // The old raw-WRITE inspection entrance is explicitly unavailable in this
    // mode; it must not produce a WRITE grant for the renderer's pin owner.
    pub fn native_identity(
        &self,
    ) -> Option<&conduit_tm1637_wire::installation::WriteNativeIdentity> {
        None
    }
    pub fn software_ready(&self) -> bool {
        false
    }
    pub fn software_refusal(&self) -> Option<&'static str> {
        Some("ordinary-mask-mode")
    }
    pub fn assignment_staged(&self) -> bool {
        false
    }
    pub fn kernel_prepared(&self) -> bool {
        self.kernel.is_some() || self.local_issued
    }
    pub fn current_software_offers(
        &mut self,
        _: &c::HostId,
        _: &c::BootId,
        _: c::OfferGeneration,
    ) -> Result<
        conduit_environment_runtime::appliance::software_compact::CompactOutputOffers<'_, 'static>,
        (),
    > {
        Err(())
    }
}
