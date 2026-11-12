use crate::{
    equipment::{MonotonicTimer, WAIT_SLOTS},
    resident::generated as resident,
    workset::{generated as source, renderer as display},
};
use alloc::boxed::Box;
use conduit_body as b;
use conduit_core as c;
use conduit_kernel::{assigned::ResidentLayout, HostedSignLog, HostedValueStore, NodeId};
use conduit_mcu_host_runtime::{
    execution::{ExecutionKernel, Policy, PreparedFragment, Refusal},
    execution_authority::{Issuer, NativeFacts, Operation},
    execution_factory::{
        Driver, PreparedCalls, PreparedDrivers, PreparedResidentSubset, SoftwareInventory,
    },
    execution_group::{self, EntryPin, FragmentFactory, MaskConstruction},
    execution_mask::ReadyMaskRouting,
    execution_workset::{PreparedKernelGroup, PreparedLocalKernels, PreparedLocalWorkset},
    native_authority::Operations,
};
use conduit_monotonic_wait::ReadyMonotonicWaitBase;
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_expression::SelectedAssignedExpressionPlan,
    selected_assigned_operation::{
        AssignedOperationInput, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
};
use conduit_pulse_capture::installation::ReadyPulseBase;
use conduit_resource_snapshot::native_append::ReadyRecordAppender;
use conduit_tm1637_wire::installation::ReadyWriteBase;
type RecordingStorage = crate::equipment::RecordingStorage;
mod recording_policy {
    include!(concat!(env!("OUT_DIR"), "/source_play_recording.rs"));
}
pub use recording_policy::{
    RecordingIssuers, RecordingOperation, OUTPUT_BYTES as RECORDING_OUTPUT_BYTES,
};

type Pulse = crate::equipment::Capture;
type Wire = crate::equipment::Wire;
mod shapes {
    include!(concat!(env!("OUT_DIR"), "/source_play_shapes.rs"));
}
pub(crate) use shapes::FORE_PORTS;
use shapes::{
    OutputBound, SamplingBound, OC, OH, OK, ON, OP, OR, OS, OT, SC, SH, SK, SN, SP, SR, SS, ST,
};
pub use shapes::{OutputKernel, SamplingCalls, SamplingKernel, CONDUIT_EXECUTION_KERNEL_BYTES};
pub mod fore {
    include!(concat!(env!("OUT_DIR"), "/source_play_fore.rs"));
}
// Specialize the generic lowering width at compile time. The physical kernel
// borrows this immutable matrix; no projection array is constructed on its stack.
const fn terminal_contracts<const N: usize, const P: usize, const WIDE: usize>(
    source: &[[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; WIDE]; N],
) -> [[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; P]; N] {
    assert!(P <= WIDE);
    let mut result = [[None; P]; N];
    let mut node = 0;
    while node < N {
        let mut port = 0;
        while port < WIDE {
            if port < P {
                result[node][port] = source[node][port];
            } else {
                assert!(source[node][port].is_none());
            }
            port += 1;
        }
        node += 1;
    }
    result
}
const SAMPLING_TERMINALS: [[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; SP];
    SN] = terminal_contracts(&source::sampling_layout::GENERATED_TERMINAL_TRANSDUCTIONS);
const OUTPUT_TERMINALS: [[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>; OP];
    ON] = terminal_contracts(&source::output_layout::GENERATED_TERMINAL_TRANSDUCTIONS);

pub const CONTINUATION_FORE_PORTS: usize = FORE_PORTS;
pub type ContinuationStarted<'a, 'f> =
    conduit_mcu_host_runtime::execution_workset::StartedLocalWorkset<(
        (
            fore::BoundSampling<'a, 'a, SamplingBound<'a, 'f>>,
            conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel,
            OutputBound<'a, 'f>,
        ),
        conduit_mcu_host_runtime::execution_mask::RendererService<'a, crate::renderer::Clock>,
    )>;
