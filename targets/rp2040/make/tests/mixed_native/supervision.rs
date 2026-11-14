//! Actual Root supervisor and mixed kernel over deterministic platform leaves.
use super::*;
use crate::{
    execution_supervisor::{self, OwnedWorkset},
    mixed_supervision::MixedWorkset,
    supervisor_platform as platform,
};
use core::{cell::RefCell, pin::pin};
pub struct Flow<'a> {
    pub ingress: (RemoteEndpointId, CordId),
    pub egress: (RemoteEndpointId, CordId),
    pub input: &'a [u8],
    pub expected: &'a [u8],
}
pub fn exercise<P: I2cProvider>(
    started: mixed_workset::StartedMixedWorkset<'_, '_, P>,
    policy: Policy<'_>,
    context: b::NativePreparationContext<'_>,
    input: &b::HostCommandInput<4096>,
    resources: &mut ResourceAdmissionOwner,
    flow: Flow<'_>,
) {
    let workset = RefCell::new(MixedWorkset::from_started(started, policy));
    workset
        .borrow_mut()
        .bound_mut()
        .unwrap()
        .kernel
        .admit_remote_input(flow.ingress.0, flow.ingress.1, 0, flow.input)
        .unwrap();
    platform::reset();
    let mut link = platform::Link;
    let mut delivered = false;
    let ((), cost) = crate::probe::observe(|| {
        {
            let mut worker = pin!(execution_supervisor::supervise(
                &mut link,
                input,
                context.bootstrap,
                context.member,
                resources,
                &workset
            ));
            platform::poll_once(worker.as_mut());
            // Original attribution expires at 20. A started Play continues with
            // no carrier, through reconnection and another disconnection.
            for now in [21, 31, 41, 42, 51, 61, 71] {
                platform::at(now);
                if now == 31 {
                    platform::connect();
                }
                if now == 41 {
                    platform::disconnect();
                }
                platform::poll_once(worker.as_mut());
                assert!(workset.borrow().is_started());
                if now >= 51 && !delivered {
                    let mut current = workset.borrow_mut();
                    let bound = current.bound_mut().unwrap();
                    if let Some(offer) = bound
                        .kernel
                        .remote_egress_offer(flow.egress.0, flow.egress.1)
                        .unwrap()
                    {
                        assert_eq!(bound.kernel.host_value(offer.value).unwrap(), flow.expected);
                        bound
                            .kernel
                            .remote_egress_accept(flow.egress.0, flow.egress.1, offer.sequence)
                            .unwrap();
                        bound
                            .kernel
                            .remote_egress_delivered(flow.egress.0, flow.egress.1, offer.sequence)
                            .unwrap();
                        delivered = true;
                    }
                }
            }
        }
        assert!(delivered);
        assert!(platform::resets() >= 1);
        assert_eq!(resources.admissions().len(), 2); // Carrier loss released nothing.
        assert!(input
            .reborrow(
                context.member,
                context.actual,
                &context.bootstrap.owner,
                b::HOST_PREPARED_WORKSET_CONTRACT,
                71
            )
            .is_err());
        assert!(workset
            .borrow_mut()
            .retire(resources, ResourceReleaseCause::Cancelled));
        assert!(resources.admissions().is_empty());
        assert_eq!(workset.borrow().terminal_count(), 1);
        assert!((0..16)
            .filter_map(|n| workset.borrow().terminal_event(0, n))
            .any(|e| e.kind == KernelEventKind::RunCancelled));
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    println!("actual shared Root supervisor: typed mixed output survives absent/disconnected carrier and attribution expiry; reservations persist until explicit Root retirement; terminal Signs retained; zero execution allocation; CPU platform leaves only");
}
