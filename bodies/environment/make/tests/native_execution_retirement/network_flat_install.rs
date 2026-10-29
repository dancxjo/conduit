//! Actual qualification-profile installer with inert SDK adapters. The complete
//! Source, ordinary planner, Root policy, selected owners and kernel are real;
//! this does not prove the target Root future, stack or physical radio.
use super::{identity, source_image, Inert};
use crate::{
    allocation_probe, c,
    native_authority::{Operation, RootIssuer},
};
use conduit_kernel as k;
use conduit_network_join_call::{
    self as n, credential_installation::ReadyCredentialBase, installation::ReadyNetworkJoinBase,
    PreparedNetworkKernel,
};
type Source = Inert;
type Provider = Inert;
const CAP: &str = "conduit/esp32/network-attachment-receipt@1";
const IMPLEMENTATION: &str = "esp32/network-attachment-receipt@1";
const ARTIFACT: &str = "native/esp32-network-attachment-receipt@1";
struct NativePlay {
    attempted: bool,
    configuration_revision: u64,
    phase: &'static str,
    active: Option<c::ActivePlayIdentity>,
    source: Option<n::NetworkCredentialsHostCall<Source>>,
    join: Option<n::NetworkJoinHostCall<Provider>>,
    prepared: Option<Box<PreparedNetworkKernel>>,
    flat_refusal: flat::IssuedCustody,
}
impl NativePlay {
    fn new() -> Self {
        Self {
            attempted: false,
            configuration_revision: 0,
            phase: "uninstalled",
            active: None,
            source: None,
            join: None,
            prepared: None,
            flat_refusal: flat::IssuedCustody::Empty,
        }
    }
}
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_play_flat.rs"]
mod flat;
fn ready() -> (
    Option<ReadyCredentialBase<Source>>,
    Option<ReadyNetworkJoinBase<Provider>>,
) {
    // SAFETY: exclusive initialized transcript providers; no SDK readiness claim.
    unsafe {
        (
            Some(ReadyCredentialBase::new(identity("private-network-input"), Inert).unwrap()),
            Some(ReadyNetworkJoinBase::new(identity("wifi-station"), u64::MAX, Inert).unwrap()),
        )
    }
}
fn issuer() -> RootIssuer {
    RootIssuer::fixture(
        "esp32/b0a732286178".into(),
        "boot/0123456789abcdef0123456789abcdef".into(),
    )
}
#[test]
fn temporary_placement_table_refusal_returns_ready_owners_before_root_issuance() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let count = image.linked_planning_facts().unwrap().gears.len();
    let _refusal = allocation_probe::refuse_next_allocation_size(
        count * core::mem::size_of::<c::PlainLocalPlacementRef<'_>>(),
    );
    assert!(play
        .install_linked_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
        )
        .is_err());
    assert!(source.is_some() && join.is_some());
    assert!(play.active.is_none());
    assert!(play.flat_refusal.is_empty());
    assert!(play.prepared.is_none() && play.source.is_none() && play.join.is_none());
    assert_eq!(play.phase, "installation-refused");
}
#[test]
fn actual_flat_installer_never_allocates_the_full_placement_table() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let count = image.linked_planning_facts().unwrap().gears.len();
    assert!(
        core::mem::size_of::<c::PlainLocalPlacementRef<'_>>()
            < core::mem::size_of::<c::PlannedGearRef<'_>>()
    );
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let _refusal = allocation_probe::refuse_next_allocation_size(
        count * core::mem::size_of::<c::PlannedGearRef<'_>>(),
    );
    play.install_linked_held(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
    )
    .unwrap();
    assert!(play.prepared.is_some() && play.flat_refusal.is_empty());
}
#[test]
fn actual_flat_installer_retains_partial_issuance_and_never_overwrites_it() {
    let image = source_image();
    let pins = image.identity().unwrap();
    for mutation in 0..5 {
        let mut issuer = issuer();
        let (mut source, mut join) = ready();
        let mut expected = pins.clone();
        match mutation {
            1 => expected.source_document_id = "wrong/source".into(),
            2 => expected.checked_plot_id = "wrong/checked".into(),
            3 => expected.expanded_plot_id = "wrong/expanded".into(),
            4 => issuer.refuse_join_fixture(),
            _ => {}
        }
        let mut play = NativePlay::new();
        let result = play.install_linked_held(
            &image,
            &expected,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
        );
        assert!(play.attempted);
        assert_eq!(play.configuration_revision, 7);
        if mutation == 0 {
            assert!(result.is_ok());
            assert_eq!(play.phase, "installed-held");
            assert!(source.is_none() && join.is_none());
            assert!(play.source.is_some() && play.join.is_some());
            assert!(play.active.is_some());
            assert!(play.flat_refusal.is_empty());
            assert_eq!(
                k::SignSink::len(play.prepared.as_ref().unwrap().kernel.signs()),
                0
            );
        } else {
            assert!(result.is_err());
            assert!(source.is_some() && join.is_some());
            assert!(play.prepared.is_none());
            if mutation == 4 {
                let flat::IssuedCustody::First(first) = &play.flat_refusal else {
                    panic!("actual first bearer must survive Join issuance refusal");
                };
                let retained = first.inspection().clone();
                assert_eq!(
                    retained.scope.active_play_id,
                    play.active.as_ref().unwrap().active_play_id
                );
                let active = play.active.clone();
                let (retry, stats) = allocation_probe::observe(|| {
                    play.install_linked_held(
                        &image,
                        &pins,
                        c::OfferGeneration(u64::MAX),
                        &mut source,
                        &mut join,
                        &mut issuer,
                    )
                });
                assert!(retry.is_err());
                assert_eq!(stats.allocations, 0);
                assert_eq!(play.active, active);
                assert!(source.is_some() && join.is_some());
                let flat::IssuedCustody::First(first) = &play.flat_refusal else {
                    panic!("retry may not overwrite irreversible issuance");
                };
                assert_eq!(first.inspection(), &retained);
            } else {
                assert!(play.active.is_none());
                assert!(play.flat_refusal.is_empty());
            }
        }
    }
}
#[test]
fn actual_whole_constructor_counts_inline_custody_separately_from_heap() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (play, stats) = allocation_probe::observe(|| {
        let (mut source, mut join) = ready();
        let mut play = NativePlay::new();
        play.install_linked_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
        )
        .unwrap();
        assert!(source.is_none() && join.is_none());
        play
    });
    println!(
        "whole actual qualification constructor CPU{} heap {:?}; inline wrapper={} custody={}",
        usize::BITS,
        stats,
        core::mem::size_of::<NativePlay>(),
        core::mem::size_of::<flat::IssuedCustody>()
    );
    assert_eq!(play.phase, "installed-held");
    assert!(play.flat_refusal.is_empty());
    assert_eq!(
        k::SignSink::len(play.prepared.as_ref().unwrap().kernel.signs()),
        0
    );
}

