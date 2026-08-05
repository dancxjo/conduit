use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionProvider,
    FailureReason, FormId, HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId,
    ImplementationId, KindId, Observation, ObservationKind, OfferGeneration, Plan, PlanFragment,
    PlanId, PlannedOperation, PlatformEffect, PROTOCOL_VERSION,
};
use conduit_form::{CheckedForm, KindDefinition, ProfileCatalog};
use conduit_planner::{default_placements, parse_placements, plan, PlacementChoices};
use conduit_runtime::{
    HostRuntime, ImplementationFailure, ImplementationRegistry, OperationAction,
    OperationCompletion, OperationImplementation, OperationState, RuntimeOutput,
};
use conduit_signal::{
    decode_signal, signal_profile_catalog, signal_registry, PULSE_KIND, SHOW_KIND,
    SIGNAL_PRESENTATION_KIND, SIGNAL_VALUE_KIND,
};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static BOOT_COUNTER: AtomicU64 = AtomicU64::new(1);
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

pub const COPY_FILE_KIND: &str = "task/copy-file";
pub const COPY_FILE_RECEIPT_KIND: &str = "task/copy-file-receipt";
pub const COPY_FILE_RECEIPT_VALUE_KIND: &str = "value/copy-file-receipt";
pub const COPY_FILE_CAPABILITY: &str = "copy-file-1";
pub const COPY_FILE_RECEIPT_CAPABILITY: &str = "copy-file-receipt-1";
pub const COPY_FILE_IMPLEMENTATION: &str = "std/copy-file-v1";
pub const COPY_FILE_RECEIPT_IMPLEMENTATION: &str = "std/copy-file-receipt-v1";
pub const COPY_FILE_ARTIFACT: &str = "conduit-std-host/copy-file-v1";
pub const COPY_FILE_RECEIPT_ARTIFACT: &str = "conduit-std-host/copy-file-receipt-v1";

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReceipt {
    pub placement_id: conduit_core::PlacementId,
    pub sequence: u64,
    pub level: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CopyReplaceMode {
    CreateOnly,
    ReplaceExisting,
    RejectExisting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyResourceBinding {
    pub binding_id: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyFileRequest {
    pub request_id: String,
    pub source: CopyResourceBinding,
    pub destination: CopyResourceBinding,
    pub replace_mode: CopyReplaceMode,
    pub max_bytes: u64,
    pub inspect: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CopyPreflight {
    WillCreate,
    WillReplace,
    RejectedDestinationExists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyTaskResult {
    Created { bytes_copied: u64 },
    Replaced { bytes_copied: u64 },
    RejectedDestinationExists,
    Denied { message: String },
    StaleSource { message: String },
    OversizedInput { size: u64, max_bytes: u64 },
    Cancelled,
    CleanupFailed { message: String },
    Failed { message: String },
}

#[derive(Debug, Clone)]
pub struct CopyTaskReport {
    pub request_id: String,
    pub run_id: String,
    pub source_binding_id: String,
    pub destination_binding_id: String,
    pub source_choice: String,
    pub destination_choice: String,
    pub inspect_requested: bool,
    pub preflight: CopyPreflight,
    pub result: CopyTaskResult,
    pub form_source: String,
    pub plan: Option<Plan>,
    pub observations: Vec<Observation>,
}

pub trait TimerAdapter {
    fn wait(&mut self, duration: Duration);
}

pub struct ThreadTimer;

impl TimerAdapter for ThreadTimer {
    fn wait(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

pub struct StdHost {
    runtime: HostRuntime,
}

impl Default for StdHost {
    fn default() -> Self {
        Self::new()
    }
}

impl StdHost {
    pub fn new() -> Self {
        Self::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: conduit_core::BootId::from(fresh_boot_id()),
            offer_generation: OfferGeneration(1),
        })
    }

    pub fn new_with_config(config: StdHostConfig) -> Self {
        let advertisement = build_advertisement(config);
        let registry = signal_registry(
            ImplementationId::from("std/pulse-v1"),
            ImplementationId::from("std/stdout-show-signal-v1"),
        )
        .expect("std signal implementations have unique identities");
        Self {
            runtime: HostRuntime::new(advertisement, registry, 256),
        }
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        self.runtime.advertisement()
    }

    pub fn handle(&mut self, command: HostCommand) -> RuntimeOutput {
        self.runtime.handle(command)
    }

    pub fn plan_local(
        &self,
        form: &CheckedForm,
        placements: Option<&PlacementChoices>,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let realm = vec![self.advertisement().clone()];
        let placements = match placements {
            Some(placements) => placements.clone(),
            None => default_placements(form, &realm)?,
        };
        Ok(plan(
            form,
            &realm,
            &placements,
            &[ConnectionProvider::Local],
        )?)
    }

    pub fn run_fragment_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
    ) -> Result<StdRunReport, String> {
        write_operator_report(
            output,
            self.advertisement(),
            &fragment.plan_id,
            &fragment.form_id,
            &fragment,
        )?;

        let prepare = self.runtime.handle(HostCommand::Prepare(fragment.clone()));
        if let Some(reason) = preparation_rejection(&prepare) {
            return Err(reason);
        }
        let activated_output = self
            .runtime
            .handle(HostCommand::Activate(fragment.plan_id.clone()));
        if let Some(reason) = activation_rejection(&activated_output) {
            return Err(reason);
        }

        let mut pending_effects = activated_output.effects;
        let mut receipts = Vec::new();
        while let Some(effect) = pending_effects.pop() {
            let follow_up = match effect {
                PlatformEffect::Wait {
                    plan_id,
                    placement_id,
                    duration_ms,
                } => {
                    timer.wait(Duration::from_millis(duration_ms));
                    self.runtime.handle(HostCommand::CompleteWait {
                        plan_id,
                        placement_id,
                    })
                }
                PlatformEffect::PresentValue {
                    plan_id,
                    placement_id,
                    presentation_kind,
                    value,
                } => {
                    if presentation_kind.as_str() != SIGNAL_PRESENTATION_KIND {
                        return Err(format!(
                            "std host cannot manifest presentation kind '{}'",
                            presentation_kind.as_str()
                        ));
                    }
                    let signal = decode_signal(&value).map_err(|err| err.to_string())?;
                    writeln!(
                        output,
                        "signal {} {}",
                        signal.sequence,
                        if signal.level { "on" } else { "off" }
                    )
                    .map_err(|error| error.to_string())?;
                    writeln!(
                        output,
                        "receipt signal placement={} sequence={} level={}",
                        placement_id.as_str(),
                        signal.sequence,
                        signal.level
                    )
                    .map_err(|error| error.to_string())?;
                    receipts.push(SignalReceipt {
                        placement_id: placement_id.clone(),
                        sequence: signal.sequence,
                        level: signal.level,
                    });
                    self.runtime.handle(HostCommand::CompletePresentation {
                        plan_id,
                        placement_id,
                        value,
                        success: true,
                        message: None,
                    })
                }
                PlatformEffect::TransmitConnection { .. } => {
                    return Err("std host has no in-memory connection driver".to_string());
                }
            };
            pending_effects.extend(follow_up.effects.into_iter().rev());
        }

        let observations = inspect_observations(&mut self.runtime);
        writeln!(output, "plan {} complete", fragment.plan_id.as_str())
            .map_err(|error| error.to_string())?;
        if let (Some(first), Some(last)) = (receipts.first(), receipts.last()) {
            writeln!(
                output,
                "receipts {} first=({}, {}) last=({}, {})",
                receipts.len(),
                first.sequence,
                first.level,
                last.sequence,
                last.level
            )
            .map_err(|error| error.to_string())?;
        } else {
            writeln!(output, "receipts 0").map_err(|error| error.to_string())?;
        }
        Ok(StdRunReport {
            observations,
            receipts,
        })
    }
}

impl CopyFileRequest {
    pub fn new(
        source_path: impl Into<PathBuf>,
        destination_path: impl Into<PathBuf>,
        replace_mode: CopyReplaceMode,
        max_bytes: u64,
        inspect: bool,
    ) -> Self {
        let request_id = fresh_request_id();
        Self {
            source: CopyResourceBinding {
                binding_id: format!("{request_id}:source"),
                path: source_path.into(),
            },
            destination: CopyResourceBinding {
                binding_id: format!("{request_id}:destination"),
                path: destination_path.into(),
            },
            request_id,
            replace_mode,
            max_bytes,
            inspect,
        }
    }
}

pub fn run_copy_file_task(request: CopyFileRequest) -> CopyTaskReport {
    let run_id = fresh_run_id();
    let form_source = copy_file_form_source();
    let source_choice = request.source.path.display().to_string();
    let destination_choice = request.destination.path.display().to_string();
    let preflight = match preflight_copy(&request) {
        Ok(preflight) => preflight,
        Err((preflight, result)) => {
            return CopyTaskReport {
                request_id: request.request_id,
                run_id,
                source_binding_id: request.source.binding_id,
                destination_binding_id: request.destination.binding_id,
                source_choice,
                destination_choice,
                inspect_requested: request.inspect,
                preflight,
                result,
                form_source,
                plan: None,
                observations: Vec::new(),
            };
        }
    };

    let catalog = copy_file_profile_catalog();
    let form = match conduit_form::parse(&form_source, &catalog) {
        Ok(form) => form,
        Err(error) => {
            return CopyTaskReport {
                request_id: request.request_id,
                run_id,
                source_binding_id: request.source.binding_id,
                destination_binding_id: request.destination.binding_id,
                source_choice,
                destination_choice,
                inspect_requested: request.inspect,
                preflight,
                result: CopyTaskResult::Failed {
                    message: error.to_string(),
                },
                form_source,
                plan: None,
                observations: Vec::new(),
            };
        }
    };
    let advertisement = copy_file_advertisement(HostId::from("std-copy-host"), fresh_boot_id());
    let placements = match default_placements(&form, core::slice::from_ref(&advertisement)) {
        Ok(placements) => placements,
        Err(error) => {
            return CopyTaskReport {
                request_id: request.request_id,
                run_id,
                source_binding_id: request.source.binding_id,
                destination_binding_id: request.destination.binding_id,
                source_choice,
                destination_choice,
                inspect_requested: request.inspect,
                preflight,
                result: CopyTaskResult::Failed {
                    message: error.to_string(),
                },
                form_source,
                plan: None,
                observations: Vec::new(),
            };
        }
    };
    let plan = match plan(
        &form,
        core::slice::from_ref(&advertisement),
        &placements,
        &[ConnectionProvider::Local],
    ) {
        Ok(plan) => plan,
        Err(error) => {
            return CopyTaskReport {
                request_id: request.request_id,
                run_id,
                source_binding_id: request.source.binding_id,
                destination_binding_id: request.destination.binding_id,
                source_choice,
                destination_choice,
                inspect_requested: request.inspect,
                preflight,
                result: CopyTaskResult::Failed {
                    message: error.to_string(),
                },
                form_source,
                plan: None,
                observations: Vec::new(),
            };
        }
    };
    let fragment = plan.fragments[0].clone();
    let mut runtime = HostRuntime::new(
        advertisement,
        copy_file_registry(request.clone()).expect("copy implementation installs"),
        128,
    );
    let prepared = runtime.handle(HostCommand::Prepare(fragment.clone()));
    let result = if let Some(reason) = preparation_rejection(&prepared) {
        CopyTaskResult::Failed { message: reason }
    } else {
        let activated = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
        if let Some(reason) = activation_rejection(&activated) {
            CopyTaskResult::Failed { message: reason }
        } else {
            copy_result_from_observations(&inspect_observations(&mut runtime), preflight, &request)
        }
    };
    let observations = inspect_observations(&mut runtime);

    CopyTaskReport {
        request_id: request.request_id,
        run_id,
        source_binding_id: request.source.binding_id,
        destination_binding_id: request.destination.binding_id,
        source_choice,
        destination_choice,
        inspect_requested: request.inspect,
        preflight,
        result,
        form_source,
        plan: Some(plan),
        observations,
    }
}

pub fn render_copy_task_report<W: Write>(
    report: &CopyTaskReport,
    output: &mut W,
) -> Result<(), String> {
    writeln!(
        output,
        "copy-file request={} run={}",
        report.request_id, report.run_id
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "source choice={} binding={}",
        report.source_choice, report.source_binding_id
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "destination choice={} binding={}",
        report.destination_choice, report.destination_binding_id
    )
    .map_err(|error| error.to_string())?;
    writeln!(output, "preflight {}", preflight_label(report.preflight))
        .map_err(|error| error.to_string())?;
    writeln!(output, "primary-action Run/Stop").map_err(|error| error.to_string())?;
    writeln!(output, "result {}", result_label(&report.result))
        .map_err(|error| error.to_string())?;
    if let Some(plan) = &report.plan {
        writeln!(
            output,
            "receipt request={} run={} plan={} source-binding={} destination-binding={} result={}",
            report.request_id,
            report.run_id,
            plan.plan_id.as_str(),
            report.source_binding_id,
            report.destination_binding_id,
            result_label(&report.result)
        )
        .map_err(|error| error.to_string())?;
    }
    if report.plan.is_some() {
        writeln!(output, "inspect available").map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn render_copy_task_inspect<W: Write>(
    report: &CopyTaskReport,
    output: &mut W,
) -> Result<(), String> {
    writeln!(output, "inspect form-source begin").map_err(|error| error.to_string())?;
    write!(output, "{}", report.form_source).map_err(|error| error.to_string())?;
    writeln!(output, "inspect form-source end").map_err(|error| error.to_string())?;
    if let Some(plan) = &report.plan {
        writeln!(
            output,
            "inspect plan {} form {} fragments={}",
            plan.plan_id.as_str(),
            plan.form_id.as_str(),
            plan.fragments.len()
        )
        .map_err(|error| error.to_string())?;
        for fragment in &plan.fragments {
            writeln!(
                output,
                "inspect fragment {} host={} placements={} connections={}",
                fragment.fragment_id.as_str(),
                fragment.host_id.as_str(),
                fragment.placements.len(),
                fragment.connections.len()
            )
            .map_err(|error| error.to_string())?;
            for placement in &fragment.placements {
                writeln!(
                    output,
                    "inspect placement {} kind={} capability={} implementation={}",
                    placement.operation_id.as_str(),
                    placement.kind_id.as_str(),
                    placement.capability_id.as_str(),
                    placement.implementation_id.as_str()
                )
                .map_err(|error| error.to_string())?;
            }
        }
    }
    writeln!(output, "inspect evidence {}", report.observations.len())
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn copy_file_form_source() -> String {
    "form 0\n\ncopy_file_task {\n copy: task/copy-file\n record: task/copy-file-receipt\n copy.receipt -> record.in\n}\n".to_string()
}

fn copy_file_profile_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(COPY_FILE_KIND),
            inputs: Vec::new(),
            outputs: vec![conduit_core::PortDescriptor {
                port_id: conduit_core::PortId::from("receipt"),
                value_kind: kind_id(COPY_FILE_RECEIPT_VALUE_KIND),
                direction: conduit_core::PortDirection::Output,
            }],
            configuration: Vec::new(),
        })
        .expect("copy file kind is unique");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(COPY_FILE_RECEIPT_KIND),
            inputs: vec![conduit_core::PortDescriptor {
                port_id: conduit_core::PortId::from("in"),
                value_kind: kind_id(COPY_FILE_RECEIPT_VALUE_KIND),
                direction: conduit_core::PortDirection::Input,
            }],
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("copy file receipt kind is unique");
    catalog
}

fn copy_file_advertisement(host_id: HostId, boot_id: String) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id: conduit_core::BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("rust-std/copy-task-v1"),
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from(COPY_FILE_CAPABILITY),
                kind_id: kind_id(COPY_FILE_KIND),
                implementation_id: ImplementationId::from(COPY_FILE_IMPLEMENTATION),
                artifact_id: ArtifactId::from(COPY_FILE_ARTIFACT),
                limits: CapabilityLimits {
                    value_kind: kind_id(COPY_FILE_RECEIPT_VALUE_KIND),
                    max_active_instances: 1,
                    max_queue_items: 4,
                    max_queue_bytes: 256,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from(COPY_FILE_RECEIPT_CAPABILITY),
                kind_id: kind_id(COPY_FILE_RECEIPT_KIND),
                implementation_id: ImplementationId::from(COPY_FILE_RECEIPT_IMPLEMENTATION),
                artifact_id: ArtifactId::from(COPY_FILE_RECEIPT_ARTIFACT),
                limits: CapabilityLimits {
                    value_kind: kind_id(COPY_FILE_RECEIPT_VALUE_KIND),
                    max_active_instances: 1,
                    max_queue_items: 4,
                    max_queue_bytes: 256,
                },
            },
        ],
    }
}

fn copy_file_registry(
    request: CopyFileRequest,
) -> Result<ImplementationRegistry, ImplementationFailure> {
    let mut registry = ImplementationRegistry::new();
    registry.install(CopyFileImplementation {
        kind_id: kind_id(COPY_FILE_KIND),
        implementation_id: ImplementationId::from(COPY_FILE_IMPLEMENTATION),
        artifact_id: ArtifactId::from(COPY_FILE_ARTIFACT),
        request,
    })?;
    registry.install(CopyReceiptImplementation)?;
    Ok(registry)
}

struct CopyFileImplementation {
    kind_id: KindId,
    implementation_id: ImplementationId,
    artifact_id: ArtifactId,
    request: CopyFileRequest,
}

impl OperationImplementation for CopyFileImplementation {
    fn kind_id(&self) -> &KindId {
        &self.kind_id
    }

    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    fn prepare(
        &self,
        _placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(CopyFileState {
            request: self.request.clone(),
            completed: false,
        }))
    }

    fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
        (value_kind.as_str() == COPY_FILE_RECEIPT_VALUE_KIND).then_some(1)
    }
}

