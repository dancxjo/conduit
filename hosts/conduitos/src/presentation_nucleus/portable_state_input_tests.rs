use conduit_core::{KeyEvent, KeyModifiers, KeyTransition};

use super::{prepare_portable_state_input, run_portable_state_input};

fn key() -> KeyEvent {
    KeyEvent::new(4, KeyTransition::Pressed, KeyModifiers::NONE).unwrap()
}

#[test]
fn count_toggle_and_key_tee_execute_in_one_ordinary_fixed_plan() {
    let prepared = prepare_portable_state_input("host-a", "boot-a", 7, true, key()).unwrap();
    let proof = run_portable_state_input(&prepared).unwrap();
    assert_eq!(proof.plan_id, prepared.plan.plan_id);
    assert_eq!(proof.counts, [7, 8]);
    assert_eq!(proof.toggles, [true, false]);
    assert_eq!(proof.text_key, key());
    assert_eq!(proof.chord_key, key());
    for kind in [
        conduit_std_catalog::STATE_COUNT_KIND,
        conduit_std_catalog::STATE_TOGGLE_KIND,
        conduit_std_catalog::KEY_EVENT_TEE_KIND,
    ] {
        assert_eq!(
            prepared
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .filter(|placement| placement.kind_id.as_str() == kind)
                .count(),
            1
        );
    }
}

#[test]
fn configuration_and_boot_identity_reseal_the_plan() {
    let base = prepare_portable_state_input("host-a", "boot-a", 7, true, key()).unwrap();
    let count = prepare_portable_state_input("host-a", "boot-a", 8, true, key()).unwrap();
    let toggle = prepare_portable_state_input("host-a", "boot-a", 7, false, key()).unwrap();
    let boot = prepare_portable_state_input("host-a", "boot-b", 7, true, key()).unwrap();
    assert_ne!(base.plan.plan_id, count.plan.plan_id);
    assert_ne!(base.plan.plan_id, toggle.plan.plan_id);
    assert_ne!(base.plan.plan_id, boot.plan.plan_id);
}

#[test]
fn authoritative_offers_keep_exact_portable_contracts() {
    let offers = [
        conduit_std_catalog::conduitos_state_count_offer(),
        conduit_std_catalog::conduitos_state_toggle_offer(),
        conduit_std_catalog::conduitos_key_event_tee_offer(),
    ];
    assert_eq!(
        offers[0].kind_contract_revision.as_str(),
        conduit_std_catalog::STATE_COUNT_CONTRACT_REVISION
    );
    assert_eq!(
        offers[1].kind_contract_revision.as_str(),
        conduit_std_catalog::STATE_TOGGLE_CONTRACT_REVISION
    );
    assert_eq!(
        offers[2].kind_contract_revision.as_str(),
        conduit_std_catalog::KEY_EVENT_TEE_REVISION
    );
    assert!(offers.iter().all(|offer| {
        offer.implementation.execution_profile_id.as_str()
            == conduit_std_catalog::CONDUITOS_PORTABLE_STATE_INPUT_PROFILE
            && offer.host_operations.is_empty()
            && offer.resource_requirements.is_empty()
    }));
}
