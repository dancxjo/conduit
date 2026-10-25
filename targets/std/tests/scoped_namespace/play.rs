use super::*;
use conduit_kernel::{
    scheduler::{
        CordSpec, FixedScheduler, SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, HostCallDisposition, HostCallId,
    HostCallOutcome, HostedSignLog, HostedValueStore, RequestId, ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
struct Back {
    input: ValueRef,
    call: HostCallId,
    requested: bool,
    refused: bool,
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((_, outcome)) = io.host_completion() {
            let successful = outcome.disposition == HostCallDisposition::Completed;
            io.consume_host_completion().unwrap();
            if successful {
                return StepOutcome::Complete;
            }
            // Apply completion consumption before reporting a failed Step;
            // failed Steps do not commit their staged I/O effects.
            self.refused = true;
            return StepOutcome::Progress;
        }
        if self.refused {
            return StepOutcome::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallFailed,
                detail: 1,
            });
        }
        if !self.requested {
            io.request_host_call(
                RequestId(1),
                self.call,
                BoundedValueRef::new(self.input, 1).unwrap(),
            )
            .unwrap();
            self.requested = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
}
type Runtime =
    FixedScheduler<Back, HostedValueStore, HostedSignLog, 1, 1, PORTS, 1, PORTS, 1, 4, 1>;
fn execute(operation: &str, refuse: bool) {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("hello"), b"hello").unwrap();
    let mut provider = provider(temp.path(), 1);
    let (mut list, c) = issue(&mut provider, LIST_CALL);
    let entry = provider.list(&mut list, &c).unwrap()[0].entry_id.clone();
    let (fragment, request, mut admission_owner) = admission_full(operation);
    let mut claim = claim(&request);
    let mut possession = provider
        .issue(request, &fragment, &fragment.placements[0].placement_id)
        .unwrap();
    if refuse {
        claim.base_provider_generation = 2;
    }
    let lowered = lower_plan_fragment(&fragment).unwrap();
    assert_eq!(lowered.nodes.len(), 1);
    let mut routes = FixedRoutes::<PORTS, 1>::new(PORTS as u16);
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::<4>::new(1);
    for call in &lowered.host_calls {
        calls.install(call.node, call.binding).unwrap();
    }
    calls.seal().unwrap();
    let mut values = HostedValueStore::new(2, 8, 16).unwrap();
    let input = values.store(&[0]).unwrap();
    let signs = HostedSignLog::new(
        64,
        64 * std::mem::size_of::<conduit_kernel::KernelEvent>() as u32,
    )
    .unwrap();
    let mut runtime = Runtime::new_with_active_counts_and_host_calls(
        1,
        0,
        lowered.node_specs.try_into().ok().unwrap(),
        [CordSpec::inactive()],
        routes,
        calls,
        [Back {
            input,
            call: lowered.host_calls[0].binding.call,
            requested: false,
            refused: false,
        }],
        values,
        signs,
    )
    .unwrap();
    let mut reached = 0;
    let mut success = false;
    let mut terminal = false;
    for _ in 0..8 {
        let status = match runtime.step() {
            Ok(status) => status,
            Err(conduit_kernel::scheduler::SchedulerError::BackFailed(failure)) if refuse => {
                assert_eq!(failure.code, conduit_kernel::FailureCode::HostCallFailed);
                terminal = true;
                break;
            }
            Err(error) => panic!("unexpected kernel outcome: {error:?}"),
        };
        if matches!(status, SchedulerStatus::Drained) {
            terminal = true;
            break;
        }
        while let Some(call) = runtime.next_host_request() {
            assert_eq!(
                lowered.identity.placement_for_node(call.node),
                Some(&fragment.placements[0].placement_id)
            );
            let mut output = [0u8; 256];
            let result = if operation == LIST_CALL {
                provider
                    .list(&mut possession, &claim)
                    .map(|entries| entries.len())
            } else {
                provider.open_read(&mut possession, &claim, &entry, &mut output)
            };
            reached += 1;
            success = result.is_ok();
            if success && operation == OPEN_CALL {
                assert_eq!(&output[..5], b"hello");
            }
            runtime
                .complete_host_call(
                    call.node,
                    call.request,
                    HostCallOutcome {
                        disposition: if success {
                            HostCallDisposition::Completed
                        } else {
                            HostCallDisposition::Denied
                        },
                        output: None,
                        failure: (!success).then_some(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostCallFailed,
                            detail: 1,
                        }),
                    },
                )
                .unwrap();
        }
    }
    assert!(terminal);
    assert_eq!(reached, 1);
    assert_eq!(success, !refuse);
    assert_eq!(runtime.pending_host_call_count(), 0);
    provider.revoke_play(&claim.active_play_id);
    admission_owner
        .release(&fragment.plan_id, &fragment.placements[0].placement_id)
        .unwrap();
    assert_eq!(fs::read(temp.path().join("hello")).unwrap(), b"hello");
}
#[test]
fn ordinary_lowered_kernel_play_reaches_selected_list_and_open_boundary() {
    execute(LIST_CALL, false);
    execute(OPEN_CALL, false);
}
#[test]
fn ordinary_kernel_play_preserves_provider_generation_refusal() {
    execute(OPEN_CALL, true);
}
