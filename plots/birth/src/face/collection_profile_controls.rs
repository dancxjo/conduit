//! Independent actual-Face observation and current-owner refusal controls.
use super::collection_profiles::{collection, property};
use super::*;
use alloc::vec::Vec;
use conduit_face::{FaceCollectionRefusal, FacePropertyValue as V, prepare_face_collection};

// Read generic graph facts, independently of prepare_face_collection or owner inventory.
fn members<'a>(face: &'a Face, subject: &str) -> Vec<&'a conduit_face::FaceSubject> {
    let mut items: Vec<_> = face
        .relationships
        .iter()
        .filter(|r| r.source == subject && r.kind == FaceRelationshipKind::Contains)
        .map(|r| {
            face.subjects
                .iter()
                .find(|s| s.identity == r.target)
                .unwrap()
        })
        .collect();
    items.sort_by_key(|s| match property(face, &s.identity, "order") {
        V::Count(n) => *n,
        _ => panic!(),
    });
    assert_eq!(
        property(face, subject, "item-count"),
        &V::Count(items.len() as u64)
    );
    items
}
#[test]
fn generic_observation_recovers_all_profiles_and_exact_source_option_values() {
    let owner = draft();
    let face = owner.host_owned_face(&host_owned_basis()).unwrap();
    let before = owner.selection(owner.revision()).unwrap();
    let review = collection(&face, "/reviewed-plots");
    let items = members(&face, &review.identity);
    let expected: Vec<_> = owner.choices().iter().filter(|c| c.selected).collect();
    assert_eq!(items.len(), expected.len());
    for (item, choice) in items.iter().zip(expected) {
        assert_eq!(item.name, choice.title);
        assert_eq!(
            item.identity,
            format!(
                "{}/review",
                crate::plot_choice_identity::plot_choice_identity(&choice.plot)
            )
        );
        assert_eq!(
            property(&face, &item.identity, "checked-plot-id"),
            &V::Identity(choice.plot.checked_plot_id.as_str().into())
        );
        assert!(face.actions.iter().all(|a| a.target != item.identity));
    }
    let names = collection(&face, "/name-system/input-details");
    for key in ["minimum-selected-count", "maximum-selected-count"] {
        assert_eq!(property(&face, &names.identity, key), &V::Count(1));
    }
    let names = members(&face, &names.identity);
    let mut selected = 0;
    assert_eq!(names.len(), owner.naming_systems().count());
    for (item, (id, label)) in names.iter().zip(owner.naming_systems()) {
        assert_eq!(item.name, label);
        let V::TypedValue { contract, bytes } = property(&face, &item.identity, "choice-value")
        else {
            panic!()
        };
        assert_eq!(bytes, id.as_bytes());
        contract.validate(bytes).unwrap();
        let V::Flag(chosen) = property(&face, &item.identity, "selected") else {
            panic!()
        };
        assert_eq!(*chosen, id == owner.requested_system());
        selected += usize::from(*chosen);
        assert!(face.actions.iter().all(|a| a.target != item.identity));
    }
    assert_eq!(selected, 1);
    let inventory = collection(&face, "/initial-plots");
    let observed = members(&face, &inventory.identity);
    assert_eq!(observed.len(), owner.choices().len());
    for (item, choice) in observed.iter().zip(owner.choices()) {
        assert_eq!(
            item.identity,
            crate::plot_choice_identity::plot_choice_identity(&choice.plot)
        );
        assert_eq!(
            property(&face, &item.identity, "selected"),
            &V::Flag(choice.selected)
        );
        let V::Identity(action) = property(&face, &item.identity, "boolean-change-action") else {
            panic!()
        };
        let action = face.actions.iter().find(|a| a.identity == *action).unwrap();
        assert_eq!(action.target, item.identity);
        assert_eq!(action.availability.is_available(), choice.refusal.is_none());
        assert_eq!(action.arguments.len(), 1);
        assert_eq!(action.arguments[0].name, "value");
        assert_eq!(
            action.arguments[0].contract.value_kind.as_str(),
            "value/bool"
        );
        assert_eq!(action.arguments[0].contract.maximum_bytes, 1);
        action.arguments[0].contract.validate(&[0]).unwrap();
        action.arguments[0].contract.validate(&[1]).unwrap();
    }
    assert_eq!(owner.selection(owner.revision()).unwrap(), before);
}
#[test]
fn malformed_exclusive_returns_and_disabled_independent_proposals_have_zero_effects() {
    let mut owner = draft();
    let basis = host_owned_basis();
    let face = owner.host_owned_face(&basis).unwrap();
    let show = fixtures::acknowledged_fixture_show(&face);
    let prepared = prepare_face_collection(
        &face,
        &collection(&face, "/name-system/input-details").identity,
        64,
    )
    .unwrap();
    let valid = prepared
        .interaction(&face, &show, &prepared.items()[0].identity, true, 1)
        .unwrap();
    let before = owner.selection(owner.revision()).unwrap();
    let naming: String = owner.requested_system().into();
    for mutation in 0..6 {
        let mut forged = valid.clone();
        match mutation {
            0 => forged.target = "foreign/target".into(),
            1 => forged.arguments[0].value = b"unknown".to_vec(),
            2 => forged.arguments[0].value_kind = "value/bool".into(),
            3 => forged.arguments.push(forged.arguments[0].clone()),
            4 => forged.arguments.clear(),
            _ => forged.action_id = "foreign/action".into(),
        }
        assert!(
            owner
                .apply_host_owned_face_interaction(&basis, &show, &forged)
                .is_err()
        );
        assert_eq!(owner.selection(owner.revision()).unwrap(), before);
        assert_eq!(owner.requested_system(), naming);
    }
    assert_eq!(
        prepared.interaction(&face, &show, "foreign/item", true, 1),
        Err(FaceCollectionRefusal::UnknownItem)
    );
    let independent =
        prepare_face_collection(&face, &collection(&face, "/initial-plots").identity, 64).unwrap();
    let disabled = independent
        .items()
        .iter()
        .find(|item| item.name == "Speaker")
        .unwrap();
    assert_eq!(
        independent.interaction(&face, &show, &disabled.identity, true, 1),
        Err(FaceCollectionRefusal::Interaction(
            FaceInteractionRefusal::UnavailableAction
        ))
    );
    assert_eq!(owner.selection(owner.revision()).unwrap(), before);
}
#[test]
fn reviewed_pairs_survive_duplicate_labels_reordering_and_filtered_source_loss() {
    use crate::BirthPlotChoice;
    use conduit_body::ResidentPlot;
    let choices: Vec<_> = ["source/one", "source/two"]
        .into_iter()
        .map(|source| BirthPlotChoice {
            title: "同じ label".into(),
            search_text: source.into(),
            plot: ResidentPlot::new(source.into(), "checked/same".into()),
            refusal: None,
            selected: true,
        })
        .collect();
    let mut identities = None;
    for choices in [choices.clone(), choices.into_iter().rev().collect()] {
        let mut owner =
            BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
        let face = owner.face(&basis()).unwrap();
        let review = collection(&face, "/reviewed-plots");
        let items = members(&face, &review.identity);
        assert_ne!(items[0].identity, items[1].identity);
        let mut pairs: Vec<_> = items
            .iter()
            .map(|s| {
                (
                    s.identity.clone(),
                    property(&face, &s.identity, "checked-plot-id").clone(),
                )
            })
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        if let Some(expected) = &identities {
            assert_eq!(&pairs, expected);
        } else {
            identities = Some(pairs);
        }
        for (item, choice) in items.iter().zip(owner.choices()) {
            assert_eq!(
                item.identity,
                format!(
                    "{}/review",
                    crate::plot_choice_identity::plot_choice_identity(&choice.plot)
                )
            );
        }
        owner.search_plots(owner.revision(), "absent").unwrap();
        let filtered = owner.face(&basis()).unwrap();
        assert!(members(&filtered, &collection(&filtered, "/initial-plots").identity).is_empty());
        assert_eq!(
            members(
                &filtered,
                &collection(&filtered, "/reviewed-plots").identity
            )
            .len(),
            2
        );
    }
}

