//! Root-owned finite handoff. Each phase retains the actual incoming owners;
//! no old scheduler, permit, static claim or provider budget is reset here.
use super::local_mask::{PreparedSamplerKernel, PreparedSamplerParts, RetainedMask};
use super::{c, NativeRoot};
use alloc::boxed::Box;
use conduit_kernel as k;

#[path = "native_local_renewal_capacity.rs"]
mod capacity;
#[path = "native_local_renewal_policy.rs"]
mod policy;
#[path = "native_local_renewal_retirement.rs"]
mod retirement;
#[path = "native_sampler_terminal.rs"]
mod sampler_terminal;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RenewalReason {
    NormalCompleted,
    CapacityReturned,
}
impl RenewalReason {
    fn name(self) -> &'static str {
        match self {
            Self::NormalCompleted => "normal-completed",
            Self::CapacityReturned => "capacity-returned",
        }
    }
}
#[allow(clippy::large_enum_variant)]
enum MaskReturn {
    Completed(crate::native_display::task::RetiredMask),
    Interrupted(crate::native_display::task::InterruptedMask),
}

/// Oldest still-bound outgoing journal scope. One bounded witness survives
/// capacity refusal; no overwrite or implicit ACK can erase its history.
pub(super) struct HistoryCustody {
    pub(super) active: c::ActivePlayIdentity,
    pub(super) receipt: crate::native_worker::SettledExecutionScope,
}
pub(super) struct Renewal {
    pub(super) phase: Phase,
    pub(super) reason: RenewalReason,
    pub(super) mask_interrupted: bool,
    pub(super) failure: Option<&'static str>,
    retry_after_ms: u64,
    pub(super) old_sampler: c::ActivePlayIdentity,
    old_mask: c::ActivePlayIdentity,
    old_body_play: conduit_body::BodyPlayIdentity,
    pub(super) sampler_handle: Option<crate::native_task::Handle>,
    pub(super) mask_handle: Option<crate::native_display::task::Handle>,
    sampler: Option<crate::native_task::Retired>,
    mask: Option<Box<MaskReturn>>,
    mask_backing: Option<Box<core::mem::MaybeUninit<MaskReturn>>>,
    pub(super) execution: Option<
        crate::native_worker::RetiredExecution<
            crate::native_root::DevicePulse<'static>,
            crate::native_wait::EspWait,
        >,
    >,
    pub(super) receipt: Option<crate::native_worker::SettledExecutionScope>,
    pub(super) face: Option<Box<FaceReturn>>,
    pub(super) values: Option<k::HostedValueStore>,
    pub(super) mask_values: Option<&'static mut k::FixedValueStore<2, 8192>>,
    pub(super) renderer: Option<
        conduit_environment_runtime::mask_renderer::ReadyRenderer<
            crate::electrical::DisplayPins<'static>,
        >,
    >,
    pub(super) mask_evidence: Option<Box<crate::native_mask_evidence::Consumer>>,
    pub(super) parked: Option<crate::native_restart::ParkedBody>,
    pub(super) sample_seal: Option<conduit_body::PreparedVerifiedBodyPlotPlan>,
    pub(super) sampler_parts: Option<Box<PreparedSamplerParts>>,
    pub(super) sampler_kernel: Option<Box<PreparedSamplerKernel>>,
    pub(super) kernel_backing: Option<crate::native_kernel_storage::KernelBacking>,
    pub(super) mask_plan: Option<c::VerifiedPlanMetadata>,
    pub(super) mask_parts:
        Option<Box<conduit_environment_runtime::mask_kernel::PreparedMaskKernelParts>>,
    pub(super) context: Option<conduit_face::PreparedMaskShowContext>,
    pub(super) retained_mask: Option<Box<Option<RetainedMask>>>,
    pub(super) local_preparation: Option<Box<crate::native_display::LocalPreparation>>,
    pub(super) started_ms: u64,
    pub(super) last_reading_ms: Option<u64>,
    pub(super) proposal: Option<conduit_body::PreparedVerifiedBodyProposal>,
    pub(super) registered_publisher: Option<crate::native_face::Publisher>,
    pub(super) fresh_kernel: Option<crate::native_kernel::PreparedKernel>,
    pub(super) fresh_native: Option<super::local_mask::Bindings>,
    pub(super) fresh_leases: Option<Box<crate::native_authority::MaskLeaseIssuer>>,
    pub(super) fresh_evidence: Option<Box<crate::native_mask_evidence::Consumer>>,
    pub(super) fresh_session: Option<Box<crate::native_display::NativeMaskSession>>,
    pub(super) history_done: bool,
    pub(super) archive_refusal: Option<&'static str>,
    pub(super) body_started: bool,
    pub(super) fresh_active: Option<c::ActivePlayIdentity>,
    pub(super) fresh_fragment: Option<c::FragmentId>,
}
// This entire cold state is already boxed; another box would add a fallible
// ownership transfer in the retry path solely to shrink its inline variants.
#[allow(clippy::large_enum_variant)]
pub(super) enum FaceReturn {
    Returning(crate::native_face::Returning),
    Returned(crate::native_face::Returned),
    Replacing(crate::native_face::Replacing),
    Fresh(crate::native_face::Publisher, crate::native_face::Reader),
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    RequestInterruption,
    AwaitInterruption,
    RetireSampler,
    RetireMask,
    FinishMask,
    FinishSampler,
    ReuniteFace,
    Lull,
    CheckpointLull,
    Reclaim,
    PrepareSoftware,
    PrepareSampler,
    PrepareMask,
    RestoreBody,
    CommitProposal,
    CheckpointProposal,
    PrepareMaskCustody,
    Assemble,
    Register,
    Start,
    Complete,
}
impl Renewal {
    fn new(root: &NativeRoot, reason: RenewalReason) -> Result<Self, &'static str> {
        let current = root
            .body
            .as_ref()
            .and_then(|b| b.realization())
            .ok_or("renewal-current-body")?;
        let old_body_play = current.play.as_ref().ok_or("renewal-current-play")?.clone();
        let old_sampler = root
            .issuer
            .native_issued_active()
            .ok_or("renewal-sampler-scope")?
            .clone();
        let old_mask = root
            .issuer
            .mask_issued_active()
            .ok_or("renewal-mask-scope")?
            .clone();
        Ok(Self {
            phase: if reason == RenewalReason::CapacityReturned {
                Phase::RequestInterruption
            } else {
                Phase::RetireSampler
            },
            reason,
            mask_interrupted: false,
            failure: None,
            retry_after_ms: 0,
            old_sampler,
            old_mask,
            old_body_play,
            sampler_handle: None,
            mask_handle: None,
            sampler: None,
            mask: None,
            mask_backing: Some(reserve_box::<MaskReturn>()?),
            execution: None,
            receipt: None,
            face: None,
            values: None,
            mask_values: None,
            renderer: None,
            mask_evidence: None,
            parked: None,
            sample_seal: None,
            sampler_parts: None,
            sampler_kernel: None,
            kernel_backing: None,
            mask_plan: None,
            mask_parts: None,
            context: None,
            retained_mask: None,
            local_preparation: None,
            started_ms: embassy_time::Instant::now().as_millis(),
            last_reading_ms: root
                .worker
                .as_ref()
                .and_then(|w| w.status().committed_millis),
            proposal: None,
            registered_publisher: None,
            fresh_kernel: None,
            fresh_native: None,
            fresh_leases: None,
            fresh_evidence: None,
            fresh_session: None,
            history_done: false,
            archive_refusal: None,
            body_started: false,
            fresh_active: None,
            fresh_fragment: None,
        })
    }
    pub(super) fn phase_name(&self) -> &'static str {
        match self.phase {
            Phase::RequestInterruption => "request-interruption",
            Phase::AwaitInterruption => "await-interruption",
            Phase::RetireSampler => "retire-sampler",
            Phase::RetireMask => "retire-mask",
            Phase::FinishMask => "finish-mask",
            Phase::FinishSampler => "finish-sampler",
            Phase::ReuniteFace => "reunite-face",
            Phase::Lull => "lull",
            Phase::CheckpointLull => "checkpoint-lull",
            Phase::Reclaim => "reclaim",
            Phase::PrepareSoftware => "prepare-software",
            Phase::PrepareSampler => "prepare-sampler",
            Phase::PrepareMask => "prepare-mask",
            Phase::RestoreBody => "restore-body",
            Phase::CommitProposal => "commit-proposal",
            Phase::CheckpointProposal => "checkpoint-proposal",
            Phase::PrepareMaskCustody => "prepare-mask-custody",
            Phase::Assemble => "assemble",
            Phase::Register => "register",
            Phase::Start => "start",
            Phase::Complete => "complete",
        }
    }
}
impl NativeRoot {
    /// Serialize new setup/membership mutations with the unpublished cold
    /// proposal. This does not gate independent runners, history reads or
    /// scoped emergency cancellation entrances.
    pub fn local_renewal_pending(&self) -> bool {
        self.local_renewal.is_some()
    }
    fn local_normal_completion(&self) -> bool {
        let (_, cadence) = crate::native_software::source_facts();
        let Some(sampler) = self.worker.as_ref().map(|w| w.status()) else {
            return false;
        };
        let Some(mask) = self.local_mask_worker.as_ref().map(|w| w.status()) else {
            return false;
        };
        matches!(sampler.phase, "drained" | "quiesced")
            && !sampler.task_active
            && sampler.progress_current
            && sampler.failure.is_none()
            && !sampler.cleanup_failure
            && sampler.completed == [cadence.count, cadence.count]
            && sampler.native_lifecycles == Some(["revoked", "revoked"])
            && sampler.native_in_flight == Some([0, 0])
            && mask.phase == "quiesced"
            && !mask.task_active
            && mask.activated
            && mask.completed == crate::native_display::MAXIMUM_CHILDREN
            && !mask.pending_evidence
            && mask.failure.is_none()
            && mask.cleanup_refusal.is_none()
    }
    /// Normal renewal waits for BOTH finite terminal gates. Capacity recovery
    /// instead requests an explicit interrupted handoff only after real reusable
    /// storage and cold control backing exist. Neither path fabricates completed
    /// work or changes the old scope's sequence; refusals retain actual custody.
    pub async fn advance_local_renewal(&mut self) -> Result<(), &'static str> {
        if self.local_renewal.is_none() {
            let reason = if self.local_normal_completion() {
                RenewalReason::NormalCompleted
            } else if self.local_capacity_returned() {
                RenewalReason::CapacityReturned
            } else {
                return Ok(());
            };
            if reason == RenewalReason::CapacityReturned && !self.capacity_checkpoint_preflight() {
                return Ok(());
            }
            let needed = core::mem::size_of::<Renewal>()
                .saturating_add(core::mem::size_of::<MaskReturn>())
                .saturating_add(8192);
            let memory = esp_alloc::HEAP.stats();
            if memory.size.saturating_sub(memory.current_usage) < needed {
                return Err("renewal-control-capacity");
            }
            let backing = reserve_box::<Renewal>()?;
            self.local_renewal = Some(Box::write(backing, Renewal::new(self, reason)?));
        }
        // Store custody can change between allocation and the next service
        // turn. Recheck immediately BEFORE any signal; unknown/full capacity
        // discards only unused cold control state and keeps both old workers.
        if self
            .local_renewal
            .as_ref()
            .is_some_and(|state| state.phase == Phase::RequestInterruption)
            && !self.capacity_checkpoint_preflight()
        {
            self.local_renewal = None;
            return Ok(());
        }
        let mut renewal = self.local_renewal.take().expect("retained renewal state");
        let now = embassy_time::Instant::now().as_millis();
        if let Some(reason) = renewal.failure {
            let retryable = !reason.ends_with("invalid")
                && (reason.ends_with("busy")
                    || reason.ends_with("capacity")
                    || reason == "restart-memory"
                    || matches!(
                        renewal.phase,
                        Phase::CheckpointLull | Phase::CheckpointProposal
                    )
                    || (renewal.phase == Phase::Start && renewal.body_started));
            if !retryable || now < renewal.retry_after_ms {
                self.local_renewal = Some(renewal);
                return Err(reason);
            }
        }
        // Retain no redundant async wrapper around this disjoint phase split.
        // The actual affine retirement helper remains synchronously outlined.
        let result = match self.advance_local_retirement_phase(&mut renewal) {
            Ok(true) => Ok(()),
            Ok(false) => self.advance_returned_local_preparation(&mut renewal).await,
            Err(reason) => Err(reason),
        };
        renewal.failure = result.as_ref().err().copied();
        renewal.retry_after_ms = now.saturating_add(1000);
        if result.is_ok() && renewal.phase == Phase::Complete {
            self.local_renewal_archive_refusal = renewal.archive_refusal;
            capacity::report_completion(&renewal);
            // All old scopes and their terminal evidence passed canonical Lull
            // and complete durable checkpoint before fresh owners were released.
            // Signed reading journals remain separately retained without ACK.
            // CapacityReturned has distinct cancellation/withdrawal evidence;
            // its fresh first sample never fills gaps in the interrupted Play.
            drop(renewal);
        } else {
            self.local_renewal = Some(renewal);
        }
        result
    }
}

