use super::*;
type Residents<'a> =
    PreparedResidentSubset<resident::PreparedResidentEntryLoan<'a>, { resident::ENTRY_COUNT }>;
type SampleBase<'a, 'f> = PreparedDrivers<
    'a,
    'a,
    'f,
    Residents<'a>,
    Pulse,
    Wire,
    MonotonicTimer,
    { resident::ENTRY_COUNT },
    SN,
    WAIT_SLOTS,
>;
#[cfg(not(record_service))]
type Sample<'a, 'f> = conduit_mcu_host_runtime::execution_factory::PreparedWithOptionalAppender<
    'a,
    SampleBase<'a, 'f>,
    RecordingStorage,
>;
#[cfg(record_service)]
type Sample<'a, 'f> = conduit_mcu_host_runtime::execution_factory::PreparedWithRecordService<
    'a, SampleBase<'a, 'f>, RecordingStorage, SP,
    { conduit_resource_snapshot::read_result::READ_RESULT_BYTES },
>;
type Output<'a, 'f> = PreparedDrivers<
    'a,
    'a,
    'f,
    Residents<'a>,
    Pulse,
    Wire,
    MonotonicTimer,
    { resident::ENTRY_COUNT },
    ON,
    0,
>;

/// Cold owner selection ends before kernel construction needs its own scratch.
/// Original opaque approvals and exclusive native/resident loans cross this
/// boundary; it performs no resource admission, provider bind or Start.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
#[inline(never)]
pub(super) fn prepare<'a, 'f: 'a>(
    context: NativeAuthority<'_, '_, '_>,
    policy: Policy<'_>,
    residents: &'a mut resident::Initialized,
    software: &'a mut SoftwareInventory<'f, SN>,
    output_software: &'a mut SoftwareInventory<'f, ON>,
    pulse: &'a mut Option<ReadyPulseBase<Pulse>>,
    write: &'a mut Option<ReadyWriteBase<Wire>>,
    waits: &'a mut [Option<ReadyMonotonicWaitBase<MonotonicTimer>>; WAIT_SLOTS],
    recording: Option<&'a mut Option<crate::equipment::RecordingReady>>,
    #[cfg(record_service)] recording_resource: Option<&'a c::ResourceBinding>,
    recording_issuer: RecordingIssuers<'_>,
    recording_operation: Option<&RecordingOperation<'_>>,
    pulse_issuer: &mut Issuer,
    write_issuer: &mut Issuer,
    wait_issuers: &mut [Issuer; WAIT_SLOTS],
    operations: &Operations<'_, WAIT_SLOTS>,
    sampling: &b::DelegatedBodyFragment<'_>,
    output: &b::DelegatedBodyFragment<'_>,
    sampling_profile: b::AssignedBodyPreparationProfile<'_>,
    output_profile: b::AssignedBodyPreparationProfile<'_>,
    sampling_fore: &[c::AssignedForeRequirement],
    output_fore: &[c::AssignedForeRequirement],
    contracts: &execution_group::NativeContracts,
) -> Result<(Sample<'a, 'f>, Output<'a, 'f>), &'static str> {
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
    let (table, handle, claim) = context
        .issue(
            pulse_issuer,
            policy,
            sampling,
            &NativeFacts::pulse(pulse.as_ref().ok_or("pulse-busy")?),
            &operations.pulse,
            crate::runtime::remaining(),
        )
        .map_err(|_| "pulse-authority")?;
    let pulse_contract = &contracts.pulse;
    let pulse = ReadyPulseBase::prepare_assigned_fore_retained(
        pulse,
        table,
        handle,
        claim,
        pulse_contract,
        SelectedAssignedForeOperationPlan {
            operation: operation(sampling, &sampling_profile, pn),
            fore: sampling_fore,
        },
    )
    .map_err(|_| "pulse-assignment")?;
    let (table, handle, claim) = context
        .issue(
            write_issuer,
            policy,
            output,
            &NativeFacts::write(write.as_ref().ok_or("write-busy")?),
            &operations.write,
            crate::runtime::remaining(),
        )
        .map_err(|_| "write-authority")?;
    let write_contract = &contracts.write;
    let write = ReadyWriteBase::prepare_assigned_fore_retained(
        write,
        table,
        handle,
        claim,
        write_contract,
        SelectedAssignedForeOperationPlan {
            operation: operation(output, &output_profile, wn),
            fore: output_fore,
        },
    )
    .map_err(|_| "write-assignment")?;
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
    let (table, handle, claim) = context
        .issue(
            issuer,
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
    // Only independently compiled Source selects this actual native owner.
    let mut append_nodes = source::sampling::NATIVE_KINDS
        .iter()
        .filter(|(_, kind)| {
            #[cfg(not(record_service))] { *kind == conduit_resource_snapshot::native_append::APPEND_KIND }
            #[cfg(record_service)] { *kind == conduit_resource_snapshot::service_contract::SERVICE_KIND }
        });
    let append_node = append_nodes.next().map(|(node, _)| *node);
    if append_nodes.next().is_some() {
        return Err("append-source-duplicate");
    }
    #[cfg(not(record_service))]
    let appender = match (append_node, contracts.append.as_ref(), recording_operation) {
        (Some(node), Some(contract), Some(op)) => {
            let recording = recording.ok_or("append-busy")?;
            let (table, handle, claim) = context
                .issue(
                    recording_issuer,
                    policy,
                    sampling,
                    &NativeFacts::append(recording.as_ref().ok_or("append-busy")?),
                    op,
                    crate::runtime::remaining(),
                )
                .map_err(|_| "append-authority")?;
            let ticket = ReadyRecordAppender::prepare_assigned_fore_retained(
                recording,
                table,
                handle,
                claim,
                contract,
                &[],
                SelectedAssignedForeOperationPlan {
                    operation: operation(sampling, &sampling_profile, node),
                    fore: sampling_fore,
                },
            )
            .map_err(|_| "append-assignment")?;
            Some(ticket)
        }
        (None, None, None) => None,
        _ => return Err("append-source-contract"),
    };
    #[cfg(record_service)]
    let service = {
        let node = append_node.ok_or("record-service-source")?;
        let operations = recording_operation.ok_or("record-service-operations")?;
        let recording = recording.ok_or("record-service-busy")?;
        let [append_issuer, read_issuer] = recording_issuer;
        let facts = NativeFacts::record_service(recording.as_ref().ok_or("record-service-busy")?);
        let issue = |issuer: &mut Issuer, op: &Operation<'_>| {
            let (table, handle, claim) = context.issue(issuer, policy, sampling, &facts, op, crate::runtime::remaining())
                .map_err(|_| "record-service-authority")?;
            c::BoundBaseOperation::bind(table, handle, claim).map_err(|_| "record-service-operation")
        };
        let append = issue(append_issuer, &operations[0])?;
        let read = issue(read_issuer, &operations[1])?;
        let selected = conduit_plan_lowering::selected_assigned_operation_set::SelectedAssignedServicePlan {
            operation: operation(sampling, &sampling_profile, node),
            fore: Some(sampling_fore),
            resource: source::sampling_layout::RECORD_SERVICE_RESOURCE.ok_or("record-service-resource-index")?,
        };
        conduit_resource_snapshot::native_service::ReadyRecordService::prepare_assigned_retained(
            recording, [append, read], recording_resource.ok_or("record-service-resource")?, selected,
        ).map_err(|_| "record-service-assignment")?
    };
    let sample_software = software
        .prepare_assigned(
            sampling.projection(),
            core::array::from_fn(|i| {
                !sample_nodes.iter().flatten().any(|n| usize::from(*n) == i)
                    && i != usize::from(pn)
                    && append_node.is_none_or(|node| i != usize::from(node))
            }),
        )
        .map_err(|_| "sampling-software")?;
    let out_software = output_software
        .prepare_assigned(
            output.projection(),
            core::array::from_fn(|i| {
                !output_nodes.iter().flatten().any(|n| usize::from(*n) == i) && i != usize::from(wn)
            }),
        )
        .map_err(|_| "output-software")?;
    #[cfg(not(record_service))]
    let sample_backs = PreparedDrivers::with_optional_appender(
        sample_loans,
        sample_nodes,
        sample_software,
        PreparedCalls {
            pulse: Some(pulse),
            write: None,
            waits: [wait],
        },
        appender,
    )?;
    #[cfg(record_service)]
    // SAFETY: this same Source factory remains behind original whole-workset
    // check_resources/Start, including the exact checked residence index above.
    let sample_backs = unsafe { PreparedDrivers::with_record_service::<_, SP,
        { conduit_resource_snapshot::read_result::READ_RESULT_BYTES }>(
        sample_loans, sample_nodes, sample_software,
        PreparedCalls { pulse: Some(pulse), write: None, waits: [wait] }, service,
    ) }?;
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
    Ok((sample_backs, output_backs))
}