struct CopyReceiptImplementation;

impl OperationImplementation for CopyReceiptImplementation {
    fn kind_id(&self) -> &KindId {
        static KIND: std::sync::OnceLock<KindId> = std::sync::OnceLock::new();
        KIND.get_or_init(|| kind_id(COPY_FILE_RECEIPT_KIND))
    }

    fn implementation_id(&self) -> &ImplementationId {
        static IMPLEMENTATION: std::sync::OnceLock<ImplementationId> = std::sync::OnceLock::new();
        IMPLEMENTATION.get_or_init(|| ImplementationId::from(COPY_FILE_RECEIPT_IMPLEMENTATION))
    }

    fn artifact_id(&self) -> &ArtifactId {
        static ARTIFACT: std::sync::OnceLock<ArtifactId> = std::sync::OnceLock::new();
        ARTIFACT.get_or_init(|| ArtifactId::from(COPY_FILE_RECEIPT_ARTIFACT))
    }

    fn prepare(
        &self,
        _placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
        Ok(Box::new(CopyReceiptState))
    }

    fn minimum_value_size(&self, value_kind: &KindId) -> Option<u32> {
        (value_kind.as_str() == COPY_FILE_RECEIPT_VALUE_KIND).then_some(1)
    }
}

struct CopyFileState {
    request: CopyFileRequest,
    completed: bool,
}

