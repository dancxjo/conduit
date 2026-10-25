//! Retained idle Host transition, not native output or image manufacture proof.
use super::*;
use crate::{kernel_preparation::KernelResourceLedger, CrecheAttachmentRefusal as E};
use conduit_core::OfferGeneration;
fn slot() -> ResourceOffer {
    conduit_core::resource_offer("test/creche-slot", CRECHE_RESOURCE, 1)
}
fn assert_unchanged(host: &mut StdHost, resource: ResourceOffer, expected: E) {
    let before = host.advertisement.clone();
    let ledger = format!("{:?}", host.kernel_resources);
    assert_eq!(host.attach_creche_encounter(resource), Err(expected));
    assert_eq!(host.advertisement, before);
    assert_eq!(format!("{:?}", host.kernel_resources), ledger);
}
#[test]
fn additive_attach_preserves_retained_identity_equipment_registry_and_cursors() {
    let mut host = StdHost::new();
    host.next_kernel_play_sequence = 41;
    host.next_kernel_sign_sequence = 73;
    #[cfg(unix)]
    let selected = {
        use std::os::fd::AsRawFd;
        let (selected, peer) = std::os::unix::net::UnixStream::pair().unwrap();
        let fd = selected.as_raw_fd();
        host.terminal_attachment = Some(selected);
        (fd, peer)
    };
    let registry = host.base_registry.clone();
    let before = host.advertisement.clone();
    host.attach_creche_encounter(slot()).unwrap();
    let after = &host.advertisement;
    assert_eq!(after.host_id, before.host_id);
    assert_eq!(after.boot_id, before.boot_id);
    assert_eq!(after.protocol_version, before.protocol_version);
    assert_eq!(after.profile, before.profile);
    assert_eq!(after.bases, before.bases);
    assert_eq!(after.planner_capabilities, before.planner_capabilities);
    assert_eq!(after.offer_generation.0, before.offer_generation.0 + 1);
    let mut resources = before.resources;
    resources.push(slot());
    resources.sort();
    assert_eq!(after.resources, resources);
    let mut capabilities = before.capabilities;
    capabilities.push(creche_std_offer());
    capabilities.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    assert_eq!(after.capabilities, capabilities);
    assert_eq!(host.base_registry, registry);
    assert_eq!(host.next_kernel_play_sequence, 41);
    assert_eq!(host.next_kernel_sign_sequence, 73);
    assert!(host.kernel_resources.is_idle());
    host.kernel_resources
        .observe_offers(after, &[], &[])
        .unwrap();
    #[cfg(unix)]
    {
        use std::{
            io::{Read, Write},
            os::fd::AsRawFd,
        };
        assert_eq!(
            host.terminal_attachment.as_ref().unwrap().as_raw_fd(),
            selected.0
        );
        host.terminal_attachment
            .as_mut()
            .unwrap()
            .write_all(b"x")
            .unwrap();
        let mut peer = selected.1;
        let mut byte = [0];
        peer.read_exact(&mut byte).unwrap();
        assert_eq!(byte, *b"x");
    }
    assert_unchanged(&mut host, slot(), E::ExistingEncounter);
}
#[test]
fn invalid_slots_collisions_overflow_and_bad_current_ledger_are_atomic() {
    for resource in [
        conduit_core::resource_offer("", CRECHE_RESOURCE, 1),
        conduit_core::resource_offer("  ", CRECHE_RESOURCE, 1),
        conduit_core::resource_offer("test/slot", "other/resource@1", 1),
        conduit_core::resource_offer("test/slot", CRECHE_RESOURCE, 0),
        conduit_core::resource_offer("test/slot", CRECHE_RESOURCE, 2),
    ] {
        assert_unchanged(&mut StdHost::new(), resource, E::InvalidSlot);
    }
    let mut host = StdHost::new();
    host.advertisement
        .resources
        .push(conduit_core::resource_offer(
            slot().pool_id.as_str(),
            "test/unrelated@1",
            1,
        ));
    host.kernel_resources = KernelResourceLedger::new(&host.advertisement).unwrap();
    assert_unchanged(&mut host, slot(), E::PoolCollision);
    let mut host = StdHost::new();
    host.advertisement.capabilities.push(creche_std_offer());
    host.kernel_resources = KernelResourceLedger::new(&host.advertisement).unwrap();
    assert_unchanged(&mut host, slot(), E::ExistingEncounter);
    let mut host = StdHost::new();
    host.advertisement.resources.push(slot());
    host.kernel_resources = KernelResourceLedger::new(&host.advertisement).unwrap();
    assert_unchanged(&mut host, slot(), E::ExistingEncounter);
    let mut host = StdHost::new();
    host.advertisement.offer_generation = OfferGeneration(u64::MAX);
    host.kernel_resources = KernelResourceLedger::new(&host.advertisement).unwrap();
    assert_unchanged(&mut host, slot(), E::GenerationExhausted);
    let mut host = StdHost::new();
    host.advertisement.offer_generation.0 += 1;
    let before = host.advertisement.clone();
    let ledger = format!("{:?}", host.kernel_resources);
    assert!(matches!(
        host.attach_creche_encounter(slot()),
        Err(E::InvalidLedger(_))
    ));
    assert_eq!(host.advertisement, before);
    assert_eq!(format!("{:?}", host.kernel_resources), ledger);
}
#[test]
fn active_reservation_refuses_and_idle_generation_transition_invalidates_old_plan() {
    let (mut host, old_plan, _) = crate::host_execution::scalar_tape_fixture::prepare(
        "plot arithmetic (\n >> value: U64\n result: U64 >>\n) = (. + 1)\n",
        "arithmetic",
        true,
    );
    let ad = host.advertisement.clone();
    let reservation = host
        .kernel_resources
        .prepare_and_reserve(&ad, &old_plan.fragments[0])
        .unwrap();
    assert!(!host.kernel_resources.is_idle());
    assert_unchanged(&mut host, slot(), E::HostActive);
    host.kernel_resources.release(reservation).unwrap();
    host.attach_creche_encounter(slot()).unwrap();
    assert!(host
        .kernel_resources
        .prepare_and_reserve(&host.advertisement, &old_plan.fragments[0])
        .is_err());
    assert!(host.kernel_resources.is_idle());
    let mut adapter = crate::hosted_creche::test_support::Adapter::new();
    let (ad, plan, play, _) =
        crate::hosted_creche::test_support::planned_on_host(host.advertisement.clone());
    let mut scope =
        crate::hosted_creche::CrecheHost::prepare(&mut adapter, &ad, &plan, &play).unwrap();
    let request = scope.request();
    assert!(
        scope.begin(request).is_err(),
        "installation cannot invent actual issued Play"
    );
    scope.retire().unwrap();
    drop(scope);
    assert_eq!(adapter.begins, 0);
}
