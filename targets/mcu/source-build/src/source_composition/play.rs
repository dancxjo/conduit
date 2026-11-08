// Composition of actual Source-selected loans, under the shared atomic Root.
use crate::{
    equipment::{MonotonicTimer, WAIT_SLOTS},
    resident::generated as resident,
    workset::{generated as source, renderer as display},
};
use alloc::boxed::Box;
use conduit_body as b;
use conduit_core as c;
use conduit_kernel::{assigned::ResidentLayout, FixedSignLog, HostedValueStore, NodeId};
use conduit_mcu_host_runtime::{
    execution::{ExecutionKernel, Policy, PreparedFragment, Refusal},
    execution_authority::{Issuer, NativeFacts},
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
use conduit_tm1637_wire::installation::ReadyWriteBase;
type Pulse = crate::equipment::Capture;
type Wire = crate::equipment::Wire;
// Retain only the independently compiled endpoint capacity, not the generic
// carriage ceiling in every fragment's temporary preparation wrapper.
const FORE_PORTS: usize = {
    let a = source::sampling_layout::FORE_PORTS;
    let b = source::mask_layout::FORE_PORTS;
    let c = source::output_layout::FORE_PORTS;
    let ab = if a > b { a } else { b };
    if ab > c { ab } else { c }
};
macro_rules! shape {
    ($name:ident,$module:ident,$tag:ident) => {
        const $name: usize = source::$module::COUNTS[(c::$tag - 1) as usize] as usize;
    };
}
shape!(SN, sampling_layout, ASSIGNED_NODE);
shape!(SC, sampling_layout, ASSIGNED_CORD);
shape!(SR, sampling_layout, ASSIGNED_ROUTE);
shape!(ST, sampling_layout, ASSIGNED_ROUTE_TARGET);
shape!(SH, sampling_layout, ASSIGNED_HOST_CALL);
shape!(ON, output_layout, ASSIGNED_NODE);
shape!(OC, output_layout, ASSIGNED_CORD);
shape!(OR, output_layout, ASSIGNED_ROUTE);
shape!(OT, output_layout, ASSIGNED_ROUTE_TARGET);
shape!(OH, output_layout, ASSIGNED_HOST_CALL);
const SP: usize = source::sampling_layout::PORTS_PER_NODE;
const OP: usize = source::output_layout::PORTS_PER_NODE;
const SS: usize = source::sampling_layout::SIGN_ITEMS;
const OS: usize = source::output_layout::SIGN_ITEMS;
pub type SamplingKernel<'a, 'f> =
    ExecutionKernel<Driver<'f, resident::BorrowedBack<'a>>, SN, SC, SP, SR, ST, SH, SS>;
pub type OutputKernel<'a, 'f> =
    ExecutionKernel<Driver<'f, resident::BorrowedBack<'a>>, ON, OC, OP, OR, OT, OH, OS>;
/// Actual target type sizes, retained in the ELF for qualification. These are
/// only kernel allocations: providers, canonical buffers and temporary
/// preparation require separate accounting.
#[used]
#[no_mangle]
pub static CONDUIT_EXECUTION_KERNEL_BYTES: [usize; 3] = [
    core::mem::size_of::<SamplingKernel<'static, 'static>>(),
    core::mem::size_of::<conduit_seven_segment_mask::native_kernel::Kernel>(),
    core::mem::size_of::<OutputKernel<'static, 'static>>(),
];
type SamplingBound<'a, 'f> = (
    Box<SamplingKernel<'a, 'f>>,
    conduit_mcu_host_runtime::execution_factory::Calls<Pulse, Wire, MonotonicTimer, WAIT_SLOTS>,
);
type OutputBound<'a, 'f> = (
    Box<OutputKernel<'a, 'f>>,
    conduit_mcu_host_runtime::execution_factory::Calls<Pulse, Wire, MonotonicTimer, 0>,
);
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
/// Only original opaque approvals enter the grouped callback. Library selection
/// checks all four real resident owners before splitting disjoint affine loans.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[inline(never)] // Keep preparation temporaries out of the continuing async poll frame.
pub fn prepare<'a, 'f: 'a, 'w>(
    workspace: &'w b::HostCommandInput<16384>,
    context: b::NativePreparationContext<'_>,
    policy: Policy<'_>,
    residents: &'a mut resident::Initialized,
    software: &'a mut SoftwareInventory<'f, SN>,
    output_software: &'a mut SoftwareInventory<'f, ON>,
    routing: &'a mut ReadyMaskRouting,
    pulse: &'a mut Option<ReadyPulseBase<Pulse>>,
    write: &'a mut Option<ReadyWriteBase<Wire>>,
    waits: &'a mut [Option<ReadyMonotonicWaitBase<MonotonicTimer>>; WAIT_SLOTS],
    pulse_issuer: &mut Issuer,
    write_issuer: &mut Issuer,
    wait_issuers: &mut [Issuer; WAIT_SLOTS],
    operations: &Operations<'_, WAIT_SLOTS>,
    sampling_values: HostedValueStore,
    output_values: HostedValueStore,
    maximum_incremental_bytes: usize,
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
    // Refuse before borrowing any provider or allocating native codecs/kernel
    // storage. This is deliberately a lower bound, not an aggregate peak or a
    // resource grant: the caller must independently fund temporary preparation
    // and the additional Ready/native/Face backing counted by the group.
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
    // These two stores are already included in Root's actual allocator
    // baseline. Only the incremental kernels and Mask slots consume the held
    // workspace; all additional native/temporary allocations must also fit it.
    let incremental = minimum
        .checked_sub(sampling_values.reserved_storage_bytes())
        .and_then(|n| n.checked_sub(output_values.reserved_storage_bytes()))
        .ok_or(Refusal::Bounds)?;
    if incremental > maximum_incremental_bytes {
        return Err(Refusal::LowerBoundCapacity {
            required: incremental,
            maximum: maximum_incremental_bytes,
        });
    }
    if minimum > policy.maximum_retained_bytes {
        return Err(Refusal::LowerBoundCapacity {
            required: minimum,
            maximum: policy.maximum_retained_bytes,
        });
    }
    let host = c::AssignedIdentity::from_text(context.actual.host_id.as_str());
    let boot = c::AssignedIdentity::from_text(context.actual.boot_id.as_str());
    let sf = source::sampling_layout::fore();
    let mf = source::mask_layout::fore();
    let of = source::output_layout::fore();
    execution_group::prepare_with_source::<_, _, 16384, FORE_PORTS>(
        workspace,
        context,
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
            let sampling_profile = profile!(sampling_layout, host, boot, sampling_fore);
            let output_profile = profile!(output_layout, host, boot, output_fore);
            let mut is_sampling = [false; resident::ENTRY_COUNT];
            let mut selected = core::array::from_fn(|_| None);
            for entry in 0..resident::ENTRY_COUNT {
                let identity = residents.available_identity(entry).ok_or("resident-busy")?;
                let same = |pin: &&c::AssignedComputationRequirement| {
                    pin.key == identity.computation.key
                        && pin.text_bytes == identity.computation.text_bytes
                        && pin.text_digest == identity.computation.text_digest
                };
                let sample = source::sampling_layout::COMPUTATIONS.iter().find(same);
                let out = source::output_layout::COMPUTATIONS.iter().find(same);
                let (approved, profile, fore, pin) = match (sample, out) {
                    (Some(pin), None) => {
                        is_sampling[entry] = true;
                        (sampling, &sampling_profile, sampling_fore, pin)
                    }
                    (None, Some(pin)) => (output, &output_profile, output_fore, pin),
                    _ => return Err("resident-source-selection"),
                };
                selected[entry] = Some(SelectedAssignedExpressionPlan {
                    input: AssignedOperationInput::Contiguous(approved.projection()),
                    maxima: profile.maxima,
                    requirements: profile.requirements,
                    computations: profile.computations,
                    fore: Some(fore),
                    fragment_id: approved.metadata().fragment,
                    scope: AssignedPlayScope::Delegated(approved),
                    node: NodeId(pin.node),
                    work_resource: 0, // Checked BSP publishes its cooperative pool first.
                });
            }
            let mut loans = residents
                .prepare_selected_array(selected)
                .map_err(|_| "resident-assignment")?
                .split();
            let sample_loans =
                PreparedResidentSubset::<_, { resident::ENTRY_COUNT }>(core::array::from_fn(|i| {
                    if is_sampling[i] {
                        loans[i].take()
                    } else {
                        None
                    }
                }));
            let output_loans =
                PreparedResidentSubset::<_, { resident::ENTRY_COUNT }>(core::array::from_fn(|i| {
                    loans[i].take()
                }));
            use conduit_mcu_host_runtime::execution_factory::PreparedResidents;
            let sample_nodes = sample_loans.nodes();
            let output_nodes = output_loans.nodes();
            let node = |kinds: &[(u16, &str)], kind: &str| {
                kinds
                    .iter()
                    .find(|n| n.1 == kind)
                    .map(|n| n.0)
                    .ok_or("native-node")
            };
            let pn = node(
                source::sampling::NATIVE_KINDS,
                conduit_pulse_capture::PULSE_READ_KIND,
            )?;
            let wn = node(
                source::output::NATIVE_KINDS,
                conduit_tm1637_wire::WRITE_KIND,
            )?;
            let (table, handle, claim) = pulse_issuer
                .issue(
                    context,
                    policy,
                    sampling,
                    &NativeFacts::pulse(pulse.as_ref().ok_or("pulse-busy")?),
                    &operations.pulse,
                    crate::runtime::remaining(),
                )
                .map_err(|_| "pulse-authority")?;
            let pulse_contract = conduit_pulse_capture::PulseReadContract::prepare()
                .map_err(|_| "pulse-contract")?;
            let pulse = ReadyPulseBase::prepare_assigned_fore_retained(
                pulse,
                table,
                handle,
                claim,
                &pulse_contract,
                SelectedAssignedForeOperationPlan {
                    operation: operation(sampling, &sampling_profile, pn),
                    fore: sampling_fore,
                },
            )
            .map_err(|_| "pulse-assignment")?;
            drop(pulse_contract); // Cold schema metadata is not execution custody.
            let (table, handle, claim) = write_issuer
                .issue(
                    context,
                    policy,
                    output,
                    &NativeFacts::write(write.as_ref().ok_or("write-busy")?),
                    &operations.write,
                    crate::runtime::remaining(),
                )
                .map_err(|_| "write-authority")?;
            let write_contract =
                conduit_tm1637_wire::WriteContract::prepare().map_err(|_| "write-contract")?;
            let write = ReadyWriteBase::prepare_assigned_fore_retained(
                write,
                table,
                handle,
                claim,
                &write_contract,
                SelectedAssignedForeOperationPlan {
                    operation: operation(output, &output_profile, wn),
                    fore: output_fore,
                },
            )
            .map_err(|_| "write-assignment")?;
            drop(write_contract);
            // This checked BSP construction currently admits exactly one Source
            // clock; shared mechanisms support larger independently built sets.
            let [wait] = waits;
            let [issuer] = wait_issuers;
            let [Some(op)] = &operations.waits else {
                return Err("wait-operation");
            };
            let [(clock, period, count)] = source::sampling::CLOCKS else {
                return Err("source-clock-count");
            };
            let configuration = [
                // The assigned ABI retains canonical Source field order.
                c::ConfigurationEntry {
                    key: "count".into(),
                    value: c::ConfigurationValue::U64(*count),
                },
                c::ConfigurationEntry {
                    key: "period-ms".into(),
                    value: c::ConfigurationValue::U64(*period),
                },
            ];
            let (table, handle, claim) = issuer
                .issue(
                    context,
                    policy,
                    sampling,
                    &NativeFacts::wait(wait.as_ref().ok_or("wait-busy")?),
                    op,
                    crate::runtime::remaining(),
                )
                .map_err(|_| "wait-authority")?;
            let wait = ReadyMonotonicWaitBase::prepare_assigned_fore_retained(
                wait,
                table,
                handle,
                claim,
                &configuration,
                SelectedAssignedForeOperationPlan {
                    operation: operation(sampling, &sampling_profile, *clock),
                    fore: sampling_fore,
                },
            )
            .map_err(|_| "wait-assignment")?;
            drop(configuration);
            let sample_software = software
                .prepare_assigned(
                    sampling.projection(),
                    core::array::from_fn(|i| {
                        !sample_nodes.iter().flatten().any(|n| usize::from(*n) == i)
                            && i != usize::from(pn)
                    }),
                )
                .map_err(|_| "sampling-software")?;
            let out_software = output_software
                .prepare_assigned(
                    output.projection(),
                    core::array::from_fn(|i| {
                        !output_nodes.iter().flatten().any(|n| usize::from(*n) == i)
                            && i != usize::from(wn)
                    }),
                )
                .map_err(|_| "output-software")?;
            let sample_backs = PreparedDrivers::new(
                sample_loans,
                sample_nodes,
                sample_software,
                PreparedCalls {
                    pulse: Some(pulse),
                    write: None,
                    waits: [wait],
                },
            )?;
            let output_backs = PreparedDrivers::<_, Pulse, Wire, MonotonicTimer, 4, ON, 0>::new(
                output_loans,
                output_nodes,
                out_software,
                PreparedCalls {
                    pulse: None,
                    write: Some(write),
                    waits: [],
                },
            )?;
            let sample = PreparedFragment::<_, SN, SC, SP, SR, ST, SH, SS>::prepare(
                sampling,
                sampling_profile,
                sampling_fore,
                &ResidentLayout {
                    records: source::sampling_layout::RECORDS,
                    counts: source::sampling_layout::COUNTS,
                },
                sampling_values,
                FixedSignLog::new(
                    (SS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
                )
                .map_err(|_| "sampling-sign-storage")?,
                policy.maximum_retained_bytes,
                || Ok::<_, &'static str>(sample_backs),
            )
            .map_err(|_| "sampling-kernel")?;
            let output = PreparedFragment::<_, ON, OC, OP, OR, OT, OH, OS>::prepare(
                output,
                output_profile,
                output_fore,
                &ResidentLayout {
                    records: source::output_layout::RECORDS,
                    counts: source::output_layout::COUNTS,
                },
                output_values,
                FixedSignLog::new(
                    (OS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
                )
                .map_err(|_| "output-sign-storage")?,
                policy.maximum_retained_bytes,
                || Ok::<_, &'static str>(output_backs),
            )
            .map_err(|_| "output-kernel")?;
            let mask = MaskConstruction {
                entry: pin!(mask_layout),
                configuration: source::mask_layout::CONFIGURATIONS,
                profile: profile!(mask_layout, host, boot, mask_fore),
                layout: conduit_seven_segment_mask::native_kernel::AssignedMaskProfile {
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
            .map_err(|_| "mask-kernel")?;
            Ok(PreparedKernelGroup(sample, mask, output))
        },
    )
}