#[test]
fn final_backing_refusal_returns_actual_pair_and_retains_both_issued_bearers() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    flat::refuse_next_backing();
    assert!(play
        .install_linked_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer
        )
        .is_err());
    assert!(source.is_some() && join.is_some());
    assert!(play.prepared.is_none() && play.source.is_none() && play.join.is_none());
    let flat::IssuedCustody::Both(first, second) = &play.flat_refusal else {
        panic!("allocation refusal must retain both actual issued bearers");
    };
    let retained = (first.inspection().clone(), second.inspection().clone());
    let active = play.active.clone().unwrap();
    assert_eq!(retained.0.scope.active_play_id, active.active_play_id);
    assert_eq!(retained.1.scope.active_play_id, active.active_play_id);
    let (retry, allocation) = allocation_probe::observe(|| {
        play.install_linked_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
        )
    });
    assert!(retry.is_err());
    assert_eq!(allocation.allocations, 0);
    let flat::IssuedCustody::Both(first, second) = &play.flat_refusal else {
        panic!("lost custody");
    };
    assert_eq!(
        (first.inspection(), second.inspection()),
        (&retained.0, &retained.1)
    );
}

#[test]
fn reserved_backing_is_same_allocation_and_shared_checks_preserve_custody() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let backing = NativePlay::reserve_linked_backing().unwrap();
    let address = backing.as_ptr() as usize;
    play.install_linked_reserved_held(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
        backing,
    )
    .unwrap();
    assert_eq!(
        play.prepared.as_ref().unwrap().as_ref() as *const _ as usize,
        address
    );
    assert_eq!(play.phase, "installed-held");
    assert!(source.is_none() && join.is_none());

    let mut issuer = self::issuer();
    issuer.refuse_join_fixture();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let backing = NativePlay::reserve_linked_backing().unwrap();
    assert!(play
        .install_linked_reserved_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            backing
        )
        .is_err());
    assert!(source.is_some() && join.is_some());
    assert!(matches!(play.flat_refusal, flat::IssuedCustody::First(_)));
}
#[test]
fn early_backing_allocation_refusal_precedes_any_owner_or_bearer_move() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    flat::refuse_next_backing();
    assert!(NativePlay::reserve_linked_backing().is_err());
    assert!(!play.attempted && play.flat_refusal.is_empty());
    assert!(source.is_some() && join.is_some());
    // The same original owners and fresh issuer still accept normal selection.
    play.install_linked_held(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
    )
    .unwrap();
    assert_eq!(play.phase, "installed-held");
}
#[test]
fn reserved_constructor_measures_changed_overlap_explicitly() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    // Match Root's actual boundary: initialized Ready owners already exist.
    let (mut source, mut join) = ready();
    let ((play, address), stats) = allocation_probe::observe(|| {
        let backing = NativePlay::reserve_linked_backing().unwrap();
        let address = backing.as_ptr() as usize;
        let mut play = NativePlay::new();
        play.install_linked_reserved_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            backing,
        )
        .unwrap();
        (play, address)
    });
    assert_eq!(
        play.prepared.as_ref().unwrap().as_ref() as *const _ as usize,
        address
    );
    assert!(stats.peak_bytes <= 8192 + n::PreparedNetworkKernelStorage::allocation_bytes());
    println!(
        "reserved constructor CPU{} backing={} peak={} retained={}",
        usize::BITS,
        n::PreparedNetworkKernelStorage::allocation_bytes(),
        stats.peak_bytes,
        stats.retained_bytes()
    );
}

