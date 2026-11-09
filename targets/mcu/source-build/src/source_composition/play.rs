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
type Pulse = crate::equipment::Capture;
type Wire = crate::equipment::Wire;
mod shapes {
    include!(concat!(env!("OUT_DIR"), "/source_play_shapes.rs"));
}
use shapes::{
    OutputBound, SamplingBound, FORE_PORTS, OC, OH, ON, OP, OR, OS, OT, SC, SH, SN, SP, SR, SS, ST,
};
pub use shapes::{OutputKernel, SamplingKernel, CONDUIT_EXECUTION_KERNEL_BYTES};
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
    recording: Option<&'a mut Option<ReadyRecordAppender<RecordingStorage>>>,
    recording_issuer: &mut Issuer,
    recording_operation: Option<&Operation<'_>>,
    append_output_bytes: usize,
    pulse_issuer: &mut Issuer,
    write_issuer: &mut Issuer,
    wait_issuers: &mut [Issuer; WAIT_SLOTS],
    operations: &Operations<'_, WAIT_SLOTS>,
    sampling_values: HostedValueStore,
    output_values: HostedValueStore,
    contracts: execution_group::NativeContracts,
    diagnostic: &mut heapless::String<192>,
) -> Result<
    PreparedLocalWorkset<
        'w,
        impl PreparedLocalKernels<
                Bound = (
                    SamplingBound<'a, 'f>,
                    conduit_seven_segment_mask::native_kernel::PreparedAssignedMaskKernel,
                    OutputBound<'a, 'f>,
                ),
            > + 'a,
        3,
    >,
    Refusal<Refusal<&'static str>>,
> {
    let workspace = workspace.into();
    let append_selected = source::sampling::NATIVE_KINDS
        .iter()
        .any(|(_, kind)| *kind == conduit_resource_snapshot::native_append::APPEND_KIND);
    if append_selected
        && append_output_bytes
            < conduit_resource_snapshot::native_append::APPEND_RESULT_BYTES as usize
    {
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
            profile!(sampling_layout, host, boot, &sf),
            profile!(mask_layout, host, boot, &mf),
            profile!(output_layout, host, boot, &of),
        ],
        #[inline(always)]
        move |[sampling, mask, output], [sampling_fore, mask_fore, output_fore]| {
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
            let sampling_profile = profile!(sampling_layout, host, boot, sampling_fore);
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
                recording_issuer,
                recording_operation,
                pulse_issuer,
                write_issuer,
                wait_issuers,
                operations,
                sampling,
                output,
                profile!(sampling_layout, host, boot, sampling_fore),
                profile!(output_layout, host, boot, output_fore),
                sampling_fore,
                output_fore,
                contracts,
            )?;
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
            let sample = PreparedFragment::<_, SN, SC, SP, SR, ST, SH, SS, HostedSignLog>::prepare(
                sampling,
                sampling_profile,
                sampling_fore,
                &ResidentLayout {
                    records: source::sampling_layout::RECORDS,
                    counts: source::sampling_layout::COUNTS,
                },
                sampling_values,
                HostedSignLog::new(
                    SS as u16,
                    (SS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
                )
                .map_err(|_| "sampling-sign-storage")?,
                policy.maximum_retained_bytes,
                || Ok::<_, &'static str>(sample_backs),
            )
            .map_err(|error| kernel_refusal(diagnostic, error, "sampling-kernel"))?;
            let output = PreparedFragment::<_, ON, OC, OP, OR, OT, OH, OS, HostedSignLog>::prepare(
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
