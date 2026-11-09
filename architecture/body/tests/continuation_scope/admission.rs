//! Exercise the SAME Core admission bridge; no kernel Start or provider exists.
use conduit_body::*;
use conduit_core as c;
pub fn check(
    scope: &FreshContinuationScope<'_>,
    verified: &VerifiedContinuationIntent<'_>,
    actual: &c::HostAdvertisement,
) {
    let mut fragments = Vec::new();
    for index in 0..3 {
        let profile = AssignedBodyPreparationProfile {
            maxima: c::AssignedPlanMaxima {
                encoded_bytes: 4096,
                runtime_state_bytes: 4096,
                counts: [255; 13],
            },
            requirements: c::AssignedPlanRequirements {
                host: c::AssignedIdentity::from_text(actual.host_id.as_str()),
                boot: c::AssignedIdentity::from_text(actual.boot_id.as_str()),
                host_calls: &[],
                resources: &[0],
                remote_bindings: &[],
            },
            computations: c::AssignedComputationProfile {
                maximum_text_bytes: 0,
                maximum_total_text_bytes: 0,
                requirements: &[],
            },
            fore: None,
        };
        fragments.push(
            scope
                .approve_fragment::<0>(index, verified, actual, profile)
                .unwrap()
                .0,
        );
    }
    let mut owner = c::ResourceAdmissionOwner::new(actual.clone());
    let pool = &actual.resources[0];
    let observation = c::ResourceObservation {
        host_id: actual.host_id.clone(),
        boot_id: actual.boot_id.clone(),
        offer_generation: actual.offer_generation,
        pool_id: pool.pool_id.clone(),
        class_id: pool.class_id.clone(),
        health: c::ResourceHealth::Ready,
        unreserved_units: 3,
        utilized_units: 0,
        sign_id: "observed".into(),
    };
    let refs: Vec<_> = fragments.iter().collect();
    let before = owner.clone();
    assert!(admit_delegated_resources(&refs, &mut owner, &[]).is_err());
    assert_eq!(owner, before);
    admit_delegated_resources(&refs, &mut owner, &[observation]).unwrap();
    assert_eq!(owner.admissions().len(), 3);
    assert!(owner
        .admissions()
        .iter()
        .all(|a| a.host_id == actual.host_id && a.boot_id == actual.boot_id));
}