#[test]
fn actual_sign_allocation_refusal_preserves_both_ready_owners_and_issued_bearers() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    flat::refuse_next_log();
    assert!(play
        .install_linked_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer
        )
        .is_err());
    assert!(source.is_some() && join.is_some());
    assert!(play.source.is_none() && play.join.is_none() && play.prepared.is_none());
    let flat::IssuedCustody::Both(first, second) = &play.flat_refusal else {
        panic!("actual Sign allocation refusal must retain both issued bearers");
    };
    let retained = (first.inspection().clone(), second.inspection().clone());
    let active = play.active.clone().unwrap();
    assert_eq!(retained.0.scope.active_play_id, active.active_play_id);
    assert_eq!(retained.1.scope.active_play_id, active.active_play_id);
    let (result, memory) = allocation_probe::observe(|| {
        play.install_linked_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
        )
    });
    assert!(result.is_err());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let flat::IssuedCustody::Both(first, second) = &play.flat_refusal else {
        panic!("lost custody");
    };
    assert_eq!(
        (first.inspection(), second.inspection()),
        (&retained.0, &retained.1)
    );
}

#[test]
fn early_sign_storage_refusal_releases_kernel_before_any_owner_moves() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let refusal = allocation_probe::refuse_next_allocation_size(n::CONTROL_SIGN_BACKING_BYTES);
    let (result, memory) = allocation_probe::observe(NativePlay::reserve_linked_backing);
    assert!(result.is_err());
    drop(refusal);
    assert_eq!(memory.allocations, 1);
    assert_eq!(
        memory.deallocated_bytes,
        core::mem::size_of::<PreparedNetworkKernel>()
    );
    assert_eq!(memory.retained_bytes(), 0);
    assert!(!play.attempted && play.flat_refusal.is_empty());
    assert!(source.is_some() && join.is_some());
    let storage = NativePlay::reserve_linked_backing().unwrap();
    play.install_linked_reserved_held(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
        storage,
    )
    .unwrap();
    assert_eq!(
        k::SignSink::len(play.prepared.as_ref().unwrap().kernel.signs()),
        0
    );
}

