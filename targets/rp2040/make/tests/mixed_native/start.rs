//! Actual Root Start gate against current original input and Core reservations.
use super::*;
use mixed_workset::{CommittedMixedWorkset, StartedMixedWorkset};

pub fn check<'w, 'o, P: I2cProvider>(
    mut committed: CommittedMixedWorkset<'w, 'o, P>,
    receiver: &receiver::Receiver,
    retained: &b::HostCommandInput<4096>,
    policy: Policy<'_>,
    owner: &ResourceAdmissionOwner,
    observations: &[ResourceObservation],
) -> StartedMixedWorkset<'w, 'o, P> {
    let address = &*committed.bound.kernel as *const _;
    macro_rules! refuse {
        ($input:expr, $policy:expr, $current:expr, $owner:expr, $pattern:pat) => {{
            let (result, cost) =
                crate::probe::observe(|| committed.start($input, $policy, $current, $owner));
            let (returned, reason) = result
                .err()
                .unwrap_or_else(|| panic!("Start unexpectedly accepted"));
            assert!(matches!(reason, $pattern));
            assert_eq!(&*returned.bound.kernel as *const _, address);
            assert_eq!((cost.allocations, cost.reallocations), (0, 0));
            committed = returned;
        }};
    }
    let mut expired = receiver.context();
    expired.now_millis = 20;
    refuse!(
        retained,
        policy,
        expired,
        owner,
        mixed_workset::Refusal::Retention(b::HostCommandRetentionRefusal::Clock)
    );
    let mut substitute = b::HostCommandInput::<4096>::new();
    let payload = retained.payload().unwrap();
    substitute.writable().unwrap()[..payload.len()].copy_from_slice(payload);
    substitute
        .retain(
            receiver
                .command_with_contract(payload, b::HOST_PREPARED_WORKSET_CONTRACT)
                .into_retained_stamp()
                .unwrap(),
        )
        .unwrap();
    let current = receiver.context();
    // The substitute is independently valid and contains identical bytes, but
    // it is a different nonce/transcript than the one actually admitted.
    substitute
        .reborrow(
            current.member,
            current.actual,
            &current.bootstrap.owner,
            b::HOST_PREPARED_WORKSET_CONTRACT,
            current.now_millis,
        )
        .unwrap();
    refuse!(
        &substitute,
        policy,
        receiver.context(),
        owner,
        mixed_workset::Refusal::Retention(b::HostCommandRetentionRefusal::Context)
    );
    refuse!(
        retained,
        Policy {
            execution: None,
            ..policy
        },
        receiver.context(),
        owner,
        mixed_workset::Refusal::Execution(b::ExecutionDelegationRefusal::MissingDelegation)
    );
    let mut different = ResourceAdmissionOwner::new(owner.host().clone());
    refuse!(
        retained,
        policy,
        receiver.context(),
        &different,
        mixed_workset::Refusal::Resources(b::DelegatedResourceRefusal::Scope)
    );
    let requests = owner
        .admissions()
        .iter()
        .map(|a| {
            let mut items = a.items.clone();
            for item in &mut items {
                if item.binding.class_id.as_str() == COOPERATIVE_STEP_WORK_CLASS_ID {
                    item.binding.units -= 1;
                    item.requirement.units -= 1;
                }
            }
            ResourceAdmissionRequest {
                plan_id: a.plan_id.clone(),
                placement_id: a.placement_id.clone(),
                host_id: a.host_id.clone(),
                boot_id: a.boot_id.clone(),
                offer_generation: a.offer_generation,
                items,
            }
        })
        .collect();
    different.admit_batch(requests, observations).unwrap();
    assert_eq!(different.admissions().len(), 2);
    refuse!(
        retained,
        policy,
        receiver.context(),
        &different,
        mixed_workset::Refusal::Resources(b::DelegatedResourceRefusal::Scope)
    );
    let (started, cost) =
        crate::probe::observe(|| committed.start(retained, policy, receiver.context(), owner));
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    println!("actual Root Start: expiry, substituted valid nonce, absent policy and missing/changed reservations refuse without allocation or lost custody; original attempt starts once");
    started.unwrap_or_else(|_| panic!("original admitted Start refused"))
}
