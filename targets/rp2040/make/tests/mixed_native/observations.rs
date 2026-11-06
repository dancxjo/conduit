//! Production prepared-custody observations, not an arbitrary Ready fixture.
use super::*;
use crate::mixed_workset::PreparedMixedWorkset;

pub fn check<P: I2cProvider>(
    prepared: &PreparedMixedWorkset<'_, '_, '_, P>,
    owner: &ResourceAdmissionOwner,
    work_capacity: u32,
) -> usize {
    let (observed, cost) =
        crate::probe::observe(|| prepared.observe_owned_resources(owner, work_capacity, 1, 4096));
    let observed = observed.unwrap();
    assert_eq!(observed.as_slice().len(), 2);
    assert_eq!(observed.owned_bytes, cost.requested_bytes);
    assert_eq!(cost.allocations, 10);
    assert_ne!(
        observed.as_slice()[0].sign_id,
        observed.as_slice()[1].sign_id
    );
    for entry in observed.as_slice() {
        let pool = owner
            .host()
            .resources
            .iter()
            .find(|p| p.pool_id == entry.pool_id)
            .unwrap();
        assert_eq!(entry.health, ResourceHealth::Ready);
        assert_eq!(entry.unreserved_units, pool.capacity_units);
        assert_eq!(entry.host_id, owner.host().host_id);
        assert_eq!(entry.boot_id, owner.host().boot_id);
        assert_eq!(entry.offer_generation, owner.host().offer_generation);
    }
    let total = prepared
        .preparation_bytes(owner, observed.as_slice())
        .unwrap()
        + observed.owned_bytes;
    for (capacity, sequence, budget) in [
        (work_capacity, 0, 4096),
        (work_capacity - 1, 1, 4096),
        (work_capacity, 1, observed.owned_bytes - 1),
    ] {
        let (refused, probe) = crate::probe::observe(|| {
            prepared.observe_owned_resources(owner, capacity, sequence, budget)
        });
        assert!(refused.is_err());
        assert_eq!((probe.allocations, probe.reallocations), (0, 0));
    }
    for case in 0..4 {
        let mut actual = owner.host().clone();
        match case {
            0 => actual
                .resources
                .retain(|p| p.class_id.as_str() != I2C_ATTACHMENT_CLASS),
            1 => {
                actual
                    .resources
                    .iter_mut()
                    .find(|p| p.class_id.as_str() == I2C_ATTACHMENT_CLASS)
                    .unwrap()
                    .capacity_units = 2
            }
            2 => actual.resources.push(actual.resources[0].clone()),
            _ => actual.boot_id = "other/boot".into(),
        }
        let contrary = ResourceAdmissionOwner::new(actual);
        let (result, refused) = crate::probe::observe(|| {
            prepared.observe_owned_resources(&contrary, work_capacity, 1, 4096)
        });
        assert!(result.is_err());
        assert_eq!((refused.allocations, refused.reallocations), (0, 0));
        assert!(contrary.admissions().is_empty());
    }
    for nth in 1..=cost.allocations {
        let (result, probe) = crate::probe::observe_refusing(nth, || {
            prepared.observe_owned_resources(owner, work_capacity, 1, 4096)
        });
        assert!(matches!(
            result,
            Err(mixed_workset::Refusal::Resources(
                b::DelegatedResourceRefusal::Allocation
            ))
        ));
        assert_eq!(probe.refused_allocations, 1);
        assert_eq!(probe.allocations, probe.deallocations);
    }
    println!("actual prepared-custody observations: {} B in ten fallible allocations; native/resident/Core total {total} B; all observation shortages/failures leave original ledger unchanged",observed.owned_bytes);
    total
}