impl OperationState for CopyFileState {
    fn start(&mut self) -> OperationAction {
        if self.completed {
            return OperationAction::Complete;
        }
        self.completed = true;
        match perform_copy(&self.request) {
            Ok(()) => OperationAction::Emit(conduit_core::ValuePayload {
                value_kind: kind_id(COPY_FILE_RECEIPT_VALUE_KIND),
                encoded: b"copy-complete".to_vec(),
            }),
            Err(result) => OperationAction::Fail(copy_failure(result)),
        }
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Emitted => OperationAction::Complete,
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "copy-file task only accepts emitted completion",
            )),
        }
    }
}

struct CopyReceiptState;

impl OperationState for CopyReceiptState {
    fn start(&mut self) -> OperationAction {
        OperationAction::Idle
    }

    fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
        match completion {
            OperationCompletion::Value(_) | OperationCompletion::InputsClosed => {
                OperationAction::Complete
            }
            _ => OperationAction::Fail(ImplementationFailure::new(
                FailureReason::InvalidLifecycleCommand,
                "copy receipt received invalid completion",
            )),
        }
    }
}

fn preflight_copy(
    request: &CopyFileRequest,
) -> Result<CopyPreflight, (CopyPreflight, CopyTaskResult)> {
    let metadata = match fs::metadata(&request.source.path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err((
                CopyPreflight::WillCreate,
                CopyTaskResult::StaleSource {
                    message: error.to_string(),
                },
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err((
                CopyPreflight::WillCreate,
                CopyTaskResult::Denied {
                    message: error.to_string(),
                },
            ));
        }
        Err(error) => {
            return Err((
                CopyPreflight::WillCreate,
                CopyTaskResult::Failed {
                    message: error.to_string(),
                },
            ));
        }
    };
    if !metadata.is_file() {
        return Err((
            CopyPreflight::WillCreate,
            CopyTaskResult::Denied {
                message: "source binding is not a regular file".to_string(),
            },
        ));
    }
    if metadata.len() > request.max_bytes {
        return Err((
            CopyPreflight::WillCreate,
            CopyTaskResult::OversizedInput {
                size: metadata.len(),
                max_bytes: request.max_bytes,
            },
        ));
    }

    if request.destination.path.exists() {
        match request.replace_mode {
            CopyReplaceMode::ReplaceExisting => Ok(CopyPreflight::WillReplace),
            CopyReplaceMode::CreateOnly | CopyReplaceMode::RejectExisting => Err((
                CopyPreflight::RejectedDestinationExists,
                CopyTaskResult::RejectedDestinationExists,
            )),
        }
    } else {
        Ok(CopyPreflight::WillCreate)
    }
}

fn perform_copy(request: &CopyFileRequest) -> Result<(), CopyTaskResult> {
    if let Some(parent) = request.destination.path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(CopyTaskResult::StaleSource {
                message: "destination parent does not exist".to_string(),
            });
        }
    }
    match fs::copy(&request.source.path, &request.destination.path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(CopyTaskResult::Denied {
                message: error.to_string(),
            })
        }
        Err(error) => {
            if request.destination.path.exists()
                && fs::remove_file(&request.destination.path).is_err()
            {
                Err(CopyTaskResult::CleanupFailed {
                    message: error.to_string(),
                })
            } else {
                Err(CopyTaskResult::Failed {
                    message: error.to_string(),
                })
            }
        }
    }
}