#[test]
fn phased_storage_refusal_returns_original_owners_before_admission_or_root_mutation() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    for refuse_signs in [false, true] {
        let admitted = core::cell::Cell::new(false);
        if refuse_signs {
            flat::refuse_next_log();
        } else {
            flat::refuse_next_backing();
        }
        assert!(play
            .install_linked_phased_held(
                &image,
                &pins,
                c::OfferGeneration(u64::MAX),
                &mut source,
                &mut join,
                &mut issuer,
                || {
                    admitted.set(true);
                    Ok(())
                }
            )
            .is_err());
        assert!(!admitted.get() && !play.attempted);
        assert!(play.active.is_none() && play.flat_refusal.is_empty());
        assert!(source.is_some() && join.is_some());
        assert!(play.prepared.is_none() && play.source.is_none() && play.join.is_none());
        assert_eq!(play.phase, "uninstalled");
    }
    play.install_linked_phased_held(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
}
#[test]
fn phased_admission_refusal_releases_storage_without_consuming_root_or_owners() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    assert!(play
        .install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || Err(())
        )
        .is_err());
    assert!(!play.attempted && play.active.is_none() && play.flat_refusal.is_empty());
    assert!(source.is_some() && join.is_some());
    play.install_linked_phased_held(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
        || Ok(()),
    )
    .unwrap();
    assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
}
#[test]
fn phased_constructor_counts_complete_retained_proofs_and_both_phase_peaks() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let (play, total) = allocation_probe::observe(|| {
        let mut play = NativePlay::new();
        play.install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || Ok(()),
        )
        .unwrap();
        play
    });
    let phases = flat::phase_measurements();
    let cold = phases[0].observation.unwrap();
    let backed = phases[1].observation.unwrap();
    let before_finish = phases[2].observation.unwrap();
    let finished = phases[3].observation.unwrap();
    assert_eq!(
        phases[1].backing.unwrap(),
        play.prepared.as_ref().unwrap().as_ref() as *const _ as usize
    );
    assert_eq!(
        backed.retained_bytes() - cold.retained_bytes(),
        n::PreparedNetworkKernelStorage::allocation_bytes()
    );
    assert_eq!(before_finish.allocations, finished.allocations);
    assert_eq!(before_finish.reallocations, finished.reallocations);
    println!("phased constructor CPU{} backing={} cold_peak={} complete_proofs={} backed={} issued_peak={} total_peak={} retained={} op_proof_inline={} root_proof_inline={}",
        usize::BITS, n::PreparedNetworkKernelStorage::allocation_bytes(), cold.peak_bytes, cold.retained_bytes(),
        backed.retained_bytes(), before_finish.peak_bytes, total.peak_bytes, total.retained_bytes(),
        core::mem::size_of::<conduit_plan_lowering::selected_operation::PreparedFlatDirectOperation>(),
        core::mem::size_of::<crate::native_authority::LinkedNetworkPreparation<'_>>());
    for (label, index) in [
        ("reserved", 4),
        ("credentials-bound", 5),
        ("credentials-issued", 6),
        ("join-issued", 7),
    ] {
        let snapshot = phases[index].observation.unwrap();
        println!(
            "phase {label}: peak={} retained={}",
            snapshot.peak_bytes,
            snapshot.retained_bytes()
        );
    }
    // The bound is intentionally the existing envelope; exceeding it is a
    // failed capacity proof, never permission to increase the grant or floor.
    assert!(cold.peak_bytes <= 8192);
    assert!(total.peak_bytes <= 8192);
}

#[test]
fn phased_preparation_refuses_changed_source_root_and_owner_boot_before_admission() {
    let image = source_image();
    for mutation in 0..3 {
        let mut pins = image.identity().unwrap();
        let mut issuer = if mutation == 1 {
            RootIssuer::fixture("esp32/b0a732286178".into(), "boot/substituted-root".into())
        } else {
            issuer()
        };
        if mutation == 0 {
            pins.checked_plot_id = "sha256:substituted-source".into();
        }
        let (mut source, mut join) = if mutation == 2 {
            let mut stale = identity("wifi-station");
            stale.boot = "boot/stale-owner".into();
            // SAFETY: an independently initialized transcript owner on a stale
            // Boot, never a physical readiness or Root membership claim.
            let stale = unsafe { ReadyNetworkJoinBase::new(stale, u64::MAX, Inert).unwrap() };
            let (source, _) = ready();
            (source, Some(stale))
        } else {
            ready()
        };
        let mut play = NativePlay::new();
        let admitted = core::cell::Cell::new(false);
        assert!(play
            .install_linked_phased_held(
                &image,
                &pins,
                c::OfferGeneration(u64::MAX),
                &mut source,
                &mut join,
                &mut issuer,
                || {
                    admitted.set(true);
                    Ok(())
                }
            )
            .is_err());
        assert!(!admitted.get() && !play.attempted);
        assert!(play.active.is_none() && play.flat_refusal.is_empty());
        assert!(source.is_some() && join.is_some());
    }
}

