//! Production context run/cleanup; GPIO acknowledgements are explicit CPU models.
use super::*;
use std::{cell::Cell, rc::Rc};
struct Pins {
    edges: Rc<Cell<usize>>,
    nack: bool,
}
impl conduit_tm1637_wire::OpenDrain for Pins {
    type Error = core::convert::Infallible;
    fn clock(&mut self, _: bool) -> Result<(), Self::Error> {
        self.edges.set(self.edges.get() + 1);
        Ok(())
    }
    fn data(&mut self, _: bool) -> Result<(), Self::Error> {
        self.edges.set(self.edges.get() + 1);
        Ok(())
    }
    fn read_data(&mut self) -> Result<bool, Self::Error> {
        Ok(self.nack)
    }
    fn half_period(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
pub(super) fn check(
    assignment: &ValidatedAssignedDisplay,
    fragment: &c::PlanFragment,
    wire: &[u8],
    profile: &AssignedDisplayProfile<'_>,
    resident: &[c::AssignedRecord<'_>],
) {
    for nack in [false, true] {
        let source = DisplayOutputSource::compile().unwrap();
        let mut software = source
            .prepare_software(fragment.host_id.clone(), fragment.boot_id.clone())
            .unwrap();
        software
            .current_offers(&fragment.host_id, &fragment.boot_id, c::OfferGeneration(1))
            .unwrap();
        let mut prepared = native_display_kernel::prepare(
            wire,
            profile,
            resident,
            AssignedDisplayScope {
                host: &fragment.host_id,
                boot: &fragment.boot_id,
                generation: c::OfferGeneration(1),
            },
            &mut software,
            2,
            usize::MAX,
        )
        .unwrap();
        let mut encoder =
            conduit_environment_runtime::display_packets::PreparedDisplayPackets::prepare()
                .unwrap();
        let frame = encoder
            .encode_frame([0x7d, 0xff, 0x66, 0x71, 0, 0], 3, true)
            .unwrap();
        prepared
            .kernel
            .admit_remote_input(
                conduit_kernel::RemoteEndpointId(prepared.input.endpoint()),
                conduit_kernel::CordId(prepared.input.cord()),
                0,
                frame,
            )
            .unwrap();
        prepared
            .kernel
            .close_remote_input(
                conduit_kernel::RemoteEndpointId(prepared.input.endpoint()),
                conduit_kernel::CordId(prepared.input.cord()),
            )
            .unwrap();
        fixtures::scope_mode(3);
        let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
        fixtures::scope_mode(0);
        let active =
            c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
        issuer
            .reserve_display_play_matching(
                assignment,
                &fragment.plan_id,
                &fragment.fragment_id,
                active.active_play_id.as_str(),
            )
            .unwrap();
        let edges = Rc::new(Cell::new(0));
        let identity = display_fixture::ready(&fragment.host_id, &fragment.boot_id)
            .native_identity()
            .clone();
        // SAFETY: owned finite CPU pin model; no physical attachment is claimed.
        let mut ready = Some(
            unsafe {
                conduit_tm1637_wire::installation::ReadyWriteBase::new(
                    identity,
                    conduit_tm1637_wire::owner::WriteAttachment {
                        generation: 1,
                        resource_bytes: 64,
                    },
                    Pins {
                        edges: edges.clone(),
                        nack,
                    },
                )
            }
            .unwrap(),
        );
        let owner = native_display_binding::bind(
            &mut issuer,
            &mut ready,
            native_display_binding::BindingInput {
                assignment,
                wire,
                profile,
                resident,
                fragment: &fragment.fragment_id,
                active: &active,
                maximum_frames: 2,
                available_bytes: usize::MAX,
            },
        )
        .unwrap();
        let mut storage = Some(prepared);
        let mut owner = Some(owner);
        let mut execution =
            Some(display_execution::Execution::boxed_retained(&mut storage, &mut owner).unwrap());
        assert_eq!(edges.get(), 0);
        let (outcome, usage) = allocation_probe::observe(|| {
            complete(execution.as_mut().unwrap().run(core::future::pending()))
        });
        assert_eq!((usage.allocations, usage.reallocations), (0, 0));
        assert_eq!(outcome.cleanup, Ok(()));
        assert_eq!(outcome.progress.native_calls, 1);
        assert!(outcome.progress.steps > 0);
        assert!(edges.get() > 0);
        if nack {
            assert!(
                matches!(outcome.exit,display_execution::Exit::Failed(k) if k.code==conduit_kernel::FailureCode::HostCallFailed)
            );
        } else {
            assert_eq!(outcome.exit, display_execution::Exit::Drained);
        }
        assert_eq!(
            execution.as_ref().unwrap().inspection().lifecycle,
            c::CapabilityLifecycle::Revoked
        );
        let retired = display_execution::Execution::retire_retained(&mut execution).unwrap();
        issuer.finish_display_play(&active, &retired).unwrap();
    }
}
fn complete<T>(future: impl core::future::Future<Output = T>) -> T {
    let mut future = core::pin::pin!(future);
    let mut context = core::task::Context::from_waker(core::task::Waker::noop());
    for _ in 0..1000 {
        if let core::task::Poll::Ready(result) = future.as_mut().poll(&mut context) {
            return result;
        }
    }
    panic!("bounded output fixture failed to terminate");
}
