use conduit_core::{
    BaseImplementationId, HostAdvertisement, HostId, Observation, OfferGeneration, Plan,
    PlanFragment, PlanId,
};
use conduit_form::CheckedForm;
use conduit_planner::{default_placements, parse_placements, plan, PlacementChoices};
use conduit_signal::{PULSE_KIND, SHOW_KIND};
use std::fs;
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(all(target_os = "linux", feature = "bluetooth-bluez"))]
pub mod bluetooth_gatt;
pub mod body_coordination;
mod boot_identity;
pub mod browser_admission;
mod composition;
#[cfg(test)]
mod composition_test_offers;
mod copy_task;
mod deadline_reactor;
pub mod distributed_signal;
pub mod distributed_toggle;
pub mod text_lab_live;
pub mod text_lab_split;
pub use composition::{reference_advertisement, supported_nucleus_offers, StdHostComposition};
pub use copy_task::{
    prepare_copy_task, CopyRequestId, CopyResult, CopyRunReceipt, CopyStopToken, PreparedCopyTask,
    ProtectedFileAvailability, ProtectedFileRegistry,
};
pub use deadline_reactor::{
    DeadlineClock, DeadlineClockError, DeadlineHostAdapter, DeadlineHostError, DeadlineKey,
    DeadlineReactor, DeadlineReactorError, DeadlineWake, ThreadMonotonicClock,
};
pub mod external_signal;
pub mod external_websocket;
pub mod hosted_audio;
pub mod hosted_calendar;
pub mod hosted_data;
pub mod hosted_geometry;
pub mod hosted_http;
pub mod hosted_job;
pub mod hosted_keyboard;
pub mod hosted_linguistics;
pub mod hosted_local_model;
pub mod hosted_messaging;
pub mod hosted_midi;
pub mod hosted_model;
pub mod hosted_model_compute;
pub mod hosted_network;
pub mod hosted_reminder;
pub mod hosted_resource;
pub mod hosted_synth;
pub mod hosted_vector_index;
pub mod hosted_vector_search;
#[cfg(test)]
mod image_binding_tests;
mod installed_std;
#[cfg(test)]
mod installed_std_tests;
pub mod kernel_multivalue;
mod kernel_preparation;
mod kernel_signal;
#[cfg(feature = "local-model-proof")]
pub mod local_model_proof;
mod run_control;
pub use run_control::{
    RejectedRunControlRequest, RunControl, RunControlDisposition, RunControlReceipt,
    RunControlRequestId,
};
#[cfg(unix)]
pub mod pico_admission;
pub mod pico_control_source;
#[cfg(unix)]
pub mod pico_spawn;
pub mod pico_usb_source;
pub mod pico_wifi_bootstrap;
pub mod pool_webchat;
pub mod r1_control;
pub mod r1_control_input;
pub mod reaction_diffusion;
pub use reaction_diffusion::*;
pub mod sound_recovery;
#[cfg(all(target_os = "linux", feature = "pete-create"))]
pub mod std_create_uart;
pub mod triple_signal;
pub mod usb_cdc;
pub mod websocket;

#[cfg(test)]
mod allocation_probe {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    pub struct TrackingAllocator;

    thread_local! {
        static TRACKING: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    fn record() {
        let _ = TRACKING.try_with(|tracking| {
            if tracking.get() {
                let _ = ALLOCATIONS.try_with(|allocations| {
                    allocations.set(allocations.get().saturating_add(1));
                });
            }
        });
    }

    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) };
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let pointer = unsafe { System.realloc(pointer, layout, new_size) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }
    }

    #[global_allocator]
    static ALLOCATOR: TrackingAllocator = TrackingAllocator;

    pub struct Guard {
        finished: bool,
    }

    impl Guard {
        pub fn finish(mut self) -> usize {
            self.finished = true;
            TRACKING.with(|tracking| tracking.set(false));
            ALLOCATIONS.with(Cell::get)
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            if !self.finished {
                TRACKING.with(|tracking| tracking.set(false));
            }
        }
    }

    pub fn begin() -> Guard {
        ALLOCATIONS.with(|allocations| allocations.set(0));
        TRACKING.with(|tracking| tracking.set(true));
        Guard { finished: false }
    }
}

#[derive(Debug, Clone)]
pub struct StdHostConfig {
    pub host_id: HostId,
    pub boot_id: conduit_core::BootId,
    pub offer_generation: OfferGeneration,
}

