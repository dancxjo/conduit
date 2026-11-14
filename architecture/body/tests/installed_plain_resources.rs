#![cfg(feature = "authenticated-admission")]
//! Signed table resolution is preparation data, never an admission grant.
use conduit_body::*;
use conduit_core as c;
#[path = "common/admission_allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOCATOR: probe::Allocator = probe::Allocator;
#[path = "installed_plain_resources/support.rs"]
mod support;
use support::*;
#[test]
fn authenticated_pinned_receiver_borrows_exact_installed_rows_then_original_owner_admits() {
    let f = actual();
    let p = projection();
    let installed = table(&[row()]);
    let reference = installed.reference();
    let document = document(&p, &reference);
    let attributed = command(&f, &document);
    let (received, cost) = probe::observe(|| {
        receive_delegated_body_fragment_with_installed_plain_resources::<0>(
            Some(&policy()),
            f.context(),
            attributed,
            profile(installed),
        )
    });
    let (fragment, fore) = received.unwrap();
    assert!(fore.as_slice().is_empty());
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert_eq!(fragment.metadata().schema, 4);
    assert!(core::ptr::eq(
        fragment.metadata().resource_bindings,
        installed.canonical_bytes()
    ));
    assert_eq!(fragment.projection(), p);
    assert!(
        encode_body_fragment_document(fragment.metadata(), &p, 4096).is_err(),
        "resolved logical metadata cannot masquerade as signed wire metadata"
    );
    let mut owner = c::ResourceAdmissionOwner::new(f.host.clone());
    assert!(owner.admissions().is_empty());
    let observations = observations(&f);
    assert!(
        delegated_plain_resource_preparation_bytes(&[&fragment], &owner, &observations).is_ok()
    );
    assert_eq!(
        admit_delegated_plain_resources(&[&fragment], &mut owner, &observations)
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn every_reference_component_wrong_installation_and_default_receivers_refuse() {
    let f = actual();
    let p = projection();
    let installed = table(&[row()]);
    let reference = installed.reference();
    for at in [8, 39, 40, 44] {
        let mut changed = reference;
        changed[at] = changed[at].wrapping_add(1);
        let document = document(&p, &changed);
        assert!(
            receive_delegated_body_fragment_with_installed_plain_resources::<0>(
                Some(&policy()),
                f.context(),
                command(&f, &document),
                profile(installed)
            )
            .is_err()
        );
    }
    let document = document(&p, &reference);
    let mut other = row();
    other.pool = "pool/other";
    let other = table(&[other]);
    assert!(
        receive_delegated_body_fragment_with_installed_plain_resources::<0>(
            Some(&policy()),
            f.context(),
            command(&f, &document),
            profile(other)
        )
        .is_err()
    );
    assert!(receive_delegated_body_fragment(
        Some(&policy()),
        f.context(),
        command(&f, &document),
        profile(installed).preparation
    )
    .is_err());
    assert!(
        receive_delegated_body_fragment_with_installed_plain_resources::<0>(
            None,
            f.context(),
            command(&f, &document),
            profile(installed)
        )
        .is_err()
    );
    let mut stale = f.context();
    stale.now_millis = 30;
    assert!(
        receive_delegated_body_fragment_with_installed_plain_resources::<0>(
            Some(&policy()),
            stale,
            command(&f, &document),
            profile(installed)
        )
        .is_err()
    );
    let mut unpinned = profile(installed);
    unpinned.preparation.fore = None;
    assert!(
        receive_delegated_body_fragment_with_installed_plain_resources::<0>(
            Some(&policy()),
            f.context(),
            command(&f, &document),
            unpinned
        )
        .is_err()
    );
}
#[test]
fn installed_rows_must_match_original_projection_and_actual_pools_not_just_digest() {
    let f = actual();
    let p = projection();
    for field in 0..4 {
        let mut changed = row();
        match field {
            0 => changed.node = 1,
            1 => changed.resource = 1,
            2 => changed.units = 2,
            _ => {}
        }
        let rows = if field == 3 {
            vec![changed, changed]
        } else {
            vec![changed]
        };
        let installed = table(&rows);
        let reference = installed.reference();
        let document = document(&p, &reference);
        assert!(
            receive_delegated_body_fragment_with_installed_plain_resources::<0>(
                Some(&policy()),
                f.context(),
                command(&f, &document),
                profile(installed)
            )
            .is_err()
        );
    }
    let mut changed = row();
    changed.pool = "pool/not-installed";
    let installed = table(&[changed]);
    let reference = installed.reference();
    let document = document(&p, &reference);
    let (fragment, _) = receive_delegated_body_fragment_with_installed_plain_resources::<0>(
        Some(&policy()),
        f.context(),
        command(&f, &document),
        profile(installed),
    )
    .unwrap();
    let mut owner = c::ResourceAdmissionOwner::new(f.host.clone());
    assert!(admit_delegated_plain_resources(&[&fragment], &mut owner, &observations(&f)).is_err());
    assert!(owner.admissions().is_empty());
}
#[test]
fn original_whole_set_attribution_is_rechecked_without_child_authority() {
    let f = actual();
    let p = projection();
    let installed = table(&[row()]);
    let reference = installed.reference();
    let document = document(&p, &reference);
    let bytes = encode_body_workset_document(&[&document], 4096).unwrap();
    let policy = policy();
    let set = receive_delegated_body_workset::<1>(
        Some(&policy),
        f.context(),
        workset_command(&f, &bytes),
    )
    .unwrap();
    let ((fragment, _), cost) = probe::observe(|| {
        set.receive_fragment_with_installed_plain_resources::<0>(
            0,
            Some(&policy),
            f.context(),
            profile(installed),
        )
        .unwrap()
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(core::ptr::eq(
        fragment.metadata().resource_bindings,
        installed.canonical_bytes()
    ));
    assert!(set
        .receive_fragment_with_pinned_fore::<0>(
            0,
            Some(&policy),
            f.context(),
            profile(installed).preparation
        )
        .is_err());
    let mut stale = f.context();
    stale.now_millis = 30;
    assert!(set
        .receive_fragment_with_installed_plain_resources::<0>(
            0,
            Some(&policy),
            stale,
            profile(installed)
        )
        .is_err());
}
#[test]
fn installed_canonical_rows_reject_malformed_noncanonical_zero_units_and_unrepresentable_counts() {
    let canonical = bytes(&[row()]);
    let (result, cost) = probe::observe(|| plain_resource_table_reference(&canonical));
    assert!(result.is_ok());
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    let mut zero = row();
    zero.units = 0;
    assert!(plain_resource_table_reference(&postcard::to_allocvec(&zero).unwrap()).is_err());
    let mut noncanonical = canonical.clone();
    noncanonical.splice(0..1, [0x80, 0]);
    assert!(plain_resource_table_reference(&noncanonical).is_err());
    for invalid in [&canonical[..canonical.len() - 1], &[255, 255, 255][..]] {
        assert!(plain_resource_table_reference(invalid).is_err());
    }
    let mut many = Vec::new();
    for _ in 0..=(c::MAXIMUM_RESOURCE_ADMISSIONS * c::MAXIMUM_BINDINGS_PER_ADMISSION) {
        row().append_to(&mut many).unwrap();
    }
    assert!(plain_resource_table_reference(&many).is_err());
}