impl NativeRoot {
    pub(super) fn reclaim_local_renewal(
        &mut self,
        state: &mut Renewal,
    ) -> Result<(), &'static str> {
        use k::ValueStorage;
        let generation = self
            .offer_generation
            .checked_add(1)
            .ok_or("renewal-offer-exhausted")?;
        if state.mask_values.is_none() {
            let parts = match *state.mask.take().ok_or("renewal-mask-material")? {
                MaskReturn::Completed(retired) => retired.into_parts(),
                MaskReturn::Interrupted(retired) => retired.into_parts(),
            };
            let values = parts.kernel.values;
            values.clear(); // Original slot generations/retired slots survive.
            state.mask_values = Some(values);
            state.renderer = Some(parts.renderer);
            state.mask_evidence = Some(parts.evidence);
            // Final Closed and its exact Sign tail were already accepted by the
            // actual Consumer before normal RetiredMask could be constructed.
        }
        if state.renderer.is_some() {
            self.display.retain_returned_renderer(&mut state.renderer)?;
        }
        if state.values.is_none() {
            let retired = state.execution.as_ref().ok_or("renewal-sampler-material")?;
            let generations = retired.providers.provider_generations();
            let pulse = linked_pulse(
                &self.host,
                &self.boot,
                generations[0]
                    .checked_add(1)
                    .ok_or("renewal-pulse-generation")?,
            );
            let wait = linked_wait(
                &self.host,
                &self.boot,
                generations[1]
                    .checked_add(1)
                    .ok_or("renewal-wait-generation")?,
            );
            retired
                .providers
                .check_reattach(&pulse, &wait)
                .map_err(|_| "renewal-provider-identity")?;
            let retired = state
                .execution
                .take()
                .expect("preflighted sampler material");
            let (pulse, wait) = retired
                .providers
                .reattach(pulse, wait)
                .unwrap_or_else(|_| unreachable!("exclusively preflighted provider continuation"));
            let (values, signs) = retired.material.into_reusable_backing();
            self.pulse = Some(pulse);
            self.wait = Some(wait);
            state.values = Some(values);
            // Existing acceptance boundary: both actual terminal scopes were
            // finished, the exact old BodyPlay was canonically Lulled, and its
            // complete checkpoint succeeded BEFORE Reclaim became reachable.
            // The transient bounded kernel tail ends with that execution's
            // lifetime. This is not remote ACK of signed reading history.
            drop(signs);
        }
        self.offer_generation = generation;
        state.phase = Phase::PrepareSoftware;
        Ok(())
    }
}
// Canonical native installation facts, checked against the full opaque Ready
// origin seal before actual returned owners can be transferred. These do not
// construct a provider or grant a capability from the names alone.
fn linked_pulse(
    host: &c::HostId,
    boot: &c::BootId,
    generation: u64,
) -> conduit_pulse_capture::installation::PulseNativeIdentity {
    conduit_pulse_capture::installation::PulseNativeIdentity {
        host_id: host.clone(),
        boot_id: boot.clone(),
        base_id: "base/esp32/gpio27".into(),
        provider_instance_id: "instance/esp32/gpio27".into(),
        provider_generation: generation,
        resource_pool_id: "pool/esp32/gpio27".into(),
        resource_generation_id: c::ResourceGenerationId("generation/gpio27/1".into()),
        envelope_id: "envelope/esp32/gpio27".into(),
        artifact_id: "native/esp32-gpio-pulse-root@1".into(),
        implementation_id: "esp32/gpio27-pulse@1".into(),
        base_implementation_id: "esp32/gpio27-pulse-base@1".into(),
    }
}
fn linked_wait(
    host: &c::HostId,
    boot: &c::BootId,
    generation: u64,
) -> conduit_monotonic_wait::WaitNativeIdentity {
    conduit_monotonic_wait::WaitNativeIdentity {
        host_id: host.clone(),
        boot_id: boot.clone(),
        base_id: "base/esp32/monotonic-wait".into(),
        provider_instance_id: "instance/esp32/monotonic-wait".into(),
        provider_generation: generation,
        resource_pool_id: "pool/esp32/monotonic-wait".into(),
        resource_generation_id: c::ResourceGenerationId("generation/monotonic-wait/1".into()),
        envelope_id: "envelope/esp32/monotonic-wait".into(),
        artifact_id: "native/esp32-monotonic-wait@1".into(),
        implementation_id: "esp32/monotonic-wait@1".into(),
        base_implementation_id: "esp32/monotonic-wait-base@1".into(),
    }
}

/// Reserve cold control storage before consuming any actual input owner.
/// Allocation refusal is separate from semantic/native admission refusal.
pub(super) fn reserve_box<T>() -> Result<Box<core::mem::MaybeUninit<T>>, &'static str> {
    crate::native_box_backing::reserve_box().map_err(|_| "renewal-cold-capacity")
}
/// Move a cold transaction out while preserving its exact Box allocation for
/// every retry/refusal. No reference to the old initialized value survives.
pub(super) fn take_box<T>(owned: Box<T>) -> (T, Box<core::mem::MaybeUninit<T>>) {
    let raw = Box::into_raw(owned);
    // SAFETY: consume the unique Box, move T exactly once, then own only its
    // uninitialized allocation. Box::write is the only reinitialization route.
    unsafe {
        (
            raw.read(),
            Box::from_raw(raw.cast::<core::mem::MaybeUninit<T>>()),
        )
    }
}