#[derive(Debug, Clone)]
pub struct StdRunReport {
    pub observations: Vec<Observation>,
    pub receipts: Vec<SignalReceipt>,
    pub kernel: Option<StdKernelExecutionReport>,
    pub control_receipts: Vec<RunControlReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdKernelExecutionReport {
    pub active_play_id: conduit_core::ActivePlayId,
    pub decisions: u32,
    pub kernel_events: u16,
    pub kernel_sign: Vec<conduit_kernel::KernelEvent>,
    pub value_allocation_capacity_before: (usize, usize),
    pub value_allocation_capacity_after: (usize, usize),
    pub presentation_ids: Vec<conduit_core::PresentationId>,
    pub playback: Vec<hosted_audio::PlaybackReport>,
    pub midi_input: Vec<hosted_midi::MidiInputReport>,
    pub midi_output: Vec<hosted_midi::MidiOutputReport>,
    pub identity: conduit_plan_lowering::lowering::KernelExecutionIdentityMap,
    #[cfg(test)]
    pub post_play_start_allocations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReceipt {
    pub placement_id: conduit_core::PlacementId,
    pub sequence: u64,
    pub level: bool,
}

pub trait TimerAdapter {
    fn wait(&mut self, duration: Duration);

    /// Returns the current host/boot-scoped monotonic millisecond reading when
    /// this adapter offers the admitted deadline contract.
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        None
    }

    /// Returns the admitted Host/Boot-scoped monotonic microsecond reading.
    fn monotonic_now_micros(&mut self) -> Option<u64> {
        None
    }

    /// Waits until one exact reading on the same monotonic basis. Returning
    /// false means the adapter does not offer that basis.
    fn wait_until_monotonic_ms(&mut self, deadline_ms: u64) -> bool {
        let Some(now_ms) = self.monotonic_now_ms() else {
            return false;
        };
        self.wait(Duration::from_millis(deadline_ms.saturating_sub(now_ms)));
        true
    }
}

pub struct ThreadTimer;

static THREAD_TIMER_EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

impl TimerAdapter for ThreadTimer {
    fn wait(&mut self, duration: Duration) {
        thread::sleep(duration);
    }

    fn monotonic_now_ms(&mut self) -> Option<u64> {
        u64::try_from(
            THREAD_TIMER_EPOCH
                .get_or_init(Instant::now)
                .elapsed()
                .as_millis(),
        )
        .ok()
    }

    fn monotonic_now_micros(&mut self) -> Option<u64> {
        u64::try_from(
            THREAD_TIMER_EPOCH
                .get_or_init(Instant::now)
                .elapsed()
                .as_micros(),
        )
        .ok()
    }
}

pub fn run_kernel_multivalue_path_to<W: Write, T: TimerAdapter>(
    path: &str,
    output: &mut W,
    timer: &mut T,
) -> Result<kernel_multivalue::MultiValueRunReport, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let form = conduit_form::parse(&source, &kernel_multivalue::profile_catalog())
        .map_err(|error| error.to_string())?;
    let advertisement = kernel_multivalue::advertisement(
        HostId::from("std-host-1"),
        conduit_core::BootId::from(boot_identity::fresh_boot_id()),
        OfferGeneration(1),
    );
    let plan =
        kernel_multivalue::plan_local(&form, &advertisement).map_err(|error| error.to_string())?;
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == advertisement.host_id)
        .ok_or_else(|| "no local multi-value fragment for std host".to_string())?;
    write_operator_report(output, &advertisement, &fragment.plan_id, &fragment)?;
    let mut resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
    let reservation = resources.prepare_and_reserve(&advertisement, &fragment)?;
    let mut sign_sequence = 0;
    let result = kernel_multivalue::execute_fragment(
        &advertisement,
        &fragment,
        0,
        &mut sign_sequence,
        output,
        timer,
    );
    let release = resources.release(reservation);
    let report = result?;
    release?;
    writeln!(output, "plan {} complete", fragment.plan_id.as_str())
        .map_err(|error| error.to_string())?;
    writeln!(output, "receipts 3 even=(0, 2) latest=(3)").map_err(|error| error.to_string())?;
    writeln!(
        output,
        "kernel active_play={} decisions={} events={} stable_allocations={} pressure_connection={} pressure_items={} pressure_bytes={} input_closed={} terminal_order_exact={}",
        report.active_play_id.as_str(),
        report.decisions,
        report.kernel_events,
        report.value_allocation_capacity_before == report.value_allocation_capacity_after,
        report.pressure_connection_id.as_str(),
        report.pressure_items,
        report.pressure_bytes,
        report.input_closed_events,
        report.terminal_order_exact,
    )
    .map_err(|error| error.to_string())?;
    Ok(report)
}

pub struct StdHost {
    advertisement: HostAdvertisement,
    image_identity: Option<conduit_host_fabrication::ImageBootIdentity>,
    playback: Option<hosted_audio::HostedPlaybackSelection>,
    midi_input: Option<hosted_midi::HostedRawMidiSelection>,
    midi_output: Option<hosted_midi::MidiOutputSelection>,
    local_model: Option<Box<dyn hosted_local_model::HostedLocalModelAdapter>>,
    vector_search: Option<Box<dyn hosted_vector_search::HostedVectorSearchAdapter>>,
    calendar: Option<Box<dyn hosted_calendar::HostedCalendarAdapter>>,
    kernel_resources: kernel_preparation::KernelResourceLedger,
    next_kernel_play_sequence: u64,
    next_kernel_sign_sequence: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct IssuedKernelPlay {
    identity: conduit_core::ActivePlayIdentity,
}

impl IssuedKernelPlay {
    pub fn identity(&self) -> &conduit_core::ActivePlayIdentity {
        &self.identity
    }
}

impl Default for StdHost {
    fn default() -> Self {
        Self::new()
    }
}

impl StdHost {
    pub fn issue_kernel_play(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<IssuedKernelPlay, String> {
        if fragment.host_id != self.advertisement.host_id
            || fragment.boot_id != self.advertisement.boot_id
        {
            return Err("Plan fragment is stale for this Host boot".to_string());
        }
        let play_sequence = self.next_kernel_play_sequence;
        self.next_kernel_play_sequence = play_sequence
            .checked_add(1)
            .ok_or_else(|| "kernel Play sequence exhausted".to_string())?;
        Ok(IssuedKernelPlay {
            identity: conduit_core::bind_active_play(
                &fragment.plan_id,
                &fragment.host_id,
                &fragment.boot_id,
                play_sequence,
            ),
        })
    }
    pub fn new() -> Self {
        Self::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: conduit_core::BootId::from(boot_identity::fresh_boot_id()),
            offer_generation: OfferGeneration(1),
        })
    }

