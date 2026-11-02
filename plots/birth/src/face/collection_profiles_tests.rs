//! Actual Birth producer and owner; Show acknowledgement is the existing fixture.
use super::*;
use alloc::{borrow::ToOwned, vec::Vec};
use conduit_face::{
    ChoiceMultiplicity, FaceCollectionRefusal, FacePropertyValue as V, prepare_face_collection,
};

pub(super) fn collection<'a>(face: &'a Face, suffix: &str) -> &'a conduit_face::FaceSubject {
    face.subjects
        .iter()
        .find(|s| s.role == FaceRole::Collection && s.identity.ends_with(suffix))
        .expect("actual source collection")
}
pub(super) fn property<'a>(face: &'a Face, subject: &str, name: &str) -> &'a V {
    let mut values = face
        .properties
        .iter()
        .filter(|p| p.subject == subject && p.name == name);
    let value = &values.next().expect("source fact").value;
    assert!(values.next().is_none());
    value
}
#[test]
fn actual_birth_face_prepares_all_three_source_profiles_without_mutation() {
    let owner = draft();
    let before = owner.selection(owner.revision()).unwrap();
    let face = owner.host_owned_face(&host_owned_basis()).unwrap();
    let show = fixtures::acknowledged_fixture_show(&face);
    for (suffix, mode) in [
        ("/reviewed-plots", None),
        (
            "/name-system/input-details",
            Some(ChoiceMultiplicity::Exclusive),
        ),
        ("/initial-plots", Some(ChoiceMultiplicity::Independent)),
    ] {
        let subject = collection(&face, suffix);
        let prepared = prepare_face_collection(&face, &subject.identity, 64).unwrap();
        assert_eq!(prepared.multiplicity(), mode);
        assert!(core::ptr::eq(prepared.source(), &face));
        let page = prepared.page(0, 1).unwrap();
        assert_eq!(page.face_identity, face.identity.as_str());
        assert_eq!(page.revision, face.revision);
        assert_eq!(page.total, prepared.items().len());
        if mode.is_none() {
            assert_eq!(prepared.items().len(), 1);
            assert_eq!(prepared.items()[0].name, "Clock");
            assert_eq!(
                prepared.interaction(&face, &show, &prepared.items()[0].identity, true, 1),
                Err(FaceCollectionRefusal::NoSelectionAction)
            );
        }
    }
    assert_eq!(owner.selection(owner.revision()).unwrap(), before);
}
#[test]
fn empty_query_retains_true_inventory_and_readonly_selected_review() {
    let mut owner = draft();
    let before = owner.selection(owner.revision()).unwrap().workset;
    owner.search_plots(owner.revision(), "no-match").unwrap();
    let face = owner.face(&basis()).unwrap();
    let inventory = collection(&face, "/initial-plots");
    let prepared = prepare_face_collection(&face, &inventory.identity, 64).unwrap();
    assert!(prepared.items().is_empty());
    assert_eq!(
        prepared.multiplicity(),
        Some(ChoiceMultiplicity::Independent)
    );
    assert_eq!(
        property(&face, &inventory.identity, "inventory-count"),
        &V::Count(2)
    );
    assert_eq!(
        property(&face, &inventory.identity, "selected-count"),
        &V::Count(1)
    );
    assert_eq!(
        property(&face, &inventory.identity, "query-source"),
        &V::Identity(format!("birth/{}/plot-search", basis().encounter_id))
    );
    let review = collection(&face, "/reviewed-plots");
    let reviewed = prepare_face_collection(&face, &review.identity, 64).unwrap();
    assert_eq!(reviewed.items()[0].name, "Clock");
    assert_eq!(owner.selection(owner.revision()).unwrap().workset, before);
    apply(
        &mut owner,
        "creche.search",
        Some((UTF8_TEXT_VALUE_KIND, b"")),
    )
    .unwrap();
    apply(&mut owner, "creche.plot.0", Some(("value/bool", &[0]))).unwrap();
    let face = owner.face(&basis()).unwrap();
    let review = collection(&face, "/reviewed-plots");
    assert!(
        prepare_face_collection(&face, &review.identity, 64)
            .unwrap()
            .items()
            .is_empty()
    );
}
#[test]
fn exclusive_naming_and_reversible_plot_choices_commit_only_at_current_owner() {
    let mut owner = draft();
    let basis = host_owned_basis();
    let original_name = owner.requested_system().to_owned();
    let face = owner.host_owned_face(&basis).unwrap();
    let names = collection(&face, "/name-system/input-details");
    let prepared = prepare_face_collection(&face, &names.identity, 64).unwrap();
    let alternative = prepared
        .items()
        .iter()
        .find(|s| property(&face, &s.identity, "selected") == &V::Flag(false))
        .unwrap();
    let V::TypedValue { bytes, .. } = property(&face, &alternative.identity, "choice-value") else {
        panic!()
    };
    let expected = core::str::from_utf8(bytes).unwrap().to_owned();
    let show = fixtures::acknowledged_fixture_show(&face);
    let proposal = prepared
        .interaction(&face, &show, &alternative.identity, true, 1)
        .unwrap();
    assert_eq!(proposal.action_id, "creche.naming");
    assert_eq!(owner.requested_system(), original_name);
    assert_eq!(
        prepared.interaction(&face, &show, &alternative.identity, false, 1),
        Err(FaceCollectionRefusal::Selection)
    );
    owner
        .apply_host_owned_face_interaction(&basis, &show, &proposal)
        .unwrap();
    assert_eq!(owner.requested_system(), expected);
    let revision = owner.revision();
    assert!(
        owner
            .apply_host_owned_face_interaction(&basis, &show, &proposal)
            .is_err()
    );
    assert_eq!(owner.revision(), revision);
    for chosen in [false, true] {
        let face = owner.host_owned_face(&basis).unwrap();
        let subject = collection(&face, "/initial-plots");
        let prepared = prepare_face_collection(&face, &subject.identity, 64).unwrap();
        let show = fixtures::acknowledged_fixture_show(&face);
        let before = owner.choices()[0].selected;
        let proposal = prepared
            .interaction(&face, &show, &prepared.items()[0].identity, chosen, 1)
            .unwrap();
        assert_eq!(owner.choices()[0].selected, before);
        owner
            .apply_host_owned_face_interaction(&basis, &show, &proposal)
            .unwrap();
        assert_eq!(owner.choices()[0].selected, chosen);
        let selected: Vec<_> = owner.choices().iter().map(|c| c.selected).collect();
        let revision = owner.revision();
        assert!(
            owner
                .apply_host_owned_face_interaction(&basis, &show, &proposal)
                .is_err()
        );
        assert_eq!(owner.revision(), revision);
        assert_eq!(
            owner
                .choices()
                .iter()
                .map(|c| c.selected)
                .collect::<Vec<_>>(),
            selected
        );
    }
}
#[test]
fn complete_actual_profiles_fit_explicit_collection_storage_not_implicit_standard() {
    use crate::BirthPlotChoice;
    use conduit_body::{MAX_BODY_PLOTS, ResidentPlot};
    extern crate std;
    for count in [1, MAX_BODY_PLOTS] {
        let choices = (0..count)
            .map(|index| BirthPlotChoice {
                title: "界".repeat(21),
                search_text: "source".into(),
                plot: ResidentPlot::new(
                    format!("{index:064x}").into(),
                    format!("{index:064x}").into(),
                ),
                refusal: None,
                selected: true,
            })
            .collect();
        let mut owner =
            BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
        owner.edit_name(owner.revision(), "é".repeat(32)).unwrap();
        let mut basis = host_owned_basis();
        basis.encounter_id = "e".repeat(128);
        let face = owner.host_owned_face(&basis).unwrap();
        for suffix in [
            "/reviewed-plots",
            "/name-system/input-details",
            "/initial-plots",
        ] {
            prepare_face_collection(&face, &collection(&face, suffix).identity, 64).unwrap();
        }
        let bytes = serde_json::to_vec(&face).unwrap();
        if count == 1 {
            assert!(bytes.len() <= conduit_conduitos_mask_offer::MASK_BYTES);
        } else {
            assert!(bytes.len() > conduit_conduitos_mask_offer::MASK_BYTES);
        }
        std::println!(
            "actual-profile count={count} naming_options={} bytes={}",
            owner.naming_systems().count(),
            bytes.len()
        );
        assert!(
            bytes.len()
                <= conduit_conduitos_mask_offer::NativeMaskStorageProfile::Collection.bytes()
                    as usize,
            "collection profiles count={count} bytes={}",
            bytes.len()
        );
    }
}

// Measurement control only; does not assert that every valid Face fits a native grant.
#[test]
fn snapshot_diagnostic_for_original_and_recovered_producer() {
    extern crate std;
    use crate::BirthPlotChoice;
    use conduit_body::ResidentPlot;
    for count in [1, 16] {
        let choices = (0..count)
            .map(|index| BirthPlotChoice {
                title: "界".repeat(21),
                search_text: "source".into(),
                plot: ResidentPlot::new(
                    format!("{index:064x}").into(),
                    format!("{index:064x}").into(),
                ),
                refusal: None,
                selected: true,
            })
            .collect();
        let mut owner =
            BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
        owner.edit_name(owner.revision(), "é".repeat(32)).unwrap();
        let mut basis = host_owned_basis();
        basis.encounter_id = "e".repeat(128);
        let face = owner.host_owned_face(&basis).unwrap();
        std::println!(
            "snapshot-diagnostic count={count} names={} bytes={} properties={} subjects={}",
            owner.naming_systems().count(),
            serde_json::to_vec(&face).unwrap().len(),
            face.properties.len(),
            face.subjects.len()
        );
    }
}
