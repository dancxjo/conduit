//! Fixture producer, real sealed Plan lowering and production scheduler.
//! This proves routing, not Host-issued Play/Sign identity or Std export.
use super::*;
use conduit_kernel::scheduler::{
    CanonicalValue, FixedScheduler, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, KernelEventKind, ValueRef,
    ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment_from_plan, FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS,
};

struct Producer {
    track: ConnectionTrack,
    port: conduit_kernel::PortId,
    value: ValueRef,
    emitted: bool,
}
impl StepBack<PORTS> for Producer {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self.track {
            ConnectionTrack::Payload if !self.emitted => {
                io.send(self.port, self.value).unwrap();
                self.emitted = true;
                StepOutcome::Progress
            }
            ConnectionTrack::Payload | ConnectionTrack::NormalClose => StepOutcome::Complete,
            ConnectionTrack::AbnormalTerminal => StepOutcome::Abnormal {
                port: self.port,
                terminal: CanonicalValue::new(b"sample").unwrap(),
            },
            ConnectionTrack::Quiescence if !self.emitted => {
                // An idle producer has no quiescence edge. Exercise an admitted
                // cooperative work grant before the producer becomes idle.
                io.exhaust_fuel();
                self.emitted = true;
                StepOutcome::Yield
            }
            ConnectionTrack::Quiescence => StepOutcome::Await,
        }
    }
    fn cancel(&mut self) {}
}

#[test]
fn sealed_output_fore_tracks_route_actual_bytes_and_native_events() {
    for track in [
        ConnectionTrack::Payload,
        ConnectionTrack::NormalClose,
        ConnectionTrack::AbnormalTerminal,
        ConnectionTrack::Quiescence,
    ] {
        let envelope = artifact_context(track, true);
        let plan = &envelope.sign.play.as_ref().unwrap().plan;
        let (lowered, _) =
            lower_plan_fragment_from_plan(plan, &plan.fragments[0].fragment_id).unwrap();
        assert_eq!((lowered.nodes.len(), lowered.cords.len()), (1, 1));
        let fore = &lowered.fore_ports[0];
        assert_eq!(fore.track, track);
        let route = &lowered.routes[0];
        let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
        routes.seal().unwrap();
        let mut values = FixedValueStore::<4, 1024>::new(1024).unwrap();
        let value = values.store(b"sample").unwrap();
        let mut scheduler = FixedScheduler::<_, _, _, 1, 1, PORTS, 1, 1, 1, 1, 1>::new(
            [lowered.node_specs[0]],
            [lowered.cords[0].spec],
            routes,
            [Producer {
                track,
                port: route.source_port,
                value,
                emitted: false,
            }],
            values,
            FixedSignLog::<64>::new_with_remote_storage(
                (64 * core::mem::size_of::<KernelEvent>()) as u32,
                16,
                conduit_kernel::remote_sign_storage_bytes(16).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        // Fixed bound includes the scheduler's global quiescence transition.
        for _ in 0..4 {
            scheduler.step().unwrap();
        }
        let offer = scheduler
            .remote_egress_offer(fore.endpoint, fore.cord)
            .unwrap()
            .unwrap();
        assert_eq!(scheduler.values().get(offer.value).unwrap(), envelope.bytes);
        let events: Vec<_> = scheduler
            .signs()
            .events()
            .filter(|e| e.kind == KernelEventKind::ValueRouted)
            .collect();
        assert_eq!(events.len(), 1, "{track:?}");
        assert_eq!(events[0].node, route.source_node);
        assert_eq!(events[0].port, Some(route.source_port));
        assert_eq!(offer.cord, fore.cord);
    }
}
