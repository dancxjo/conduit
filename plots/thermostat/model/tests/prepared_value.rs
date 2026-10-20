//! Thermostat is a pure Value transition: the caller commits only accepted output.
use conduit_kernel::{
    scheduler::{
        prepared_session::{OwnedValueBoundary, PreparedKernelSession},
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerStatus,
    },
    *,
};
use conduit_thermostat_plot::{Command, Mode, ThermostatBack, ThermostatState};
const BOUNDARIES: [OwnedValueBoundary; 3] = [
    OwnedValueBoundary {
        cord: CordId(0),
        endpoint: RemoteEndpointId(0),
    },
    OwnedValueBoundary {
        cord: CordId(1),
        endpoint: RemoteEndpointId(1),
    },
    OwnedValueBoundary {
        cord: CordId(2),
        endpoint: RemoteEndpointId(2),
    },
];
fn log() -> FixedSignLog<128> {
    FixedSignLog::new_with_remote_storage(
        128 * core::mem::size_of::<KernelEvent>() as u32,
        64,
        remote_sign_storage_bytes(64).unwrap(),
    )
    .unwrap()
}
#[test]
fn successive_thermostat_values_retain_one_kernel_and_commit_only_accepted_outputs() {
    let mut routes = FixedRoutes::<2, 1>::new(2);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(2),
                sink: CordEndpoint::Remote(RemoteEndpointId(2)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let capacity = |slot_start| CordCapacity {
        slot_start,
        item_capacity: 1,
        byte_capacity: 16,
        pressure_policy: Default::default(),
    };
    let kernel = FixedScheduler::<_, _, _, 1, 3, 2, 3, 2, 1>::new(
        [NodeSpec {
            input_cords: [Some(CordId(0)), Some(CordId(1))],
            maximum_step_fuel: 8,
        }],
        [
            CordSpec::remote_ingress(
                CordId(0),
                RemoteEndpointId(0),
                (NodeId(0), PortId(0)),
                capacity(0),
            ),
            CordSpec::remote_ingress(
                CordId(1),
                RemoteEndpointId(1),
                (NodeId(0), PortId(1)),
                capacity(1),
            ),
            CordSpec::remote_egress(
                CordId(2),
                (NodeId(0), PortId(0)),
                RemoteEndpointId(2),
                capacity(2),
            ),
        ],
        routes,
        [ThermostatBack::new()],
        FixedValueStore::<4, 16>::new(64).unwrap(),
        log(),
    )
    .unwrap();
    let mut session =
        PreparedKernelSession::new_with_owned_value_boundaries(Box::new(kernel), &BOUNDARIES, 32)
            .unwrap_or_else(|_| panic!("exact Value endpoints"));
    let address = session.kernel() as *const _ as usize;
    let mut committed = ThermostatState::default();
    for (sequence, command) in [
        Command::SetTarget(225),
        Command::SetMode(Mode::Heat),
        Command::Observe(Some(180)),
    ]
    .into_iter()
    .enumerate()
    {
        if sequence != 0 {
            let previous = session
                .begin_next([()], log())
                .unwrap_or_else(|_| panic!("previous output accepted and drained"));
            assert!(previous.signs.events().next().is_some());
        }
        let state_bytes = committed.encode().unwrap();
        let command_bytes = command.encode();
        for (boundary, bytes) in [
            (BOUNDARIES[0], state_bytes.as_slice()),
            (BOUNDARIES[1], command_bytes.as_slice()),
        ] {
            session
                .admit_owned_input(boundary.endpoint, boundary.cord, sequence as u64, bytes)
                .unwrap();
            session
                .close_owned_input(boundary.endpoint, boundary.cord)
                .unwrap();
        }
        session.step().unwrap();
        let offered = session
            .owned_output(BOUNDARIES[2].endpoint, BOUNDARIES[2].cord)
            .unwrap()
            .unwrap();
        assert_eq!(offered.sequence, sequence as u64);
        let candidate =
            ThermostatState::decode(session.kernel().values().get(offered.value).unwrap()).unwrap();
        assert_eq!(candidate, committed.apply(command).unwrap());
        assert!(
            session.begin_next([()], log()).is_err(),
            "unaccepted output cannot advance recurrence"
        );
        session
            .accept_owned_output(BOUNDARIES[2].endpoint, BOUNDARIES[2].cord, offered.sequence)
            .unwrap();
        let mut drained = false;
        for _ in 0..8 {
            if session.step().unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        committed = candidate;
        assert_eq!(address, session.kernel() as *const _ as usize);
        assert_eq!(session.kernel().values().used_items(), 0);
    }
    assert_eq!(committed.revision, 3);
    session.cancel().unwrap();
    assert!(session.begin_next([()], log()).is_err());
}