fn copy_result_from_observations(
    observations: &[Observation],
    preflight: CopyPreflight,
    request: &CopyFileRequest,
) -> CopyTaskResult {
    if observations.iter().any(|observation| {
        matches!(
            observation.kind,
            ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Completed
            }
        )
    }) {
        let bytes_copied = fs::metadata(&request.destination.path)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        match preflight {
            CopyPreflight::WillCreate => CopyTaskResult::Created { bytes_copied },
            CopyPreflight::WillReplace => CopyTaskResult::Replaced { bytes_copied },
            CopyPreflight::RejectedDestinationExists => CopyTaskResult::RejectedDestinationExists,
        }
    } else if observations.iter().any(|observation| {
        matches!(
            observation.kind,
            ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Cancelled { .. }
            }
        )
    }) {
        CopyTaskResult::Cancelled
    } else if let Some(message) =
        observations
            .iter()
            .find_map(|observation| match &observation.kind {
                ObservationKind::Failure { message, .. } => message.clone(),
                _ => None,
            })
    {
        CopyTaskResult::Failed { message }
    } else {
        CopyTaskResult::Failed {
            message: "copy task did not produce terminal evidence".to_string(),
        }
    }
}

fn copy_failure(result: CopyTaskResult) -> ImplementationFailure {
    let reason = match result {
        CopyTaskResult::Denied { .. } => FailureReason::ManifestationFailed,
        CopyTaskResult::StaleSource { .. } => FailureReason::StalePlan,
        CopyTaskResult::OversizedInput { .. } => FailureReason::ByteCapacityExceeded,
        CopyTaskResult::Cancelled => FailureReason::InvalidLifecycleCommand,
        CopyTaskResult::CleanupFailed { .. } => FailureReason::ManifestationFailed,
        CopyTaskResult::Failed { .. } => FailureReason::ManifestationFailed,
        CopyTaskResult::Created { .. }
        | CopyTaskResult::Replaced { .. }
        | CopyTaskResult::RejectedDestinationExists => FailureReason::InvalidLifecycleCommand,
    };
    ImplementationFailure::new(reason, result_label(&result))
}