macro_rules! profile {
    ($module:ident,$host:expr,$boot:expr,$fore:expr) => {{
        use crate::workset::generated::$module as f;
        b::AssignedBodyPreparationProfile {
            maxima: f::MAXIMA,
            requirements: c::AssignedPlanRequirements {
                host: $host,
                boot: $boot,
                host_calls: f::HOST_CALL_CONTRACTS,
                resources: f::RESOURCE_ORDINALS,
                remote_bindings: &[],
            },
            computations: c::AssignedComputationProfile {
                maximum_text_bytes: f::COMPUTATION_MAXIMUM_TEXT_BYTES,
                maximum_total_text_bytes: f::COMPUTATION_MAXIMUM_TOTAL_TEXT_BYTES,
                requirements: f::COMPUTATIONS,
            },
            fore: Some($fore),
        }
    }};
}
macro_rules! pin {
    ($m:ident) => {
        EntryPin {
            source: source::$m::SOURCE_DOCUMENT_ID,
            checked: source::$m::CHECKED_PLOT_ID,
            expanded: source::$m::EXPANDED_PLOT_ID,
        }
    };
}
fn operation<'a>(
    approved: &'a b::DelegatedBodyFragment<'a>,
    profile: &b::AssignedBodyPreparationProfile<'a>,
    node: u16,
) -> SelectedAssignedOperationPlan<'a> {
    SelectedAssignedOperationPlan {
        input: AssignedOperationInput::Contiguous(approved.projection()),
        maxima: profile.maxima,
        requirements: profile.requirements,
        computations: profile.computations,
        fragment_id: approved.metadata().fragment,
        scope: AssignedPlayScope::Delegated(approved),
        node: NodeId(node),
    }
}
mod authority {
    include!(concat!(env!("OUT_DIR"), "/source_play_authority.rs"));
}
pub use authority::NativeAuthority;
pub mod continuation {
    include!(concat!(env!("OUT_DIR"), "/source_play_continuation.rs"));
}
mod owners {
    include!(concat!(env!("OUT_DIR"), "/source_play_owners.rs"));
}
/// Only original opaque approvals enter the grouped callback. Library selection
/// checks all four real resident owners before splitting disjoint affine loans.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[inline(never)] // Keep preparation temporaries out of the continuing async poll frame.
pub fn prepare<'a, 'f: 'a, 'w: 'scope, 'context, 'scope>(
    workspace: impl Into<Option<&'w b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>>>,
    context: NativeAuthority<'context, 'scope, 'w>,
    policy: Policy<'_>,
    residents: &'a mut resident::Initialized,
    software: &'a mut SoftwareInventory<'f, SN>,
    output_software: &'a mut SoftwareInventory<'f, ON>,
    routing: &'a mut ReadyMaskRouting,
    pulse: &'a mut Option<ReadyPulseBase<Pulse>>,
    write: &'a mut Option<ReadyWriteBase<Wire>>,
    waits: &'a mut [Option<ReadyMonotonicWaitBase<MonotonicTimer>>; WAIT_SLOTS],
    recording: Option<&'a mut Option<crate::equipment::RecordingReady>>,
    #[cfg(record_service)] recording_resource: Option<&'a c::ResourceBinding>,
    recording_issuer: RecordingIssuers<'_>,
    recording_operation: Option<&RecordingOperation<'_>>,
    append_output_bytes: usize,
    pulse_issuer: &mut Issuer,
    write_issuer: &mut Issuer,
    wait_issuers: &mut [Issuer; WAIT_SLOTS],
    operations: &Operations<'_, WAIT_SLOTS>,
    sampling_values: HostedValueStore,
    output_values: HostedValueStore,
    contracts: &mut Option<execution_group::NativeContracts>,
    diagnostic: &mut heapless::String<192>,
    line: Option<fore::Selection<'a>>,
    prepare_carrier: impl FnOnce(
        &b::DelegatedBodyFragment<'w>,
        Option<&b::ApprovedBodyForePorts<FORE_PORTS>>,
        b::AssignedBodyPreparationProfile<'_>,
    ) -> Result<Option<fore::Components<'a, 'a>>, &'static str>,
) -> Result<
    PreparedLocalWorkset<
        'w,
        impl PreparedLocalKernels<
                Bound = (
                    fore::BoundSampling<'a, 'a, SamplingBound<'a, 'f>>,
                    conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel,
                    OutputBound<'a, 'f>,
                ),
            > + 'a,
        3,
    >,
    Refusal<Refusal<&'static str>>,