    pub fn new_with_config(config: StdHostConfig) -> Self {
        Self::new_with_composition(config, StdHostComposition::reference())
    }

    pub fn new_with_composition(config: StdHostConfig, composition: StdHostComposition) -> Self {
        let advertisement =
            composition::build_advertisement(config, composition, None, None, None, false);
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)
            .expect("std kernel resource offers are exact and bounded");
        Self {
            advertisement,
            image_identity: None,
            playback: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            vector_search: None,
            calendar: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        }
    }

    pub fn new_with_local_model(
        config: StdHostConfig,
        composition: StdHostComposition,
        adapter: Box<dyn hosted_local_model::HostedLocalModelAdapter>,
    ) -> Result<Self, String> {
        let offer = adapter.offer();
        offer
            .validate()
            .map_err(|error| format!("local-model offer is not initialized: {error:?}"))?;
        let mut advertisement =
            composition::build_advertisement(config, composition, None, None, None, false);
        advertisement
            .resources
            .extend(hosted_local_model::resource_offers(&offer.limits));
        advertisement.capabilities.extend(
            offer
                .capability_offers()
                .map_err(|error| format!("local-model capabilities: {error:?}"))?,
        );
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            midi_input: None,
            midi_output: None,
            local_model: Some(adapter),
            vector_search: None,
            calendar: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    pub fn new_with_vector_search(
        config: StdHostConfig,
        composition: StdHostComposition,
        adapter: Box<dyn hosted_vector_search::HostedVectorSearchAdapter>,
    ) -> Result<Self, String> {
        let mut advertisement =
            composition::build_advertisement(config, composition, None, None, None, false);
        advertisement
            .resources
            .push(adapter.resource_offer().clone());
        advertisement
            .capabilities
            .push(adapter.capability_offer().clone());
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            vector_search: Some(adapter),
            calendar: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    pub fn new_with_calendar(
        config: StdHostConfig,
        composition: StdHostComposition,
        adapter: Box<dyn hosted_calendar::HostedCalendarAdapter>,
    ) -> Result<Self, String> {
        let mut advertisement =
            composition::build_advertisement(config, composition, None, None, None, false);
        advertisement
            .resources
            .push(hosted_calendar::google_calendar_resource_offer());
        advertisement
            .capabilities
            .extend(hosted_calendar::google_calendar_offers());
        advertisement.resources.sort();
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            vector_search: None,
            calendar: Some(adapter),
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    /// Executes against one platform-extended advertisement that was already
    /// published for this exact Host/Boot. Rebuilding from only the generic
    /// composition here would discard admitted platform implementations.
    pub fn from_advertisement(advertisement: HostAdvertisement) -> Result<Self, String> {
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            vector_search: None,
            calendar: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    pub fn new_with_playback(
        config: StdHostConfig,
        composition: StdHostComposition,
        playback: hosted_audio::HostedPlaybackSelection,
    ) -> Result<Self, String> {
        if playback.boot_id != config.boot_id
            || playback.offer_generation != config.offer_generation
        {
            return Err(
                "playback observation does not match the advertised Boot/generation".into(),
            );
        }
        let advertisement = composition::build_advertisement(
            config,
            composition,
            Some(&playback),
            None,
            None,
            false,
        );
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: Some(playback),
            midi_input: None,
            midi_output: None,
            local_model: None,
            vector_search: None,
            calendar: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    pub fn new_with_midi_output(
        config: StdHostConfig,
        composition: StdHostComposition,
        midi_output: hosted_midi::HostedMidiSelection,
    ) -> Result<Self, String> {
        if midi_output.boot_id() != &config.boot_id
            || midi_output.offer_generation() != config.offer_generation
            || midi_output.observation().direction
                != hosted_midi::MidiEndpointDirection::WritableDestination
        {
            return Err(
                "MIDI output observation does not match direction, Boot, and generation".into(),
            );
        }
        let midi_output = hosted_midi::MidiOutputSelection::sequencer(midi_output);
        let advertisement = composition::build_advertisement(
            config,
            composition,
            None,
            None,
            Some(&midi_output),
            false,
        );
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            midi_input: None,
            midi_output: Some(midi_output),
            local_model: None,
            vector_search: None,
            calendar: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }

    pub fn image_identity(&self) -> Option<&conduit_host_fabrication::ImageBootIdentity> {
        self.image_identity.as_ref()
    }

    pub fn from_image_binding(
        binding: conduit_host_fabrication::BoundHostAdvertisement,
    ) -> Result<Self, String> {
        let (image_identity, advertisement) = binding.into_parts();
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: Some(image_identity),
            playback: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            vector_search: None,
            calendar: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    pub fn midi_output_selection(&self) -> Option<&hosted_midi::HostedMidiSelection> {
        self.midi_output
            .as_ref()
            .and_then(|selection| selection.as_sequencer())
    }

    pub(crate) fn new_with_playback_proof(
        config: StdHostConfig,
        playback: hosted_audio::HostedPlaybackSelection,
    ) -> Result<Self, String> {
        if playback.boot_id != config.boot_id
            || playback.offer_generation != config.offer_generation
        {
            return Err(
                "playback observation does not match the advertised Boot/generation".into(),
            );
        }
        let advertisement = composition::build_advertisement(
            config,
            StdHostComposition::minimal(),
            Some(&playback),
            None,
            None,
            true,
        );
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: Some(playback),
            midi_input: None,
            midi_output: None,
            local_model: None,
            vector_search: None,
            calendar: None,
            kernel_resources,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
        })
    }

    pub fn plan_local(
        &self,
        form: &CheckedForm,
        placements: Option<&PlacementChoices>,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let hosts = vec![self.advertisement().clone()];
        let placements = match placements {
            Some(placements) => placements.clone(),
            None => default_placements(form, &hosts)?,
        };
        Ok(plan(
            form,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
        )?)
    }

    pub fn plan_local_with_authority(
        &self,
        form: &CheckedForm,
        placements: Option<&PlacementChoices>,
        authority_grants: &[conduit_core::AuthorityGrant],
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let hosts = vec![self.advertisement().clone()];
        let placements = match placements {
            Some(placements) => placements.clone(),
            None => default_placements(form, &hosts)?,
        };
        Ok(conduit_planner::plan_with_authority_grants(
            form,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            authority_grants,
        )?)
    }

    /// Constructs the explicit grant shape for a caller that has independently
    /// authorized this exact selected playback capability. Merely constructing
    /// or discovering a Host never calls this method.
    pub fn playback_authority_grant(
        &self,
        grant_id: &str,
    ) -> Result<conduit_core::AuthorityGrant, String> {
        let playback = self
            .playback
            .as_ref()
            .ok_or_else(|| "std Host has no selected playback resource".to_string())?;
        if playback.boot_id != self.advertisement.boot_id
            || playback.offer_generation != self.advertisement.offer_generation
        {
            return Err("selected playback observation is stale for this Host".into());
        }
        let capability = self
            .advertisement
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION
            })
            .ok_or_else(|| "selected playback capability is not advertised".to_string())?;
        let requirement = capability
            .authority_requirements
            .first()
            .ok_or_else(|| "playback capability has no authority contract".to_string())?;
        Ok(conduit_core::AuthorityGrant {
            grant_id: conduit_core::AuthorityGrantId::from(grant_id),
            contract_id: requirement.contract_id.clone(),
            host_operation_contract_id: requirement.host_operation_contract_id.clone(),
            subject_kind: requirement.subject_kind.clone(),
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            capability_id: capability.capability_id.clone(),
        })
    }

    /// Constructs the two independently typed grants for an exact selected
    /// MIDI output. Discovery and Host construction never imply these grants.
    pub fn midi_output_authority_grants(
        &self,
        grant_prefix: &str,
    ) -> Result<Vec<conduit_core::AuthorityGrant>, String> {
        let selected = self
            .midi_output
            .as_ref()
            .ok_or_else(|| "std Host has no selected MIDI output resource".to_string())?;
        if selected.boot_id() != &self.advertisement.boot_id
            || selected.offer_generation() != self.advertisement.offer_generation
        {
            return Err("selected MIDI output observation is stale for this Host".into());
        }
        let capability = self
            .advertisement
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::MUSIC_PLAY_MIDI_IMPLEMENTATION
            })
            .ok_or_else(|| "selected MIDI output capability is not advertised".to_string())?;
        if capability.authority_requirements.len() != 2 {
            return Err("MIDI output capability authority shape changed".into());
        }
        Ok(capability
            .authority_requirements
            .iter()
            .enumerate()
            .map(|(index, requirement)| conduit_core::AuthorityGrant {
                grant_id: conduit_core::AuthorityGrantId::from(format!("{grant_prefix}-{index}")),
                contract_id: requirement.contract_id.clone(),
                host_operation_contract_id: requirement.host_operation_contract_id.clone(),
                subject_kind: requirement.subject_kind.clone(),
                host_id: self.advertisement.host_id.clone(),
                boot_id: self.advertisement.boot_id.clone(),
                capability_id: capability.capability_id.clone(),
            })
            .collect())
    }

    pub fn calendar_authority_grants(
        &self,
        operation: hosted_calendar::CalendarHostedOperation,
        grant_prefix: &str,
    ) -> Result<Vec<conduit_core::AuthorityGrant>, String> {
        if self.calendar.is_none() {
            return Err("std Host has no selected calendar resource".into());
        }
        let capability = self
            .advertisement
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str() == operation.implementation()
            })
            .ok_or_else(|| "selected calendar capability is not advertised".to_string())?;
        capability
            .authority_requirements
            .iter()
            .enumerate()
            .map(|(index, _)| {
                hosted_calendar::google_calendar_authority_grant(
                    capability,
                    index,
                    &format!("{grant_prefix}-{index}"),
                    &self.advertisement.host_id,
                    &self.advertisement.boot_id,
                )
            })
            .collect()
    }

    pub fn plan_expanded_local(
        &self,
        form: &conduit_form::ExpandedCanonicalForm,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let hosts = vec![self.advertisement().clone()];
        let placements = conduit_planner::default_expanded_placements(form, &hosts)?;
        Ok(conduit_planner::plan_expanded_canonical(
            form,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
        )?)
    }

    pub fn run_fragment_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
    ) -> Result<StdRunReport, String> {
        self.run_fragment_controlled_to(fragment, output, timer, &RunControl::default())
    }

    pub fn run_fragment_controlled_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
    ) -> Result<StdRunReport, String> {
        self.run_fragment_controlled_with_keyboard_to(fragment, output, timer, control, None)
    }

    pub fn run_fragment_controlled_with_keyboard_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
        control: &RunControl,
        keyboard: Option<&mut dyn hosted_keyboard::HostedKeyboardAdapter>,
    ) -> Result<StdRunReport, String> {
        write_operator_report(output, self.advertisement(), &fragment.plan_id, &fragment)?;

        let installed_standard = installed_std::supports(&fragment);
        if !installed_standard && !is_installed_kernel_signal_profile(&fragment) {
            return Err("fragment does not match the installed std kernel profile".to_string());
        }

        let advertisement = self.advertisement().clone();
        let reservation = self
            .kernel_resources
            .prepare_and_reserve(&advertisement, &fragment)?;
        let result = (|| {
            let play = self.issue_kernel_play(&fragment)?;
            let play_sequence = play.identity.play_sequence;
            if installed_standard {
                installed_std::run_fragment(
                    installed_std::InstalledRunHost {
                        advertisement: &advertisement,
                        playback: self.playback.as_ref(),
                        midi_input: self.midi_input.as_ref(),
                        midi_output: self.midi_output.as_ref(),
                        keyboard,
                        local_model: self.local_model.as_deref_mut(),
                        vector_search: self.vector_search.as_deref_mut(),
                        calendar: self.calendar.as_deref_mut(),
                    },
                    &fragment,
                    play_sequence,
                    &mut self.next_kernel_sign_sequence,
                    output,
                    timer,
                    control,
                )
            } else {
                if control.requested_stop().is_some() {
                    return Err("kernel-signal profile cannot accept generic Run control".into());
                }
                kernel_signal::run_signal_fragment(
                    &advertisement,
                    &fragment,
                    play_sequence,
                    &mut self.next_kernel_sign_sequence,
                    output,
                    timer,
                )
            }
        })();
        let release = self.kernel_resources.release(reservation);
        let report = result?;
        release?;
        writeln!(output, "plan {} complete", fragment.plan_id.as_str())
            .map_err(|error| error.to_string())?;
        if let (Some(first), Some(last)) = (report.receipts.first(), report.receipts.last()) {
            writeln!(
                output,
                "receipts {} first=({}, {}) last=({}, {})",
                report.receipts.len(),
                first.sequence,
                first.level,
                last.sequence,
                last.level
            )
            .map_err(|error| error.to_string())?;
        } else {
            writeln!(output, "receipts 0").map_err(|error| error.to_string())?;
        }
        Ok(report)
    }
}