fn preflight_label(preflight: CopyPreflight) -> &'static str {
    match preflight {
        CopyPreflight::WillCreate => "will-create",
        CopyPreflight::WillReplace => "will-replace",
        CopyPreflight::RejectedDestinationExists => "reject-destination-exists",
    }
}

fn result_label(result: &CopyTaskResult) -> String {
    match result {
        CopyTaskResult::Created { bytes_copied } => format!("success-created bytes={bytes_copied}"),
        CopyTaskResult::Replaced { bytes_copied } => {
            format!("success-replaced bytes={bytes_copied}")
        }
        CopyTaskResult::RejectedDestinationExists => "rejected-destination-exists".to_string(),
        CopyTaskResult::Denied { message } => format!("denied message={message}"),
        CopyTaskResult::StaleSource { message } => format!("stale-handle message={message}"),
        CopyTaskResult::OversizedInput { size, max_bytes } => {
            format!("oversized-input size={size} max={max_bytes}")
        }
        CopyTaskResult::Cancelled => "cancelled".to_string(),
        CopyTaskResult::CleanupFailed { message } => format!("cleanup-failed message={message}"),
        CopyTaskResult::Failed { message } => format!("failed message={message}"),
    }
}

pub fn load_checked_form(path: &str) -> Result<CheckedForm, Box<dyn std::error::Error>> {
    Ok(conduit_form::parse(
        &fs::read_to_string(path)?,
        &signal_profile_catalog(),
    )?)
}

