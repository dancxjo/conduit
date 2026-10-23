//! Both representations enter the ordinary table builder; returned tables own
//! all execution data after this helper's descriptors and payloads are dropped.
use super::*;
use conduit_environment_runtime::assigned::{AssignedKernelTables as Tables, ResidentLayout};
use conduit_kernel as k;
pub fn check<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    bytes: &[u8],
    maxima: AssignedPlanMaxima,
    required: AssignedPlanRequirements<'_>,
    profile: AssignedComputationProfile<'_>,
    layout: &ResidentLayout<'_>,
    expected: &Tables<N, C, P, R, T, H>,
) -> Tables<N, C, P, R, T, H> {
    let records: Vec<_> = AssignedPlanRecords::new(bytes)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let (actual, memory) = super::super::allocation_probe::observe(|| {
        Tables::prepare_records(
            &bytes[..ASSIGNED_PLAN_HEADER_BYTES],
            &records,
            maxima,
            required,
            profile,
            layout,
        )
        .unwrap()
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(actual.identity, expected.identity);
    assert_eq!(actual.nodes, expected.nodes);
    assert_eq!(actual.cords, expected.cords);
    assert_eq!(actual.startup_order(), expected.startup_order());
    assert!(actual.calls.is_sealed());
    for node in 0..N {
        for port in 0..P {
            let route = |tables: &Tables<N, C, P, R, T, H>| {
                tables
                    .routes
                    .route(
                        k::NodeId(node.try_into().unwrap()),
                        k::PortId(port.try_into().unwrap()),
                    )
                    .map(|targets| targets.collect::<Vec<_>>())
            };
            assert_eq!(route(&actual), route(expected));
        }
    }
    for change in 0..4 {
        let mut altered = bytes.to_vec();
        let mut bound = maxima;
        match change {
            0 => altered[64] ^= 1, // stale Boot
            1 => {
                let last = altered.len() - 1;
                altered[last] ^= 1;
            }
            2 => {
                // Valid canonical integrity cannot replace independent layout.
                altered[ASSIGNED_PLAN_HEADER_BYTES + 3 + 2] ^= 1;
                let digest = assigned_plan_payload_digest(&altered[ASSIGNED_PLAN_HEADER_BYTES..]);
                altered[93..125].copy_from_slice(&digest);
            }
            _ => bound.encoded_bytes = 0,
        }
        let records: Vec<_> = AssignedPlanRecords::new(&altered)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let (both, memory) = super::super::allocation_probe::observe(|| {
            (
                Tables::<N, C, P, R, T, H>::prepare(&altered, bound, required, profile, layout)
                    .err(),
                Tables::<N, C, P, R, T, H>::prepare_records(
                    &altered[..ASSIGNED_PLAN_HEADER_BYTES],
                    &records,
                    bound,
                    required,
                    profile,
                    layout,
                )
                .err(),
            )
        });
        assert!(both.0.is_some());
        assert_eq!(both.0, both.1);
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    actual
}
