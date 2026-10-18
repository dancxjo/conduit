//! A current owner observation, not an advertisement supplied by a Mask.
use super::*;
use crate::durable_host::owner::controller::OwnerHost;
#[test]
fn boot_and_generation_require_current_authenticated_owner_observation() {
    let mut owner = owner("boot/commons-observation");
    let current = owner.host.advertisement().clone();
    let before = owner.truth();
    let mut commons = CommonsEncounter::prepare().unwrap();
    let (face, show) = commons
        .present(Some(&owner), &current, &mut Vec::new())
        .unwrap();
    let request = interaction(&face, &show);
    let mut unobserved_boot = current.clone();
    unobserved_boot.boot_id = "boot/unobserved-commons-host".into();
    let mut unobserved_generation = current.clone();
    unobserved_generation.offer_generation = OfferGeneration(current.offer_generation.0 + 1);
    for stale in [unobserved_boot, unobserved_generation] {
        owner.host = OwnerHost::new(StdHost::from_advertisement(stale.clone()).unwrap());
        assert_eq!(
            commons.project(Some(&owner), &stale).unwrap_err(),
            "commons/current-owner-observation-unavailable"
        );
        assert_eq!(
            commons
                .apply_clock_change(&mut owner, Path::new("unused"), &show, &request)
                .unwrap_err(),
            "commons/current-owner-observation-unavailable"
        );
    }
    owner.host = OwnerHost::new(StdHost::from_advertisement(current).unwrap());
    assert_eq!(owner.truth(), before);
}