pub fn load_placements(
    path: Option<&str>,
) -> Result<Option<PlacementChoices>, Box<dyn std::error::Error>> {
    match path {
        Some(path) => Ok(Some(parse_placements(&fs::read_to_string(path)?)?)),
        None => Ok(None),
    }
}

fn build_advertisement(config: StdHostConfig) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: config.host_id,
        boot_id: config.boot_id,
        offer_generation: config.offer_generation,
        profile: HostProfileId::from("rust-std"),
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from("pulse-1"),
                kind_id: kind_id(PULSE_KIND),
                implementation_id: ImplementationId::from("std/pulse-v1"),
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
                limits: CapabilityLimits {
                    value_kind: kind_id(SIGNAL_VALUE_KIND),
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from("stdout-show-1"),
                kind_id: kind_id(SHOW_KIND),
                implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
                limits: CapabilityLimits {
                    value_kind: kind_id(SIGNAL_VALUE_KIND),
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
        ],
    }
}

fn write_operator_report<W: Write>(
    out: &mut W,
    advertisement: &HostAdvertisement,
    plan_id: &PlanId,
    form_id: &FormId,
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
    writeln!(out, "plan {} form {}", plan_id.as_str(), form_id.as_str())
        .map_err(|error| error.to_string())?;
    for placement in &fragment.placements {
        writeln!(
            out,
            "place {} kind={} host={} boot={} capability={} implementation={} artifact={}",
            placement.operation_id.as_str(),
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
            "connection {} {}:{} -> {}:{} via {:?} queue={}",
            connection.connection_id.as_str(),
            connection.source_placement_id.as_str(),
            connection.source_port_id.as_str(),
            connection.sink_placement_id.as_str(),
            connection.sink_port_id.as_str(),
            connection.provider,
            connection.item_capacity
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn inspect_observations(runtime: &mut HostRuntime) -> Vec<Observation> {
    runtime
        .handle(HostCommand::Inspect)
        .events
        .into_iter()
        .find_map(|event| match event {
            HostEvent::Observations { items } => Some(items),
            _ => None,
        })
        .unwrap_or_default()
}

fn preparation_rejection(output: &RuntimeOutput) -> Option<String> {
    output.events.iter().find_map(|event| match event {
        HostEvent::PreparationRejected {
            reason, message, ..
        } => Some(message.clone().unwrap_or_else(|| format!("{reason:?}"))),
        _ => None,
    })
}

fn activation_rejection(output: &RuntimeOutput) -> Option<String> {
    output.events.iter().find_map(|event| match event {
        HostEvent::ActivationRejected {
            reason, message, ..
        } => Some(message.clone().unwrap_or_else(|| format!("{reason:?}"))),
        _ => None,
    })
}

fn fresh_boot_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = BOOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("boot-{now:x}-{counter:x}")
}

fn fresh_request_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("copy-request-{now:x}-{counter:x}")
}

fn fresh_run_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("copy-run-{now:x}-{counter:x}")
}

