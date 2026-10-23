//! Exercises the actual x86 service store on the host, without privileged entry.
use conduitos::{protected_region, service_memory};
#[path = "../src/arch/x86_64/service_memory/storage.rs"]
mod storage;
use service_memory::ServiceProfile;
use storage::{Lease, View};

fn assert_zero(view: &View<'_>) {
    for table in [&*view.pml4, &*view.pdpt, &*view.pd] {
        assert!(table.0.iter().all(|word| *word == 0));
    }
    assert!(
        view.pt
            .iter()
            .all(|table| table.0.iter().all(|word| *word == 0))
    );
    for bytes in [
        view.image.as_ref(),
        view.exchange.as_ref(),
        view.output.as_ref(),
        view.stack.as_ref(),
        view.metadata.as_ref(),
        view.pixels.as_ref(),
        view.scratch.as_ref(),
        view.trap.as_ref(),
    ] {
        assert!(bytes.iter().all(|byte| *byte == 0));
    }
}
fn dirty(view: &mut View<'_>) {
    view.pml4.0.fill(u64::MAX);
    view.pdpt.0.fill(u64::MAX);
    view.pd.0.fill(u64::MAX);
    for table in &mut *view.pt {
        table.0.fill(u64::MAX);
    }
    for bytes in [
        &mut *view.image,
        &mut *view.exchange,
        &mut *view.output,
        &mut *view.stack,
        &mut *view.metadata,
        &mut *view.pixels,
        &mut *view.scratch,
        &mut *view.trap,
    ] {
        bytes.fill(0xa5);
    }
}
fn fail_after_partial_preparation(
    profile: ServiceProfile,
) -> Result<(), protected_region::DomainRefusal> {
    let mut lease = Lease::claim(profile).expect("partial preparation must acquire its slot");
    dirty(&mut lease.view());
    Err(protected_region::DomainRefusal::InvalidMemory)
}
#[test]
fn zero_store_survives_failure_clear_mutation_and_duplicate_claim() {
    let profile = ServiceProfile::Compositor;
    let mut owner = Lease::claim(profile).expect("first claim");
    assert_eq!(owner.profile(), profile);
    assert!(u64::from(storage::reserved(profile)) >= profile.layout().mapped_bytes());
    assert_eq!(
        owner.output().len() as u64,
        profile.layout().output().bytes()
    );
    assert_eq!(owner.trap_top() % 4096, 0);
    assert_zero(&owner.view());
    dirty(&mut owner.view());
    assert!(Lease::claim(profile).is_err());
    // A failed duplicate must never release or clear the live owner's store.
    assert!(Lease::claim(profile).is_err());
    assert_eq!(owner.view().exchange[0], 0xa5);
    owner.clear();
    assert!(Lease::claim(profile).is_err());
    assert_zero(&owner.view());
    // Obtaining a mutable view after clear must invalidate the clean marker.
    dirty(&mut owner.view());
    drop(owner);
    for _ in 0..3 {
        let mut next = Lease::claim(profile).expect("reclaim after drop");
        assert_zero(&next.view());
        drop(next);
        assert!(fail_after_partial_preparation(profile).is_err());
    }
    let mut final_owner = Lease::claim(profile).expect("reclaim after partial failure");
    assert_zero(&final_owner.view());
}