> {
    // Keep cold schema graphs in their original backing through selection.
    // The guard retires them on every refusal as well as on success.
    struct ColdContracts<'a>(&'a mut Option<execution_group::NativeContracts>);
    impl ColdContracts<'_> {
        fn get(&self) -> Option<&execution_group::NativeContracts> {
            self.0.as_ref()
        }
        fn retire(&mut self) {
            *self.0 = None;
        }
    }
    impl Drop for ColdContracts<'_> {
        fn drop(&mut self) {
            *self.0 = None;
        }
    }
    let mut contracts = ColdContracts(contracts);
    let workspace = workspace.into();
    let append_selected = source::sampling::NATIVE_KINDS
        .iter()
        .any(|(_, kind)| *kind == conduit_resource_snapshot::native_append::APPEND_KIND);
    if append_selected && append_output_bytes < RECORDING_OUTPUT_BYTES {
        return Err(Refusal::Bounds);
    }
    // The original arena funds cold preparation and retained execution.
    // Check physical kernel headroom after retiring the cold contracts.
    let mask_values = HostedValueStore::canonical_storage_bytes([
        display::RENDERER_FACE_BYTES,
        display::RENDERER_SHOW_BYTES,
    ])
    .map_err(|_| Refusal::Bounds)?;
    let minimum = CONDUIT_EXECUTION_KERNEL_BYTES
        .iter()
        .copied()
        .chain([
            mask_values,
            sampling_values.reserved_storage_bytes(),
            output_values.reserved_storage_bytes(),
        ])
        .try_fold(0usize, usize::checked_add)
        .ok_or(Refusal::Bounds)?;
    // The two existing stores are already included in Root's allocator baseline.
    let incremental = minimum
        .checked_sub(sampling_values.reserved_storage_bytes())
        .and_then(|n| n.checked_sub(output_values.reserved_storage_bytes()))
        .ok_or(Refusal::Bounds)?;
    if minimum > policy.maximum_retained_bytes {
        return Err(Refusal::LowerBoundCapacity {
            required: minimum,
            maximum: policy.maximum_retained_bytes,
        });
    }
    let host = c::AssignedIdentity::from_text(context.actual().host_id.as_str());
    let boot = c::AssignedIdentity::from_text(context.actual().boot_id.as_str());
    let sf = source::sampling_layout::fore();
    let mf = source::mask_layout::fore();
    let of = source::output_layout::fore();
    use conduit_mcu_host_runtime::execution_workset::AssignedPreparationProfile;
    let mut sampling_profile = profile!(sampling_layout, host, boot, &sf);
    let selected_profile = match line {
        Some(line) => {
            sampling_profile.fore = None;
            sampling_profile.requirements.remote_bindings = line.bindings;
            AssignedPreparationProfile::LineFore(b::AssignedBodyLineForePreparationProfile {
                preparation: sampling_profile,
                line_fore: line.profile,
            })
        }
        None => AssignedPreparationProfile::Legacy(sampling_profile),
    };
    context.prepare::<_, _, { crate::config::MAX_ASSIGNMENT_BYTES }, FORE_PORTS>(
        workspace,
        policy,
        [
            pin!(sampling_layout),
            pin!(mask_layout),
            pin!(output_layout),
        ],
        [
            source::sampling_layout::CONFIGURATIONS,
            source::mask_layout::CONFIGURATIONS,
            source::output_layout::CONFIGURATIONS,
        ],
        [
            selected_profile,
            AssignedPreparationProfile::Legacy(profile!(mask_layout, host, boot, &mf)),
            AssignedPreparationProfile::Legacy(profile!(output_layout, host, boot, &of)),
        ],
        #[inline(always)]
        move |[sampling, mask, output], [sampling_fore, mask_fore, output_fore], approved_fore| {
            // Reconstruct only the finite bounds and immutable borrowed pins;
            // each preparation owner consumes its descriptor independently.
            let sampling_profile=|| {
                let mut profile=profile!(sampling_layout,host,boot,sampling_fore);
                if let Some(line)=line { profile.fore=None; profile.requirements.remote_bindings=line.bindings; }
                profile
            };
            let carriage=prepare_carrier(sampling,approved_fore.map(|ports|ports[0]),sampling_profile())?;
            // Bind the already funded recording owner only from the original
            // opaque approval. The stream identifies this authored producer;
            // Boot/Plan/Play remain separate, exact record context fields.
            use sha2::{Digest, Sha256};
            let mut stream = Sha256::new();
            stream.update(b"conduit.mcu/authored-recording-stream@1\0");
            for field in [
                sampling.metadata().body,
                context.actual().host_id.as_str(),
                source::sampling_layout::SOURCE_DOCUMENT_ID,
                source::sampling_layout::CHECKED_PLOT_ID,
            ] {
                stream.update((field.len() as u64).to_le_bytes());
                stream.update(field.as_bytes());
            }
            software.prepare_recording_from_approved(
                sampling,
                context.actual(),
                stream.finalize().into(),
                1,
            )?;
            let output_profile = profile!(output_layout, host, boot, output_fore);
            let (sample_backs, output_backs) = owners::prepare(
                context,
                policy,
                residents,
                software,
                output_software,
                pulse,
                write,
                waits,
                recording,
                #[cfg(record_service)] recording_resource,
                recording_issuer,
                recording_operation,
                pulse_issuer,
                write_issuer,
                wait_issuers,
                operations,
                sampling,
                output,
                sampling_profile(),
                profile!(output_layout, host, boot, output_fore),
                line.map(|l|l.profile),
                sampling_fore,
                output_fore,
                contracts.get().ok_or("native-contracts-busy")?,
            )?;
            // No native ticket retains schema references. End the immutable
            // loan before dropping the original graphs in place, so their heap
            // backing is available for physical kernel construction.
            contracts.retire();
            let available = crate::runtime::remaining();
            if incremental > available {
                return Err(kernel_refusal(
                    diagnostic,
                    Refusal::<()>::LowerBoundCapacity {
                        required: incremental,
                        maximum: available,
                    },
                    "kernel-headroom",
                ));
            }
            let sample = fore::prepare_kernel(sampling,sampling_profile(),sampling_fore,line.map(|l|l.profile),sampling_values,policy.maximum_retained_bytes,sample_backs)
                .map_err(|error| kernel_refusal(diagnostic,error,"sampling-kernel"))?;
            let sample=match carriage {Some(carriage)=>carriage.wrap(sample),None=>fore::PreparedSampling::Local(sample,core::marker::PhantomData)};
            let output = PreparedFragment::<_, ON, OC, OP, OR, OT, OH, OS, HostedSignLog, OK>::prepare_with_terminal_transductions_ref(
                output,
                output_profile,
                output_fore,
                &ResidentLayout {
                    records: source::output_layout::RECORDS,
                    counts: source::output_layout::COUNTS,
                },
                output_values,
                HostedSignLog::new_with_remote_storage(
                    OS as u16,
                    (OS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
                    OS as u16,
                    conduit_kernel::remote_sign_storage_bytes(OS as u16)
                        .ok_or("output-sign-bound")?,
                )
                .map_err(|_| "output-sign-storage")?,
                policy.maximum_retained_bytes,
                || Ok::<_, &'static str>(output_backs),
                &OUTPUT_TERMINALS,
            )
            .map_err(|error| kernel_refusal(diagnostic, error, "output-kernel"))?;
            let mask = MaskConstruction {
                entry: pin!(mask_layout),
                configuration: source::mask_layout::CONFIGURATIONS,
                profile: profile!(mask_layout, host, boot, mask_fore),
                layout: conduit_seven_segment_mask::native_kernel::AssignedMaskProfile {
                    configurations: source::mask_layout::CONFIGURATIONS,
                    layout: ResidentLayout {
                        records: source::mask_layout::RECORDS,
                        counts: source::mask_layout::COUNTS,
                    },
                    fore: mask_fore,
                    roles: source::mask_layout::MASK_ROLES.ok_or("mask-roles")?,
                    present_call: c::AssignedIdentity::from_text(
                        conduit_mcu_host_runtime::execution_mask::CALL,
                    ),
                    resources: source::mask_layout::RESOURCE_ORDINALS,
                    face_bytes: display::RENDERER_FACE_BYTES,
                    show_bytes: display::RENDERER_SHOW_BYTES,
                },
                owner: routing,
            }
            .prepare(mask, mask_fore)
            .map_err(|error| kernel_refusal(diagnostic, error, "mask-kernel"))?;
            Ok(PreparedKernelGroup(sample, mask, output))
        },
    )
}

fn kernel_refusal(
    detail: &mut heapless::String<192>,
    error: impl core::fmt::Debug,
    label: &'static str,
) -> &'static str {
    use core::fmt::Write;
    let _ = write!(detail, "{error:?}");
    label
}