#[cfg(test)]
mod tests {
    use super::{
        render_copy_task_inspect, render_copy_task_report, run_copy_file_task, CopyFileRequest,
        CopyReplaceMode, CopyTaskResult, StdHost, StdHostConfig, TimerAdapter,
    };
    use conduit_core::{BootId, HostId, OfferGeneration};
    use conduit_form::parse;
    use conduit_signal::signal_profile_catalog;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;

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
    fn streamed_output_uses_a_virtual_clock_and_retains_terminal_evidence() {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("test-host"),
            boot_id: BootId::from("virtual-clock-boot"),
            offer_generation: OfferGeneration(1),
        });
        let form = parse(
            "form 0\n\nvirtual {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 3\n pulse.period-ms = 7\n pulse.initial = false\n pulse > show\n}\n",
            &signal_profile_catalog(),
        )
        .expect("virtual-clock form parses");
        let plan = host.plan_local(&form, None).expect("local plan resolves");
        let fragment = plan.fragments[0].clone();
        let mut output = Vec::new();
        let mut timer = VirtualTimer::default();
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
        assert!(report.observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Completed
            }
        )));
    }

    #[test]
    fn copy_file_task_runs_through_plan_without_embedding_paths_in_form_source() {
        let dir = temp_task_dir("copy-success");
        fs::create_dir_all(&dir).expect("temp dir exists");
        let source = dir.join("source.txt");
        let destination = dir.join("destination.txt");
        fs::write(&source, "hello conduit\n").expect("source written");

        let report = run_copy_file_task(CopyFileRequest::new(
            &source,
            &destination,
            CopyReplaceMode::RejectExisting,
            1024,
            true,
        ));

        assert_eq!(
            fs::read_to_string(&destination).expect("destination copied"),
            "hello conduit\n"
        );
        assert!(matches!(
            report.result,
            CopyTaskResult::Created { bytes_copied: 14 }
        ));
        assert!(!report.form_source.contains(source.to_str().unwrap()));
        assert!(!report.form_source.contains(destination.to_str().unwrap()));
        assert!(report.plan.is_some());
        assert!(report.observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Completed
            }
        )));

        let mut operator_output = Vec::new();
        render_copy_task_report(&report, &mut operator_output).expect("report renders");
        render_copy_task_inspect(&report, &mut operator_output).expect("inspect renders");
        let operator_output = String::from_utf8(operator_output).expect("utf-8 report");
        assert!(operator_output.contains("primary-action Run/Stop"));
        assert!(operator_output.contains("receipt request="));
        assert!(operator_output.contains("inspect form-source begin"));
        assert!(operator_output.contains("inspect plan "));
    }

    #[test]
    fn copy_file_task_rejects_existing_destination_before_runtime() {
        let dir = temp_task_dir("copy-reject-existing");
        fs::create_dir_all(&dir).expect("temp dir exists");
        let source = dir.join("source.txt");
        let destination = dir.join("destination.txt");
        fs::write(&source, "new").expect("source written");
        fs::write(&destination, "old").expect("destination written");

        let report = run_copy_file_task(CopyFileRequest::new(
            &source,
            &destination,
            CopyReplaceMode::RejectExisting,
            1024,
            false,
        ));

        assert_eq!(report.result, CopyTaskResult::RejectedDestinationExists);
        assert!(report.plan.is_none());
        assert_eq!(
            fs::read_to_string(&destination).expect("destination remains"),
            "old"
        );
    }

    #[test]
    fn copy_file_task_distinguishes_oversized_input() {
        let dir = temp_task_dir("copy-oversized");
        fs::create_dir_all(&dir).expect("temp dir exists");
        let source = dir.join("source.txt");
        let destination = dir.join("destination.txt");
        fs::write(&source, "too large").expect("source written");

        let report = run_copy_file_task(CopyFileRequest::new(
            &source,
            &destination,
            CopyReplaceMode::RejectExisting,
            3,
            false,
        ));

        assert_eq!(
            report.result,
            CopyTaskResult::OversizedInput {
                size: 9,
                max_bytes: 3
            }
        );
        assert!(report.plan.is_none());
        assert!(!destination.exists());
    }

    fn temp_task_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("conduit-std-host-{name}-{}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).expect("old temp removed");
        }
        path
    }
}