#[test]
fn phased_join_binding_refusal_defers_active_copy_and_retains_both_bearers() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    RootIssuer::refuse_next_prepared_join_binding_fixture();
    let (mut play, memory) = allocation_probe::observe(|| {
        let mut play = NativePlay::new();
        assert!(play
            .install_linked_phased_held(
                &image,
                &pins,
                c::OfferGeneration(u64::MAX),
                &mut source,
                &mut join,
                &mut issuer,
                || Ok(())
            )
            .is_err());
        play
    });
    assert!(play.attempted && source.is_some() && join.is_some());
    let active = play.active.as_ref().unwrap();
    let flat::IssuedCustody::FirstLease(first, second) = &play.flat_refusal else {
        panic!("actual post-issuance Join refusal must retain both bearers");
    };
    assert_eq!(
        first.inspection().scope.active_play_id,
        active.active_play_id
    );
    assert_eq!(second.claim.active_play_id, active.active_play_id);
    assert_eq!(
        second
            .table
            .inspections()
            .next()
            .unwrap()
            .scope
            .active_play_id,
        active.active_play_id
    );
    assert_eq!(active.play_sequence, 1);
    let snapshots = flat::phase_measurements();
    assert!(snapshots[8].observation.is_some());
    println!(
        "phased Join refusal CPU{} peak={} retained_after_active_copy={}",
        usize::BITS,
        memory.peak_bytes,
        snapshots[8].observation.unwrap().retained_bytes()
    );
    assert!(memory.peak_bytes <= 8192);
    let (result, retry) = allocation_probe::observe(|| {
        play.install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || Ok(()),
        )
    });
    assert!(result.is_err());
    assert_eq!((retry.allocations, retry.reallocations), (0, 0));
    assert!(matches!(
        play.flat_refusal,
        flat::IssuedCustody::FirstLease(_, _)
    ));
}

#[test]
fn issuance_storage_refusal_is_reversible_and_committed_installation_allocates_nothing() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    let admitted = core::cell::Cell::new(false);
    RootIssuer::refuse_next_issuance_storage_fixture();
    assert!(play
        .install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || {
                admitted.set(true);
                Ok(())
            }
        )
        .is_err());
    assert!(!admitted.get() && !play.attempted);
    assert!(play.active.is_none() && play.flat_refusal.is_empty());
    assert!(source.is_some() && join.is_some());
    assert_eq!(issuer.next_play_sequence(), Some(1));
    let mut before = None;
    let (_, total) = allocation_probe::observe(|| {
        play.install_linked_phased_held(
            &image,
            &pins,
            c::OfferGeneration(u64::MAX),
            &mut source,
            &mut join,
            &mut issuer,
            || {
                before = allocation_probe::current_observation();
                Ok(())
            },
        )
        .unwrap();
    });
    let before = before.unwrap();
    assert_eq!((before.allocations, before.reallocations), (total.allocations, total.reallocations), "actual Root reservation/issuance/binding/materialization after admission must allocate nothing");
    assert_eq!(play.active.as_ref().unwrap().play_sequence, 1);
}

#[test]
fn compact_network_reservation_retains_full_selected_plan_and_root_binding() {
    let image = source_image();
    let pins = image.identity().unwrap();
    let mut issuer = issuer();
    let (mut source, mut join) = ready();
    let mut play = NativePlay::new();
    play.install_linked_phased_held(
        &image,
        &pins,
        c::OfferGeneration(u64::MAX),
        &mut source,
        &mut join,
        &mut issuer,
        || Ok(()),
    )
    .unwrap();
    let active = play.active.as_ref().unwrap();
    assert!(issuer.network_reservation_matches_fixture(active));
    let other_plan = c::PlanId::from("plan/independently-valid-but-unselected");
    let other = c::bind_active_play(
        &other_plan,
        &active.host_id,
        &active.boot_id,
        active.play_sequence,
    );
    assert!(!issuer.network_reservation_matches_fixture(&other));
    for field in 0..4 {
        let mut changed = active.clone();
        match field {
            0 => changed.host_id = "host/substituted".into(),
            1 => changed.boot_id = "boot/substituted".into(),
            2 => changed.play_sequence += 1,
            _ => changed.active_play_id = "active/substituted".into(),
        }
        assert!(!issuer.network_reservation_matches_fixture(&changed));
    }
    assert!(issuer.network_reservation_matches_fixture(active));
}
