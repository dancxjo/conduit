//! Actual lowered batch kernel; external read completions remain CPU fixtures.
use super::batch_plan::fixture;
use super::*;
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
#[test]
fn reverse_read_completion_still_recovers_all_overnight_chunks_in_source_order_without_allocations()
{
    execute::<4, 3>(1);
    execute::<60, 59>(15);
    execute::<64, 63>(16);
}
fn execute<const N: usize, const C: usize>(count: usize) {
    let contract = ObservationContract::prepare().unwrap();
    let mut contents = Vec::new();
    let mut references = Vec::new();
    let mut forms =
        conduit_environment_runtime::observation::PreparedObservationQuantityForms::prepare(
            &contract,
        )
        .unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap().to_vec();
    for index in 0..count {
        let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
        for offset in 1..=64 {
            let sequence = (index * 64 + offset) as u64;
            chunk
                .append(
                    sequence,
                    sequence * 60_000,
                    if sequence.is_multiple_of(17) {
                        RecordingDisposition::ProviderFailed
                    } else {
                        RecordingDisposition::Available
                    },
                    (!sequence.is_multiple_of(17)).then_some(observation.as_slice()),
                )
                .unwrap();
        }
        contents.push(chunk.seal().unwrap().to_owned());
        let mut r = reference();
        r.extent.bytes = contents.last().unwrap().len() as u64;
        r.lifetime.version = c::ResourceVersionIdentity::from_digest([(index + 1) as u8; 32]);
        references.push(r);
    }
    let mut fixture = fixture(&references);
    let plan = PreparedRecordingRecovery::prepare_batch(
        &fixture.source,
        &fixture.host,
        &fixture.choices,
        &bases(),
        &fixture.grants,
    )
    .unwrap();
    let prepared = PreparedRecordingBatchKernel::<N, C>::prepare(
        &plan.plan().fragments[0],
        plan.lowered(),
        &mut fixture.generations,
        &mut fixture.singleton,
        &mut fixture.concat,
        &mut fixture.recovery,
    )
    .unwrap();
    assert!(fixture
        .generations
        .iter()
        .all(|owner| owner.current_selection().is_none()));
    let recovery = prepared.recovery_node;
    let mut kernel = prepared.kernel;
    let read_indices: Vec<_> = prepared
        .read_nodes
        .iter()
        .map(|node| {
            let placement = &plan.plan().fragments[0]
                .placements
                .iter()
                .find(|p| p.placement_id == plan.lowered().nodes[usize::from(node.0)].placement_id)
                .unwrap();
            let content = &placement.resources[0].content.as_ref().unwrap().contract;
            let index = references
                .iter()
                .position(|r| {
                    r.identity == content.identity && r.lifetime.version == content.version
                })
                .unwrap();
            (*node, index)
        })
        .collect();
    let encoded: Vec<_> = references.iter().map(|r| r.encode().unwrap()).collect();
    let recovered = |kernel: &RecordingBatchKernel<N, C>| {
        let RecordingBack::Recovery(back) = &kernel.drivers()[usize::from(recovery.0)] else {
            unreachable!()
        };
        back.recovered()
    };
    let mut pending = [None; 16];
    let mut pending_count = 0;
    let mut drained = false;
    let ((), memory) = allocation_probe::observe(|| {
        // Dispatch every read, but complete none. A pending store cannot move
        // the history high-water mark, irrespective of other kernel work.
        for _ in 0..2000 {
            if let Some(request) = kernel.next_host_request() {
                pending[pending_count] = Some(request);
                pending_count += 1;
            }
            if pending_count == count {
                break;
            }
            kernel.step().unwrap();
        }
        assert_eq!(pending_count, count);
        assert_eq!(recovered(&kernel), RecoveredHistory::default());
        for request in pending.iter().rev().flatten() {
            let index = read_indices
                .iter()
                .find(|(node, _)| *node == request.node)
                .unwrap()
                .1;
            assert_eq!(
                kernel.values().get(request.input.value).unwrap(),
                encoded[index]
            );
            let output = kernel.store_host_value(contents[index].as_bytes()).unwrap();
            kernel
                .complete_host_call(
                    request.node,
                    request.request,
                    k::HostCallOutcome {
                        disposition: k::HostCallDisposition::Completed,
                        output: Some(k::BoundedValueRef::new(output, 4096).unwrap()),
                        failure: None,
                    },
                )
                .unwrap();
        }
        for _ in 0..4000 {
            if kernel.step().unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        assert_eq!(
            recovered(&kernel),
            RecoveredHistory {
                chunks: count as u16,
                samples: count as u32 * 64,
                first_sequence: Some(1),
                last_sequence: Some(count as u64 * 64)
            }
        );
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn stale_batch_tables_and_unavailable_owner_refuse_before_other_transfers() {
    let mut r = reference();
    r.lifetime.version = c::ResourceVersionIdentity::from_digest([1; 32]);
    let mut next = r.clone();
    next.lifetime.version = c::ResourceVersionIdentity::from_digest([2; 32]);
    let mut fixture = fixture(&[r, next]);
    let plan = PreparedRecordingRecovery::prepare_batch(
        &fixture.source,
        &fixture.host,
        &fixture.choices,
        &bases(),
        &fixture.grants,
    )
    .unwrap();
    let mut stale = plan.lowered().clone();
    stale.node_specs[0].maximum_step_fuel += 1;
    assert!(matches!(
        PreparedRecordingBatchKernel::<8, 7>::prepare(
            &plan.plan().fragments[0],
            &stale,
            &mut fixture.generations,
            &mut fixture.singleton,
            &mut fixture.concat,
            &mut fixture.recovery
        ),
        Err(RecordingKernelRefusal::Lowering)
    ));
    assert!(fixture
        .generations
        .iter()
        .all(|owner| owner.current_selection().is_some()));
    let fragment = &plan.plan().fragments[0];
    let gear = fixture.concat[0].current_selection().unwrap().0;
    let placement = &fragment
        .placements
        .iter()
        .find(|p| &p.gear_id == gear)
        .unwrap()
        .placement_id;
    let _retained = fixture.concat[0]
        .take_selected(fragment, placement)
        .unwrap();
    assert!(PreparedRecordingBatchKernel::<8, 7>::prepare(
        fragment,
        plan.lowered(),
        &mut fixture.generations,
        &mut fixture.singleton,
        &mut fixture.concat,
        &mut fixture.recovery
    )
    .is_err());
    assert!(fixture
        .generations
        .iter()
        .all(|owner| owner.current_selection().is_some()));
    assert!(fixture
        .singleton
        .iter()
        .all(|owner| owner.current_selection().is_some()));
    assert!(fixture.recovery.current_selection().is_some());
}

#[test]
fn failed_cancelled_and_invalid_batch_reads_do_not_publish_history() {
    for (disposition, invalid) in [
        (k::HostCallDisposition::Failed, false),
        (k::HostCallDisposition::Cancelled, false),
        (k::HostCallDisposition::Completed, true),
    ] {
        let contract = ObservationContract::prepare().unwrap();
        let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
        chunk
            .append(1, 10, RecordingDisposition::Checksum, None)
            .unwrap();
        let content = if invalid { "{}" } else { chunk.seal().unwrap() }.to_owned();
        let mut first = reference();
        first.extent.bytes = content.len() as u64;
        let mut second = first.clone();
        second.lifetime.version = c::ResourceVersionIdentity::from_digest([3; 32]);
        let mut fixture = fixture(&[first, second]);
        let plan = PreparedRecordingRecovery::prepare_batch(
            &fixture.source,
            &fixture.host,
            &fixture.choices,
            &bases(),
            &fixture.grants,
        )
        .unwrap();
        let prepared = PreparedRecordingBatchKernel::<8, 7>::prepare(
            &plan.plan().fragments[0],
            plan.lowered(),
            &mut fixture.generations,
            &mut fixture.singleton,
            &mut fixture.concat,
            &mut fixture.recovery,
        )
        .unwrap();
        let recovery = prepared.recovery_node;
        let mut kernel = prepared.kernel;
        let mut requests = [None; 2];
        let mut count = 0;
        let mut refused = false;
        let ((), memory) = allocation_probe::observe(|| {
            for _ in 0..200 {
                if let Some(request) = kernel.next_host_request() {
                    requests[count] = Some(request);
                    count += 1;
                }
                if count == 2 {
                    break;
                }
                kernel.step().unwrap();
            }
            assert_eq!(count, 2);
            for request in requests.iter().flatten().take(if invalid { 2 } else { 1 }) {
                let output = (disposition == k::HostCallDisposition::Completed).then(|| {
                    k::BoundedValueRef::new(
                        kernel.store_host_value(content.as_bytes()).unwrap(),
                        4096,
                    )
                    .unwrap()
                });
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition,
                            output,
                            failure: (disposition == k::HostCallDisposition::Failed).then_some(
                                k::Failure {
                                    code: k::FailureCode::HostCallFailed,
                                    detail: 1,
                                },
                            ),
                        },
                    )
                    .unwrap();
            }
            for _ in 0..200 {
                match kernel.step() {
                    Err(SchedulerError::BackFailed(_)) => {
                        refused = true;
                        break;
                    }
                    Err(error) => panic!("unexpected refusal: {error:?}"),
                    Ok(SchedulerStatus::Drained) => panic!("failed read became successful drain"),
                    _ => {}
                }
            }
            assert!(refused);
            let RecordingBack::Recovery(back) = &kernel.drivers()[usize::from(recovery.0)] else {
                unreachable!()
            };
            assert_eq!(back.recovered(), RecoveredHistory::default());
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
}