#[test]
fn existing_collection_storage_requires_its_exact_offer_and_preserves_standard_bound() {
    use conduit_conduitos_mask_offer::*;
    use conduit_core::*;
    let provider = BaseProviderEntry {
        base_id: "profiles/surface".into(),
        provider_instance_id: "profiles/provider".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: BaseLifecycle::Ready,
        capabilities: vec![],
        resources: vec![resource_offer(
            "profiles/resource",
            conduit_face::SHOW_RESOURCE_CLASS,
            1,
        )],
    };
    let small = native_host_advertisement(&"host/birth".into(), &"boot/birth".into(), 1, &provider);
    let large = collection_native_host_advertisement(
        &"host/birth".into(),
        &"boot/birth".into(),
        1,
        &provider,
    );
    assert_eq!(NativeMaskStorageProfile::Standard.bytes(), 48 * 1024);
    assert_eq!(NativeMaskStorageProfile::Collection.bytes(), 256 * 1024);
    assert!(prepare_collection_stage_from_offer(&small).is_err());
    assert!(prepare_stage_from_offer(Adapter::Native, &large).is_err());
    for (stage, expected) in [
        (
            prepare_stage_from_offer(Adapter::Native, &small).unwrap(),
            48 * 1024,
        ),
        (
            prepare_collection_stage_from_offer(&large).unwrap(),
            256 * 1024,
        ),
    ] {
        assert!(
            stage
                .planned_mask
                .plan
                .fragments
                .iter()
                .flat_map(|f| &f.fore_ports)
                .any(|p| p.front_port_id.as_str() == "face" && p.byte_capacity == expected)
        );
    }
    // This is offer/planning proof, not display possession or an acknowledged Show.
}

#[test]
fn actual_reading_preserves_naming_source_order_after_storage_reversal() {
    use conduit_face::{FaceUtteranceProvenance, plan_face_utterances};
    let owner = draft();
    let face = owner.host_owned_face(&host_owned_basis()).unwrap();
    let expected: Vec<String> = owner
        .naming_systems()
        .map(|(identity, _)| format!("birth-naming/{identity}"))
        .collect();
    let mut subjects = face.subjects.clone();
    subjects.reverse();
    let reversed = Face::new_with_semantics_and_temporal(
        face.revision,
        face.basis.clone(),
        subjects,
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
        face.actions.clone(),
        face.disclosures.clone(),
        face.temporal_references.clone(),
        face.temporal_facts.clone(),
    )
    .unwrap()
    .with_interaction_context(face.interaction_context.clone())
    .unwrap()
    .with_composition(face.composition.clone())
    .unwrap();
    for source in [&face, &reversed] {
        let actual: Vec<String> = plan_face_utterances(source)
            .unwrap()
            .clauses
            .iter()
            .filter_map(|clause| match &clause.provenance {
                FaceUtteranceProvenance::Subject(subject)
                    if subject.identity().starts_with("birth-naming/") =>
                {
                    Some(subject.identity().clone())
                }
                _ => None,
            })
            .collect();
        assert_eq!(actual, expected);
    }
}