fn is_installed_kernel_signal_profile(fragment: &PlanFragment) -> bool {
    matches!(
        (fragment.placements.len(), fragment.connections.len()),
        (2, 1) | (4, 3)
    ) && fragment
        .placements
        .iter()
        .filter(|placement| placement.kind_id.as_str() == PULSE_KIND)
        .count()
        == 1
        && fragment
            .placements
            .iter()
            .filter(|placement| placement.kind_id.as_str() == SHOW_KIND)
            .count()
            == fragment.placements.len().saturating_sub(1)
        && fragment
            .connections
            .iter()
            .all(|connection| connection.selected_line.is_none())
}

pub fn load_placements(
    path: Option<&str>,
) -> Result<Option<PlacementChoices>, Box<dyn std::error::Error>> {
    match path {
        Some(path) => Ok(Some(parse_placements(&fs::read_to_string(path)?)?)),
        None => Ok(None),
    }
}

fn write_operator_report<W: Write>(
    out: &mut W,
    advertisement: &HostAdvertisement,
    plan_id: &PlanId,
    fragment: &PlanFragment,
) -> Result<(), String> {
    writeln!(
        out,
        "host {} boot {} profile {} protocol {}",
        advertisement.host_id.as_str(),
        advertisement.boot_id.as_str(),
        advertisement.profile.as_str(),
        advertisement.protocol_version
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        out,
        "plan {} source_document={} checked_form={} expanded_form={}",
        plan_id.as_str(),
        fragment.source_document_id.as_str(),
        fragment.checked_form_id.as_str(),
        fragment.expanded_form_id.as_str()
    )
    .map_err(|error| error.to_string())?;
    for placement in &fragment.placements {
        writeln!(
            out,
            "place {} kind={} host={} boot={} capability={} implementation={} artifact={}",
            placement.gear_id.as_str(),
            placement.kind_id.as_str(),
            placement.host_id.as_str(),
            placement.boot_id.as_str(),
            placement.capability_id.as_str(),
            placement.implementation_id.as_str(),
            placement.artifact_id.as_str()
        )
        .map_err(|error| error.to_string())?;
    }
    for connection in &fragment.connections {
        writeln!(
            out,
            "connection {} {}:{} > {}:{} line={} base={:?} queue={}",
            connection.connection_id.as_str(),
            connection.source_placement_id.as_str(),
            connection.source_port_id.as_str(),
            connection.sink_placement_id.as_str(),
            connection.sink_port_id.as_str(),
            connection
                .selected_line
                .as_ref()
                .map_or("local", |line| line.line_id.as_str()),
            connection
                .selected_line
                .as_ref()
                .map(|line| line.binding.base.clone())
                .unwrap_or(BaseImplementationId::from("conduit.base/local@1")),
            connection.item_capacity
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{StdHost, StdHostConfig, TimerAdapter};
    use conduit_core::{
        seal_plan, BootId, ConnectionId, FormIdentity, HostId, OfferGeneration, PortDirection,
        PortId,
    };
    use conduit_form::parse_with_startup;
    use conduit_signal::signal_profile_catalog;
    use std::time::Duration;

    fn sealed_play_start_region(source: &str) -> &str {
        source
            .split_once("SEALED PROFILE PLAY START BEGIN")
            .and_then(|(_, remainder)| {
                remainder
                    .split_once("SEALED PROFILE PLAY START END")
                    .map(|(trigger, _)| trigger)
            })
            .expect("sealed Play-start markers remain paired")
    }

    #[test]
    fn sealed_profiles_do_not_reenter_semantic_or_allocating_preparation() {
        let forbidden = [
            "fragment.",
            "lowered.",
            "kind_id(",
            "base",
            "registry",
            ".find(",
            ".collect(",
            "Vec::",
            "vec![",
            ".to_vec(",
            ".clone(",
            ".reserve(",
        ];
        for (name, source) in [
            ("signal", include_str!("kernel_signal.rs")),
            ("multi-value", include_str!("kernel_multivalue.rs")),
        ] {
            let trigger = sealed_play_start_region(source);
            for token in forbidden {
                assert!(
                    !trigger.contains(token),
                    "{name} sealed Play-start region reintroduced '{token}'"
                );
            }
        }
    }

    #[test]
    fn exact_signal_fragment_lowers_to_numeric_kernel_tables() {
        let host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("lowering-host"),
            boot_id: BootId::from("lowering-boot"),
            offer_generation: OfferGeneration(1),
        });
        let form = parse_with_startup(
            include_str!("../../../proof/fixtures/forms/signal-demo.conduit"),
            &conduit_signal::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .expect("signal form parses");
        let plan = host.plan_local(&form, None).expect("local plan resolves");
        let fragment = &plan.fragments[0];
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
            .expect("exact fragment lowers");
        let narrow_profile = conduit_plan_lowering::lowering::KernelStorageProfile::new(1).unwrap();
        conduit_plan_lowering::lowering::lower_plan_fragment_for_profile(fragment, narrow_profile)
            .expect("the selected one-port profile admits the exact signal fragment");

        assert_eq!(lowered.identity.plan_id, fragment.plan_id);
        assert_eq!(lowered.identity.fragment_id, fragment.fragment_id);
        assert_eq!(lowered.nodes.len(), 2);
        assert_eq!(lowered.cords.len(), 1);
        assert_eq!(lowered.routes.len(), 1);
        assert_eq!(lowered.host_operations.len(), 2);
        assert_eq!(lowered.resources.len(), 2);
        assert_eq!(lowered.cord_value_slots, 4);
        assert_eq!(lowered.cord_value_bytes, 64);
        assert_eq!(lowered.sign_items, fragment.expected_sign.len() as u16);
        assert_eq!(lowered.identity.placements.len(), 2);
        assert_eq!(lowered.identity.connections.len(), 1);
        assert_eq!(lowered.identity.ports.len(), 2);
        for (node, placement) in &lowered.identity.placements {
            assert_eq!(lowered.identity.placement_for_node(*node), Some(placement));
            assert_eq!(lowered.identity.node_for_placement(placement), Some(*node));
        }
        for (cord, connection) in &lowered.identity.connections {
            assert_eq!(
                lowered.identity.connection_for_cord(*cord),
                Some(connection)
            );
            assert_eq!(
                lowered.identity.cord_for_connection(connection),
                Some(*cord)
            );
        }
        for port in &lowered.identity.ports {
            assert_eq!(
                lowered
                    .identity
                    .port_identity(port.node, port.direction, port.port),
                Some(port)
            );
            assert_eq!(
                lowered
                    .identity
                    .port_for_identity(port.node, port.direction, &port.port_id),
                Some(port.port)
            );
        }
        for (node, operation, contract) in &lowered.identity.host_operations {
            assert_eq!(
                lowered.identity.host_operation_contract(*node, *operation),
                Some(contract)
            );
            assert_eq!(
                lowered
                    .identity
                    .host_operation_for_contract(*node, contract),
                Some(*operation)
            );
        }
        assert!(lowered
            .identity
            .ports
            .iter()
            .any(|port| port.direction == PortDirection::Input));
        assert!(lowered
            .identity
            .ports
            .iter()
            .any(|port| port.direction == PortDirection::Output));
        assert_eq!(lowered.signs.len(), fragment.expected_sign.len());
        assert!(lowered
            .host_operations
            .iter()
            .any(|operation| operation.binding.maximum_output_bytes == 0));
        assert_eq!(
            lowered.node_specs[1].input_cords[0],
            Some(lowered.cords[0].spec.cord)
        );

        let mut mutated = fragment.clone();
        mutated.fragment_id = conduit_core::FragmentId::from("mutated-after-seal");
        assert!(matches!(
            conduit_plan_lowering::lowering::lower_plan_fragment(&mutated),
            Err(conduit_plan_lowering::lowering::LoweringError::InvalidFragment)
        ));

        let form_identity = FormIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_form_id: fragment.checked_form_id.clone(),
            expanded_form_id: fragment.expanded_form_id.clone(),
        };
        let mut concurrent = fragment.clone();
        concurrent.placements[0].host_operations[0].maximum_in_flight = 2;
        let concurrent = seal_plan(form_identity.clone(), vec![concurrent]);
        assert!(matches!(
            conduit_plan_lowering::lowering::lower_plan_fragment(&concurrent.fragments[0]),
            Err(
                conduit_plan_lowering::lowering::LoweringError::UnsupportedHostOperationConcurrency(
                    _
                )
            )
        ));

        let mut fan_in = fragment.clone();
        let mut second = fan_in.connections[0].clone();
        second.connection_id = ConnectionId::from("second-cord-to-same-input");
        fan_in.connections.push(second);
        let fan_in = seal_plan(form_identity, vec![fan_in]);
        assert!(matches!(
            conduit_plan_lowering::lowering::lower_plan_fragment(&fan_in.fragments[0]),
            Err(conduit_plan_lowering::lowering::LoweringError::MultipleConnectionsToInput { .. })
        ));

        let form_identity = FormIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_form_id: fragment.checked_form_id.clone(),
            expanded_form_id: fragment.expanded_form_id.clone(),
        };
        let mut remote = fragment.clone();
        let foreign_line: conduit_core::AdmittedLine =
            (&conduit_signal_conformance::distributed_websocket_line_offer()).into();
        remote.connections[0].selected_line = Some(foreign_line.clone());
        remote.connections[0].admitted_lines = vec![foreign_line];
        let remote = seal_plan(form_identity.clone(), vec![remote]);
        assert!(matches!(
            conduit_plan_lowering::lowering::lower_plan_fragment(&remote.fragments[0]),
            Err(conduit_plan_lowering::lowering::LoweringError::InvalidFragment)
                | Err(conduit_plan_lowering::lowering::LoweringError::InvalidRemoteConnection(_))
        ));

        let mut too_wide = fragment.clone();
        let output = too_wide.placements[0].outputs[0].clone();
        for index in 1..=16 {
            let mut extra = output.clone();
            extra.port_id = PortId::from(format!("extra-output-{index}"));
            too_wide.placements[0].outputs.push(extra);
        }
        let too_wide = seal_plan(form_identity, vec![too_wide]);
        assert!(matches!(
            conduit_plan_lowering::lowering::lower_plan_fragment(&too_wide.fragments[0]),
            Err(
                conduit_plan_lowering::lowering::LoweringError::ProfileCapacityExceeded {
                    direction: PortDirection::Output,
                    required: 17,
                    available: 16,
                    ..
                }
            )
        ));

        let mut profile_wide = fragment.clone();
        let mut second_output = profile_wide.placements[0].outputs[0].clone();
        second_output.port_id = PortId::from("second-output");
        profile_wide.placements[0].outputs.push(second_output);
        let profile_wide = seal_plan(
            FormIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_form_id: fragment.checked_form_id.clone(),
                expanded_form_id: fragment.expanded_form_id.clone(),
            },
            vec![profile_wide],
        );
        assert!(matches!(
            conduit_plan_lowering::lowering::lower_plan_fragment_for_profile(
                &profile_wide.fragments[0],
                narrow_profile,
            ),
            Err(
                conduit_plan_lowering::lowering::LoweringError::ProfileCapacityExceeded {
                    direction: PortDirection::Output,
                    required: 2,
                    available: 1,
                    ..
                }
            )
        ));
    }

    #[derive(Default)]
    struct VirtualTimer {
        waits: Vec<Duration>,
    }

    impl TimerAdapter for VirtualTimer {
        fn wait(&mut self, duration: Duration) {
            self.waits.push(duration);
        }
    }

    #[test]
    fn fresh_starts_get_fresh_boot_ids() {
        let first = StdHost::new();
        let second = StdHost::new();
        assert_ne!(
            first.advertisement().boot_id.as_str(),
            second.advertisement().boot_id.as_str()
        );
    }

    #[test]
    fn deterministic_boot_ids_are_injectable() {
        let host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("test-host"),
            boot_id: BootId::from("boot-test"),
            offer_generation: OfferGeneration(9),
        });
        assert_eq!(host.advertisement().boot_id.as_str(), "boot-test");
        assert_eq!(host.advertisement().offer_generation.0, 9);
    }

    #[test]
    fn streamed_output_uses_a_virtual_clock_and_retains_terminal_sign() {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("test-host"),
            boot_id: BootId::from("virtual-clock-boot"),
            offer_generation: OfferGeneration(1),
        });
        let form = parse_with_startup(
            "form virtual {\n pulse: flow/pulse(count = 3, period-ms = 7, initial = false)\n show: presentation/show\n pulse > show\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
        .expect("virtual-clock form parses");
        let plan = host.plan_local(&form, None).expect("local plan resolves");
        let fragment = plan.fragments[0].clone();
        let plan_id = fragment.plan_id.clone();
        let mut output = Vec::with_capacity(65_536);
        let mut timer = VirtualTimer {
            waits: Vec::with_capacity(2),
        };
        let report = host
            .run_fragment_to(fragment, &mut output, &mut timer)
            .expect("streamed run completes");

        assert_eq!(timer.waits, vec![Duration::from_millis(7); 2]);
        let output = String::from_utf8(output).expect("stream is utf-8");
        assert!(output.lines().any(|line| line == "signal 0 off"));
        assert!(output.lines().any(|line| line == "signal 1 on"));
        assert!(output.lines().any(|line| line == "signal 2 off"));
        assert!(output
            .lines()
            .any(|line| line.starts_with("receipt signal placement=")
                && line.ends_with(" sequence=0 level=false")));
        assert!(output
            .lines()
            .any(|line| line.starts_with("receipt signal placement=")
                && line.ends_with(" sequence=2 level=false")));
        assert!(output.contains("receipts 3 first=(0, false) last=(2, false)"));
        assert_eq!(report.receipts.len(), 3);
        assert_eq!(report.receipts[0].sequence, 0);
        assert!(!report.receipts[0].level);
        assert_eq!(report.receipts[2].sequence, 2);
        assert!(!report.receipts[2].level);
        let kernel = report.kernel.as_ref().expect("signal pair uses kernel");
        assert!(kernel.decisions > 0);
        assert!(kernel.kernel_events > 0);
        assert_ne!(kernel.active_play_id.as_str(), plan_id.as_str());
        assert_eq!(kernel.presentation_ids.len(), 3);
        assert_eq!(kernel.identity.plan_id, plan_id);
        assert_eq!(kernel.identity.active_play_id, kernel.active_play_id);
        assert_eq!(kernel.identity.lengths(), (5, 3, 4));
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert!(kernel
            .presentation_ids
            .windows(2)
            .all(|pair| pair[0] != pair[1]));
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(
            report
                .observations
                .iter()
                .filter(|observation| {
                    observation.active_play_id.as_ref() == Some(&kernel.active_play_id)
                        && observation.presentation_id.is_some()
                })
                .count()
                == 3
        );
        for observation in &report.observations {
            let sign = kernel
                .identity
                .sign_identity(&observation.sign_id)
                .expect("host sign reverses to its kernel identity row");
            assert_eq!(
                sign.presentation_id.as_ref(),
                observation.presentation_id.as_ref()
            );
            if let Some(presentation_id) = &observation.presentation_id {
                let presentation = kernel
                    .identity
                    .presentation(presentation_id)
                    .expect("presentation reverses to one kernel request");
                let request = kernel
                    .identity
                    .request(presentation.node, presentation.request)
                    .expect("presentation request reverses to its host-operation contract");
                assert!(kernel
                    .identity
                    .request_for_contract(presentation.node, &request.contract_id)
                    .any(|candidate| candidate == request));
                assert_eq!(
                    kernel
                        .identity
                        .presentation_for_request(presentation.node, presentation.request)
                        .map(|identity| &identity.presentation_id),
                    Some(presentation_id)
                );
                assert_eq!(
                    kernel
                        .identity
                        .sign_for_presentation(presentation_id)
                        .map(|identity| &identity.sign_id),
                    Some(&observation.sign_id)
                );
            }
        }
        assert!(report.observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Completed
            }
        )));
    }

    #[test]
    fn local_three_sink_signal_fanout_uses_only_the_sealed_kernel_profile() {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: BootId::from("fanout-boot"),
            offer_generation: OfferGeneration(1),
        });
        let form = parse_with_startup(
            include_str!("../../../proof/fixtures/forms/triple-signal.conduit"),
            &conduit_signal::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .expect("triple signal form parses");
        let placements = conduit_planner::parse_placements(include_str!(
            "../../../proof/fixtures/placements/triple-local.placements"
        ))
        .expect("triple local placements parse");
        let plan = host
            .plan_local(&form, Some(&placements))
            .expect("triple local plan resolves");
        let fragment = plan.fragments[0].clone();
        let mut output = Vec::with_capacity(65_536);
        let mut timer = VirtualTimer {
            waits: Vec::with_capacity(15),
        };
        let report = host
            .run_fragment_to(fragment, &mut output, &mut timer)
            .expect("triple local kernel run completes");

        assert_eq!(timer.waits, vec![Duration::from_millis(250); 15]);
        assert_eq!(report.receipts.len(), 48);
        let kernel = report.kernel.expect("triple local form uses kernel");
        assert_eq!(kernel.identity.lengths(), (63, 48, 49));
        assert_eq!(kernel.post_play_start_allocations, 0);
    }

    #[test]
    fn unsupported_production_std_form_fails_closed_without_a_legacy_pump() {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: BootId::from("unsupported-form-boot"),
            offer_generation: OfferGeneration(1),
        });
        let form = parse_with_startup(
            "form wider {\n first: flow/pulse(count = 1)\n second: flow/pulse(count = 1)\n left: presentation/show\n right: presentation/show\n first > left\n second > right\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
        .expect("unsupported wider form remains semantically valid");
        let plan = host
            .plan_local(&form, None)
            .expect("wider local plan resolves");
        let mut output = Vec::with_capacity(8_192);
        let mut timer = VirtualTimer::default();

        let error = host
            .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
            .expect_err("production std host must not fall back to the legacy pump");

        assert_eq!(
            error,
            "fragment does not match the installed std kernel profile"
        );
        assert!(timer.waits.is_empty());
    }
}
