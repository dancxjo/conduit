use conduit_core::{
    BaseImplementationId, HostAdvertisement, HostId, Observation, OfferGeneration, Plan,
    PlanFragment, PlanId,
};
use conduit_planner::{default_placements, parse_placements, plan, PlacementChoices};
use conduit_plot::CheckedPlot;
use conduit_signal::{PULSE_KIND, SHOW_KIND};
use std::fs;
use std::io::Write;
use std::thread;
use std::time::{Duration, Instant};

pub mod acoustic_emergency;
#[cfg(all(target_os = "linux", feature = "bluetooth-bluez"))]
pub mod bluetooth_gatt;
pub mod body_causal_evidence;
pub mod body_clock_line;
pub mod body_coordination;
pub mod body_execution;
mod boot_identity;
pub mod browser_admission;
pub mod civil_deadline_wait;
mod composition;
#[cfg(test)]
mod composition_test_offers;
#[cfg(feature = "confined-gear")]
pub mod confined_gear;
mod copy_task;
mod deadline_reactor;
#[cfg(feature = "local-model-proof")]
pub mod distributed_house_plan;
pub mod distributed_signal;
pub mod distributed_toggle;
pub mod relay_client;
pub mod remote_emergency;
pub mod text_lab_live;
pub mod text_lab_split;
#[cfg(feature = "local-model-proof")]
pub mod whisper_clip_proof;
pub use composition::{reference_advertisement, supported_nucleus_offers, StdHostComposition};
pub use conduit_std_offers::hosted_keyboard_offer;
#[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
pub use copy_task::prepare_isolated_copy_task;
pub use copy_task::{
    prepare_copy_task, CopyRequestId, CopyResult, CopyRunReceipt, CopyStopToken, PreparedCopyTask,
    ProtectedFileAvailability, ProtectedFileRegistry,
};
pub use deadline_reactor::{
    DeadlineClock, DeadlineClockError, DeadlineHostAdapter, DeadlineHostError, DeadlineKey,
    DeadlineReactor, DeadlineReactorError, DeadlineWake, ThreadMonotonicClock,
};
mod body_live_fore;
pub mod external_signal;
pub mod external_websocket;
pub mod flow_activation;
pub use body_live_fore::{BodyLiveForeAdmission, BodyLiveForeQueue, BodyLiveForeStatus};
mod host_execution;
pub mod hosted_audio;
mod hosted_body_conversation_context;
pub use hosted_body_conversation_context::{
    BodyConversationContextReplacement, BodyConversationContextSource,
    BodyConversationContextUpdateRefusal,
};
pub mod hosted_calendar;
pub mod hosted_data;
pub mod hosted_geometry;
pub mod hosted_history;
pub mod hosted_http;
pub mod hosted_indicator;
pub mod hosted_job;
pub mod hosted_keyboard;
mod hosted_language;
mod hosted_process;
#[cfg(feature = "local-model-proof")]
mod house_conversation_topology;
#[cfg(unix)]
pub mod pico_indicator;
mod whisper_installation;
pub use host_execution::HostedRunAdapters;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalForeInput {
    pub front_port_id: conduit_core::PortId,
    pub track: conduit_core::ConnectionTrack,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalForeDelivery {
    pub front_port_id: conduit_core::PortId,
    pub track: conduit_core::ConnectionTrack,
    pub value_kind: conduit_core::KindId,
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

/// A host acknowledges external Fore output only after its exact effect has
/// been accepted. Returning an error leaves the kernel value undelivered.
pub trait ExternalForeOutputAdapter {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String>;
}
pub mod direct_spoken_mask;
pub mod direct_spoken_mask_runtime;
pub mod hosted_linguistics;
pub mod hosted_local_model;
pub mod hosted_messaging;
pub mod hosted_microphone;
pub mod hosted_midi;
pub mod hosted_model;
pub mod hosted_model_compute;
pub mod hosted_network;
pub mod hosted_reminder;
pub mod hosted_resource;
pub mod hosted_speech_recognition;
pub mod hosted_speech_synthesis;
mod hosted_spoken_output_host;
pub mod hosted_synth;
#[cfg(unix)]
pub mod hosted_terminal_mask_host;
pub mod hosted_vector_index;
pub mod hosted_vector_search;
pub mod hosted_vision;
pub mod hosted_wav_artifact;
#[cfg(test)]
mod image_binding_tests;
mod installed_std;
pub mod llm_spoken_mask;
pub mod spoken_face_mask;
pub mod spoken_face_stream_execution;
pub mod spoken_mask_journey;
pub mod spoken_mask_runtime;
#[cfg(test)]
mod spoken_mask_runtime_tests;
pub mod terminal_face_mask;
pub mod terminal_mask_execution;
pub mod todo_checkpoint_call;
pub mod todo_checkpoint_read_call;
mod todo_checkpoint_transition;
pub mod todo_durable_resource;
mod vision_ocr;
mod vision_tracker;

pub use installed_std::{InstalledRemoteFragment, RemoteHostWork, RemoteValueTransfer};
pub use vision_ocr::{
    encode_graymap, visit_tesseract_tsv, OcrCandidate, OcrProviderRefusal, TesseractOcrProvider,
    MAXIMUM_OCR_ITEMS,
};
mod remote_host_fragment;
pub use remote_host_fragment::{AdmittedRemoteFragment, RemoteBodyTimeStepRefusal};
#[cfg(test)]
mod body_chat_tests;
#[cfg(test)]
mod installed_std_tests;
#[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
pub mod isolated_base;
#[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
pub mod isolated_copy_base;
pub mod microphone_whisper_proof;
#[cfg(feature = "native-webrtc")]
pub mod native_webrtc;
#[cfg(all(target_os = "linux", feature = "isolated-file-base"))]
pub use isolated_copy_base::provider_main as isolated_copy_provider_main;
#[cfg(all(target_os = "linux", feature = "isolated-http-base"))]
pub mod isolated_http_base;
#[cfg(all(target_os = "linux", feature = "isolated-http-base"))]
pub use isolated_http_base::provider_main as isolated_http_provider_main;
pub mod kernel_multivalue;
mod kernel_preparation;
mod kernel_signal;
mod local_model_observation;
mod local_model_pool_member;
#[cfg(feature = "local-model-proof")]
pub mod local_model_proof;
mod run_control;
#[cfg(feature = "local-model-proof")]
pub mod spoken_birth_journey;
pub mod state_value;
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
pub mod pool_member_sessions;
pub use local_model_pool_member::AdmittedLocalModelPoolMember;
pub mod pool_webchat;
pub mod r1_control;
pub mod r1_control_input;
pub mod reaction_diffusion;
pub mod remote_cord_sessions;
pub mod ros2_base;
pub use conduit_plan_lowering::shared_pool_runtime;
pub use reaction_diffusion::*;
pub mod secure_websocket;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolatedFileBaseConfig {
    pub executable: std::path::PathBuf,
    pub base_instance_id: conduit_core::BaseInstanceId,
    pub provider_generation: u64,
}

impl IsolatedFileBaseConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.executable.is_file()
            || self.base_instance_id.as_str().is_empty()
            || self.provider_generation == 0
        {
            return Err("isolated file Base configuration is not current and exact".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct StdRunReport {
    pub observations: Vec<Observation>,
    pub receipts: Vec<SignalReceipt>,
    pub kernel: Option<StdKernelExecutionReport>,
    pub control_receipts: Vec<RunControlReceipt>,
    pub speech_recognition: Vec<SpeechRecognitionExecutionReceipt>,
    pub microphone: Vec<hosted_microphone::MicrophoneCaptureReceipt>,
    pub external_fore_deliveries: Vec<ExternalForeDelivery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechRecognitionExecutionReceipt {
    pub plan_id: conduit_core::PlanId,
    pub active_play_id: conduit_core::ActivePlayId,
    pub placement_id: conduit_core::PlacementId,
    pub implementation_id: conduit_core::ImplementationId,
    pub executable_sha256: String,
    pub model_sha256: String,
    pub audio_sha256: [u8; 32],
    pub text_sha256: Option<String>,
    pub text_bytes: u16,
    pub diagnostic_bytes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdKernelExecutionReport {
    pub active_play: conduit_core::ActivePlayIdentity,
    pub active_play_id: conduit_core::ActivePlayId,
    pub decisions: u32,
    pub kernel_events: u16,
    pub kernel_sign: Vec<conduit_kernel::KernelEvent>,
    pub value_allocation_capacity_before: (usize, usize),
    pub value_allocation_capacity_after: (usize, usize),
    pub presentation_ids: Vec<conduit_core::PresentationId>,
    pub playback: Vec<hosted_audio::PlaybackReport>,
    pub wav_artifacts: Vec<hosted_wav_artifact::WavArtifactReport>,
    pub midi_input: Vec<hosted_midi::MidiInputReport>,
    pub midi_output: Vec<hosted_midi::MidiOutputReport>,
    pub identity: conduit_plan_lowering::lowering::KernelExecutionIdentityMap,
    pub fore_endpoints: Vec<conduit_plan_lowering::lowering::KernelForeEndpointIdentity>,
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

    /// An exact sample on an identified provider basis. Adapters that only
    /// offer a duration or an unlabelled counter leave event time unsupported.
    fn monotonic_observation(
        &mut self,
        _host_id: &conduit_core::HostId,
        _boot_id: &conduit_core::BootId,
    ) -> Option<conduit_core::MonotonicInstant> {
        None
    }

    /// Returns the current host/boot-scoped monotonic millisecond reading when
    /// this adapter offers the admitted deadline contract.
    fn monotonic_now_ms(&mut self) -> Option<u64> {
        None
    }

    /// Returns the admitted host/Boot-scoped monotonic microsecond reading.
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
static THREAD_TIMER_BASIS: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

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

    fn monotonic_observation(
        &mut self,
        host_id: &conduit_core::HostId,
        boot_id: &conduit_core::BootId,
    ) -> Option<conduit_core::MonotonicInstant> {
        let basis = THREAD_TIMER_BASIS
            .get_or_init(|| {
                let mut nonce = [0u8; 16];
                getrandom::fill(&mut nonce).ok()?;
                Some(format!(
                    "std/thread-timer/process-epoch/{:08x}{}",
                    std::process::id(),
                    nonce
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                ))
            })
            .as_ref()?;
        let identity = conduit_core::MonotonicClockIdentity::new(
            host_id.clone(),
            boot_id.clone(),
            basis.clone(),
            conduit_core::TemporalScale::Microseconds,
            1,
            1,
        )
        .ok()?;
        conduit_core::MonotonicInstant::new(self.monotonic_now_micros()?, identity).ok()
    }
}

pub fn run_kernel_multivalue_path_to<W: Write, T: TimerAdapter>(
    path: &str,
    output: &mut W,
    timer: &mut T,
) -> Result<kernel_multivalue::MultiValueRunReport, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let plot = conduit_plot::parse(&source, &kernel_multivalue::profile_catalog())
        .map_err(|error| error.to_string())?;
    let advertisement = kernel_multivalue::advertisement(
        HostId::from("std-host-1"),
        conduit_core::BootId::from(boot_identity::fresh_boot_id()),
        OfferGeneration(1),
    );
    let plan =
        kernel_multivalue::plan_local(&plot, &advertisement).map_err(|error| error.to_string())?;
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

struct TodoCheckpointResidenceRoot {
    path: std::path::PathBuf,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

pub struct StdHost {
    advertisement: HostAdvertisement,
    #[cfg(unix)]
    terminal_attachment: Option<std::os::unix::net::UnixStream>,
    image_identity: Option<conduit_host_make::ImageBootIdentity>,
    playback: Option<hosted_audio::HostedPlaybackSelection>,
    wav_artifact: Option<hosted_wav_artifact::WavArtifactSelection>,
    speech_synthesis: Option<hosted_speech_synthesis::EspeakSpeechAdapter>,
    midi_input: Option<hosted_midi::HostedRawMidiSelection>,
    midi_output: Option<hosted_midi::MidiOutputSelection>,
    local_model: Option<Box<dyn hosted_local_model::HostedLocalModelAdapter>>,
    speech_recognition: Option<hosted_speech_recognition::WhisperSpeechAdapter>,
    microphone: Option<hosted_microphone::AlsaMicrophoneAdapter>,
    base_registry: conduit_core::BaseRegistry,
    vector_search: Option<Box<dyn hosted_vector_search::HostedVectorSearchAdapter>>,
    calendar: Option<Box<dyn hosted_calendar::HostedCalendarAdapter>>,
    body_conversation_context: Option<BodyConversationContextSource>,
    vision: Option<hosted_vision::FiniteHostedVisionBase>,
    kernel_resources: kernel_preparation::KernelResourceLedger,
    todo_checkpoint_root: Option<TodoCheckpointResidenceRoot>,
    next_kernel_play_sequence: u64,
    next_kernel_sign_sequence: u64,
}

fn empty_base_registry() -> conduit_core::BaseRegistry {
    conduit_core::BaseRegistry::new(conduit_core::BaseRegistryLimits {
        maximum_bases: 16,
        maximum_capabilities_per_base: 16,
        maximum_resources_per_base: 16,
        maximum_advertised_capabilities: 256,
        maximum_advertised_resources: 256,
    })
    .expect("std Base registry bounds are fixed and valid")
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

fn normalize_capability_offers(
    capabilities: &mut Vec<conduit_core::CapabilityOffer>,
) -> Result<(), String> {
    capabilities.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    if let Some(conflict) = capabilities
        .windows(2)
        .find(|pair| pair[0].capability_id == pair[1].capability_id && pair[0] != pair[1])
    {
        return Err(format!(
            "Host composition produced conflicting offers for capability {}",
            conflict[0].capability_id.as_str()
        ));
    }
    capabilities.dedup_by(|left, right| left.capability_id == right.capability_id);
    Ok(())
}

impl StdHost {
    #[cfg(test)]
    fn install_test_capability(&mut self, offer: conduit_core::CapabilityOffer) {
        self.advertisement.capabilities.push(offer);
        self.advertisement
            .capabilities
            .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        self.kernel_resources = kernel_preparation::KernelResourceLedger::new(&self.advertisement)
            .expect("test capability ledger is exact");
    }

    pub fn install_body_conversation_context(
        &mut self,
        context: &conduit_body::BodyConversationContext,
    ) -> Result<(), String> {
        let offer = conduit_std_offers::body_conversation_context_std_offer();
        let newly_offered = !self
            .advertisement
            .capabilities
            .iter()
            .any(|installed| installed.capability_id == offer.capability_id);
        if newly_offered {
            self.advertisement.capabilities.push(offer);
            self.advertisement
                .capabilities
                .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        }
        if let Some(source) = &self.body_conversation_context {
            source
                .replace(context)
                .map_err(|error| format!("replace Body conversation context: {error:?}"))?;
        } else {
            self.body_conversation_context = Some(BodyConversationContextSource::new(context)?);
        }
        if newly_offered {
            self.advertisement.offer_generation = OfferGeneration(
                self.advertisement
                    .offer_generation
                    .0
                    .checked_add(1)
                    .ok_or_else(|| "Body context offer generation exhausted".to_string())?,
            );
        }
        Ok(())
    }

    pub fn body_conversation_context_source(&self) -> Option<BodyConversationContextSource> {
        self.body_conversation_context.clone()
    }

    pub fn issue_kernel_play(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<IssuedKernelPlay, String> {
        if fragment.host_id != self.advertisement.host_id
            || fragment.boot_id != self.advertisement.boot_id
        {
            return Err("Plan fragment is stale for this host boot".to_string());
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

    /// Continue a boot's Play sequence when a fresh hosted adapter is
    /// reconstructed for the same real Host Boot. The caller owns durable,
    /// exclusive allocation of the supplied sequence range.
    pub fn set_initial_kernel_play_sequence(&mut self, first: u64) -> Result<(), String> {
        if self.next_kernel_play_sequence != 0 || first == 0 {
            return Err("kernel Play sequence is already in use or invalid".into());
        }
        self.next_kernel_play_sequence = first;
        Ok(())
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

    /// Construct one std Host whose Todo scan Back is bound to the caller's
    /// canonical initial Form before planning or resource-ledger admission.
    /// The broad reference inventory remains unchanged.
    pub fn new_for_todo_scan(
        config: StdHostConfig,
        initial: &conduit_todo_plot::TodoState,
        maximum_items: u16,
    ) -> Result<Self, String> {
        let mut advertisement = composition::build_advertisement(
            config,
            StdHostComposition::reference(),
            None,
            None,
            None,
            false,
        );
        for offer in [
            flow_activation::todo_scan_offer(initial, maximum_items)?,
            flow_activation::todo_combine_offer(),
        ] {
            if advertisement
                .capabilities
                .iter()
                .any(|existing| existing.capability_id == offer.capability_id)
            {
                return Err("std Todo scan capability identity is already offered".into());
            }
            advertisement.capabilities.push(offer);
        }
        advertisement
            .capabilities
            .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        Self::from_advertisement(advertisement)
    }

    /// Advertise one exact Todo checkpoint publication and its selected
    /// ExternalDurable residence before resource-ledger admission. The root
    /// remains explicitly selected and must match the later Play call.
    pub fn new_for_todo_checkpoint_once(
        config: StdHostConfig,
        root: &std::path::Path,
        content: conduit_core::ResourceContentRequirement,
    ) -> Result<Self, String> {
        let offer =
            conduit_std_offers::todo_checkpoint_offer(content.clone()).map_err(str::to_string)?;
        Self::new_for_selected_todo_checkpoint(config, root, content, offer)
    }

    /// Advertise one exact ReadPublished generation and its separate read
    /// authority before ledger admission on the selected std Host.
    pub fn new_for_todo_checkpoint_read(
        config: StdHostConfig,
        root: &std::path::Path,
        content: conduit_core::ResourceContentRequirement,
    ) -> Result<Self, String> {
        let offer = conduit_std_offers::todo_checkpoint_read_offer(content.clone())
            .map_err(str::to_string)?;
        Self::new_for_selected_todo_checkpoint(config, root, content, offer)
    }

    fn new_for_selected_todo_checkpoint(
        config: StdHostConfig,
        root: &std::path::Path,
        content: conduit_core::ResourceContentRequirement,
        offer: conduit_core::CapabilityOffer,
    ) -> Result<Self, String> {
        let metadata = std::fs::symlink_metadata(root)
            .map_err(|error| format!("Todo checkpoint root: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("Todo checkpoint root must be an existing directory".into());
        }
        let root = root
            .canonicalize()
            .map_err(|error| format!("Todo checkpoint root: {error}"))?;
        let mut advertisement = composition::build_advertisement(
            config,
            StdHostComposition::reference(),
            None,
            None,
            None,
            false,
        );
        advertisement.resources.push(conduit_core::ResourceOffer {
            pool_id: "std/todo-checkpoint".into(),
            class_id: "resource/todo-checkpoint@1".into(),
            capacity_units: 1,
            compute: None,
            content: Some(conduit_core::ResourceContentOffer {
                contract: content,
                owner_host: advertisement.host_id.clone(),
                owner_boot: advertisement.boot_id.clone(),
                base_id: "std/explicit-shared-checkpoint".into(),
                residence_profile: conduit_core::kind_id("std/explicit-shared-checkpoint@1"),
            }),
        });
        advertisement.capabilities.push(offer);
        advertisement.resources.sort();
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let mut host = Self::from_advertisement(advertisement)?;
        #[cfg(unix)]
        let selected = {
            use std::os::unix::fs::MetadataExt;
            let metadata = root
                .metadata()
                .map_err(|error| format!("Todo checkpoint root: {error}"))?;
            TodoCheckpointResidenceRoot {
                path: root,
                device: metadata.dev(),
                inode: metadata.ino(),
            }
        };
        #[cfg(not(unix))]
        let selected = TodoCheckpointResidenceRoot { path: root };
        host.todo_checkpoint_root = Some(selected);
        Ok(host)
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
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
        }
    }

    pub fn new_with_finite_vision(
        config: StdHostConfig,
        composition: StdHostComposition,
        vision: hosted_vision::FiniteHostedVisionBase,
    ) -> Result<Self, String> {
        let mut advertisement =
            composition::build_advertisement(config, composition, None, None, None, false);
        let mut base_registry = empty_base_registry();
        let mut vision_capabilities = vec![
            hosted_vision::FiniteHostedVisionBase::motion_offer(),
            hosted_vision::FiniteHostedVisionBase::objects_offer(),
            hosted_vision::FiniteHostedVisionBase::experience_offer(),
        ];
        if let Some(ocr_offer) = vision.ocr_offer() {
            vision_capabilities.push(ocr_offer);
        }
        if let Some(describe_offer) = vision.describe_offer() {
            vision_capabilities.push(describe_offer);
        }
        base_registry
            .register(conduit_core::BaseProviderEntry {
                base_id: conduit_core::HostBaseId::from("std/base/finite-vision"),
                provider_instance_id: conduit_core::BaseInstanceId::from(
                    vision.provider_instance_id(),
                ),
                provider_generation: advertisement.offer_generation.0,
                implementation_id: conduit_core::BaseImplementationId::from(
                    conduit_std_offers::LOCAL_VISION_IMPLEMENTATION,
                ),
                mechanism_family: conduit_core::HostBaseKindId::from("std.base/finite-vision@1"),
                enforcement_class: conduit_core::BaseEnforcementClass::Cooperative,
                lifecycle: conduit_core::BaseLifecycle::Ready,
                capabilities: vision_capabilities,
                resources: vec![hosted_vision::FiniteHostedVisionBase::resource_offer()],
            })
            .map_err(|error| format!("finite vision Base registration: {error:?}"))?;
        base_registry
            .project_ready_into(&mut advertisement)
            .map_err(|error| format!("finite vision Base advertisement: {error:?}"))?;
        advertisement
            .capabilities
            .push(conduit_std_offers::local_vision_offers()[4].clone());
        advertisement.resources.sort();
        normalize_capability_offers(&mut advertisement.capabilities)?;
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry,
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: Some(vision),
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
        })
    }

    pub fn new_with_local_model(
        config: StdHostConfig,
        composition: StdHostComposition,
        adapter: Box<dyn hosted_local_model::HostedLocalModelAdapter>,
    ) -> Result<Self, String> {
        Self::new_with_local_model_capabilities(config, composition, adapter, Vec::new())
    }

    pub(crate) fn new_with_local_model_capabilities(
        config: StdHostConfig,
        composition: StdHostComposition,
        adapter: Box<dyn hosted_local_model::HostedLocalModelAdapter>,
        additional_capabilities: Vec<conduit_core::CapabilityOffer>,
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
        advertisement
            .capabilities
            .push(conduit_std_offers::house_prompt_std_offer());
        advertisement
            .capabilities
            .push(conduit_std_offers::body_chat_prompt_std_offer());
        advertisement
            .capabilities
            .push(conduit_std_offers::model_result_to_text_std_offer());
        advertisement
            .capabilities
            .push(conduit_std_offers::model_result_flow_to_text_std_offer());
        advertisement
            .capabilities
            .push(conduit_std_offers::generated_chunk_to_text_std_offer());
        advertisement
            .capabilities
            .push(conduit_std_offers::address_detect_offer());
        advertisement
            .capabilities
            .push(conduit_std_offers::recognition_to_text_std_offer());
        advertisement
            .capabilities
            .push(conduit_std_offers::committed_turn_to_text_std_offer());
        advertisement.capabilities.extend(additional_capabilities);
        advertisement.resources.sort();
        normalize_capability_offers(&mut advertisement.capabilities)?;
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: Some(adapter),
            speech_recognition: None,
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
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
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: Some(adapter),
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
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
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: Some(adapter),
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
        })
    }

    /// Executes against one platform-extended advertisement that was already
    /// published for this exact host/Boot. Rebuilding from only the generic
    /// composition here would discard admitted platform implementations.
    pub fn from_advertisement(advertisement: HostAdvertisement) -> Result<Self, String> {
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: None,
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
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
        let mut advertisement = composition::build_advertisement(
            config,
            composition,
            Some(&playback),
            None,
            None,
            false,
        );
        advertisement
            .capabilities
            .push(conduit_std_offers::audio_convert_pcm_profile_offer());
        advertisement.capabilities.sort_by(|left, right| {
            left.capability_id
                .as_str()
                .cmp(right.capability_id.as_str())
        });
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: None,
            playback: Some(playback),
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
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
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: Some(midi_output),
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
        })
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }

    pub fn image_identity(&self) -> Option<&conduit_host_make::ImageBootIdentity> {
        self.image_identity.as_ref()
    }

    pub fn from_image_binding(
        binding: conduit_host_make::BoundHostAdvertisement,
    ) -> Result<Self, String> {
        let (image_identity, advertisement) = binding.into_parts();
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
        Ok(Self {
            advertisement,
            image_identity: Some(image_identity),
            playback: None,
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
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
            wav_artifact: None,
            speech_synthesis: None,
            midi_input: None,
            midi_output: None,
            local_model: None,
            speech_recognition: None,
            microphone: None,
            base_registry: empty_base_registry(),
            vector_search: None,
            calendar: None,
            body_conversation_context: None,
            vision: None,
            kernel_resources,
            todo_checkpoint_root: None,
            next_kernel_play_sequence: 0,
            next_kernel_sign_sequence: 0,
            #[cfg(unix)]
            terminal_attachment: None,
        })
    }

    pub fn plan_local(
        &self,
        plot: &CheckedPlot,
        placements: Option<&PlacementChoices>,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let hosts = vec![self.advertisement().clone()];
        let placements = match placements {
            Some(placements) => placements.clone(),
            None => default_placements(plot, &hosts)?,
        };
        Ok(plan(
            plot,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
        )?)
    }

    pub fn plan_local_with_authority(
        &self,
        plot: &CheckedPlot,
        placements: Option<&PlacementChoices>,
        authority_grants: &[conduit_core::AuthorityGrant],
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let hosts = vec![self.advertisement().clone()];
        let placements = match placements {
            Some(placements) => placements.clone(),
            None => default_placements(plot, &hosts)?,
        };
        Ok(conduit_planner::plan_with_authority_grants(
            plot,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            authority_grants,
        )?)
    }

    /// Constructs the explicit grant shape for a caller that has independently
    /// authorized this exact selected playback capability. Merely constructing
    /// or discovering a host never calls this method.
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
            return Err("selected playback observation is stale for this host".into());
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
            host_call_contract_id: requirement.host_call_contract_id.clone(),
            subject_kind: requirement.subject_kind.clone(),
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            capability_id: capability.capability_id.clone(),
        })
    }

    pub fn wav_artifact_authority_grant(
        &self,
        grant_id: &str,
    ) -> Result<conduit_core::AuthorityGrant, String> {
        let artifact = self
            .wav_artifact
            .as_ref()
            .ok_or_else(|| "std Host has no selected WAV artifact destination".to_string())?;
        if artifact.boot_id != self.advertisement.boot_id
            || artifact.offer_generation != self.advertisement.offer_generation
        {
            return Err("selected WAV artifact destination is stale for this host".into());
        }
        let capability = self
            .advertisement
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::AUDIO_WAV_ARTIFACT_IMPLEMENTATION
            })
            .ok_or_else(|| "selected WAV artifact capability is not advertised".to_string())?;
        let requirement = capability
            .authority_requirements
            .first()
            .ok_or_else(|| "WAV artifact capability has no authority contract".to_string())?;
        Ok(conduit_core::AuthorityGrant {
            grant_id: conduit_core::AuthorityGrantId::from(grant_id),
            contract_id: requirement.contract_id.clone(),
            host_call_contract_id: requirement.host_call_contract_id.clone(),
            subject_kind: requirement.subject_kind.clone(),
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            capability_id: capability.capability_id.clone(),
        })
    }

    pub fn spoken_mask_artifact_authority_grant(
        &self,
        grant_id: &str,
    ) -> Result<conduit_core::AuthorityGrant, String> {
        let artifact = self
            .wav_artifact
            .as_ref()
            .ok_or_else(|| "std Host has no selected WAV artifact destination".to_string())?;
        if artifact.boot_id != self.advertisement.boot_id
            || artifact.offer_generation != self.advertisement.offer_generation
        {
            return Err("selected spoken artifact destination is stale for this host".into());
        }
        let capability = self
            .advertisement
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_offers::SPOKEN_ARTIFACT_IMPLEMENTATION
            })
            .ok_or_else(|| "spoken artifact capability is not advertised".to_string())?;
        let requirement = capability
            .authority_requirements
            .first()
            .ok_or_else(|| "spoken artifact capability has no authority contract".to_string())?;
        Ok(conduit_core::AuthorityGrant {
            grant_id: conduit_core::AuthorityGrantId::from(grant_id),
            contract_id: requirement.contract_id.clone(),
            host_call_contract_id: requirement.host_call_contract_id.clone(),
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
            return Err("selected MIDI output observation is stale for this host".into());
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
                host_call_contract_id: requirement.host_call_contract_id.clone(),
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
        plot: &conduit_plot::ExpandedCanonicalPlot,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let hosts = vec![self.advertisement().clone()];
        let placements = conduit_planner::default_expanded_placements(plot, &hosts)?;
        Ok(conduit_planner::plan_expanded_canonical(
            plot,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
        )?)
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
        "plan {} source_document={} checked_plot={} expanded_plot={}",
        plan_id.as_str(),
        fragment.source_document_id.as_str(),
        fragment.checked_plot_id.as_str(),
        fragment.expanded_plot_id.as_str()
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
        seal_plan, BootId, ConnectionId, HostId, OfferGeneration, PlotIdentity, PortDirection,
        PortId,
    };
    use conduit_plot::parse_with_startup;
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
        let plot = parse_with_startup(
            include_str!("../../../proof/fixtures/plots/signal-demo.conduit"),
            &conduit_signal::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .expect("signal plot parses");
        let plan = host.plan_local(&plot, None).expect("local plan resolves");
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
        assert_eq!(lowered.host_calls.len(), 2);
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
        for (node, operation, contract) in &lowered.identity.host_calls {
            assert_eq!(
                lowered.identity.host_call_contract(*node, *operation),
                Some(contract)
            );
            assert_eq!(
                lowered.identity.host_call_for_contract(*node, contract),
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
            .host_calls
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

        let plot_identity = PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        };
        let mut concurrent = fragment.clone();
        concurrent.placements[0].host_calls[0].maximum_in_flight = 2;
        let concurrent = seal_plan(plot_identity.clone(), vec![concurrent]);
        assert!(matches!(
            conduit_plan_lowering::lowering::lower_plan_fragment(&concurrent.fragments[0]),
            Err(conduit_plan_lowering::lowering::LoweringError::UnsupportedHostCallConcurrency(_))
        ));

        let mut fan_in = fragment.clone();
        let mut second = fan_in.connections[0].clone();
        second.connection_id = ConnectionId::from("second-cord-to-same-input");
        fan_in.connections.push(second);
        let fan_in = seal_plan(plot_identity, vec![fan_in]);
        assert!(matches!(
            conduit_plan_lowering::lowering::lower_plan_fragment(&fan_in.fragments[0]),
            Err(conduit_plan_lowering::lowering::LoweringError::MultipleConnectionsToInput { .. })
        ));

        let plot_identity = PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        };
        let mut remote = fragment.clone();
        let foreign_line: conduit_core::AdmittedLine =
            (&conduit_signal_conformance::distributed_websocket_line_offer()).into();
        remote.connections[0].selected_line = Some(foreign_line.clone());
        remote.connections[0].admitted_lines = vec![foreign_line];
        let remote = seal_plan(plot_identity.clone(), vec![remote]);
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
        let too_wide = seal_plan(plot_identity, vec![too_wide]);
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
            PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
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
        let plot = parse_with_startup(
            "plot virtual {\n pulse: flow/pulse(count = 3, period-ms = 7, initial = false)\n show: presentation/show\n pulse >> show\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
        .expect("virtual-clock plot parses");
        let plan = host.plan_local(&plot, None).expect("local plan resolves");
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
                    .expect("presentation request reverses to its Host Call contract");
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
        let plot = parse_with_startup(
            include_str!("../../../proof/fixtures/plots/triple-signal.conduit"),
            &conduit_signal::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .expect("triple signal plot parses");
        let placements = conduit_planner::parse_placements(include_str!(
            "../../../proof/fixtures/placements/triple-local.placements"
        ))
        .expect("triple local placements parse");
        let plan = host
            .plan_local(&plot, Some(&placements))
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
        let kernel = report.kernel.expect("triple local plot uses kernel");
        assert_eq!(kernel.identity.lengths(), (63, 48, 49));
        assert_eq!(kernel.post_play_start_allocations, 0);
    }

    #[test]
    fn unsupported_production_std_plot_fails_closed_without_a_legacy_pump() {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: BootId::from("unsupported-plot-boot"),
            offer_generation: OfferGeneration(1),
        });
        let plot = parse_with_startup(
            "plot wider {\n first: flow/pulse(count = 1)\n second: flow/pulse(count = 1)\n left: presentation/show\n right: presentation/show\n first >> left\n second >> right\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
        .expect("unsupported wider plot remains semantically valid");
        let plan = host
            .plan_local(&plot, None)
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
