//! Actual owner -> emitted LinearFace -> independent observation/proposal proof.
//! Acknowledged Shows remain the existing simulated Mask fixture, not platform proof.
use super::*;
use alloc::{borrow::ToOwned, vec::Vec};
use conduit_face::{
    ChoiceMultiplicity, FaceCollectionRefusal, FacePropertyValue, prepare_face_collection,
    render_linear_face,
};
#[path = "collection_linear_reader.rs"]
mod reader;
use reader::{Availability, Constraint, Records, Value};

fn observe(face: &Face) -> Records {
    Records::read(&render_linear_face(face).unwrap().lines)
}
fn compare(face: &Face, observed: &Records) {
    for suffix in [
        "/reviewed-plots",
        "/name-system/input-details",
        "/initial-plots",
    ] {
        let id = observed.collection(suffix);
        let members = observed.members(id);
        let generic = prepare_face_collection(face, id, 64).unwrap();
        assert_eq!(
            generic
                .items()
                .iter()
                .map(|s| s.identity.as_str())
                .collect::<Vec<_>>(),
            members
        );
        let mode = generic.multiplicity();
        if mode.is_none() {
            assert!(
                !observed
                    .properties
                    .contains_key(&(id.into(), "choice-multiplicity".into()))
            );
        } else {
            assert_eq!(
                observed.value(id, "choice-multiplicity"),
                &Value::Text(
                    match mode.unwrap() {
                        ChoiceMultiplicity::Exclusive => "exclusive",
                        ChoiceMultiplicity::Independent => "independent",
                    }
                    .into()
                )
            );
        }
        for first in 0..=members.len() {
            let page = generic.page(first, 3).unwrap();
            let end = (first + 3).min(members.len());
            assert_eq!(
                page.items
                    .iter()
                    .map(|s| s.identity.as_str())
                    .collect::<Vec<_>>(),
                members[first..end]
            );
            assert_eq!(page.total, members.len());
            assert_eq!(page.next, (end < members.len()).then_some(end));
        }
        // Decode all source properties associated with the collection and items,
        // including the exclusive authoritative value-source.
        let value_source = observed
            .properties
            .get(&(id.into(), "value-source".into()))
            .map(|v| match v {
                Value::Identity(s) => s.as_str(),
                _ => panic!(),
            });
        for p in face.properties.iter().filter(|p| {
            p.subject == id
                || members.contains(&p.subject.as_str())
                || Some(p.subject.as_str()) == value_source
        }) {
            let actual = observed.value(&p.subject, &p.name);
            match (&p.value, actual) {
                (FacePropertyValue::Identity(a), Value::Identity(b))
                | (FacePropertyValue::Text(a), Value::Text(b)) => assert_eq!(a, b),
                (FacePropertyValue::Count(a), Value::Count(b)) => assert_eq!(a, b),
                (FacePropertyValue::Flag(a), Value::Flag(b)) => assert_eq!(a, b),
                (FacePropertyValue::TypedValue { contract, bytes }, Value::Typed(c, b)) => {
                    c.matches(contract);
                    assert_eq!(b, bytes);
                }
                _ => panic!("new source property variant needs independent observation"),
            }
        }
        for subject in face
            .subjects
            .iter()
            .filter(|s| s.identity == id || members.contains(&s.identity.as_str()))
        {
            let role = match subject.role {
                conduit_face::FaceRole::Collection => "Collection",
                conduit_face::FaceRole::Item => "Item",
                conduit_face::FaceRole::Plot => "Plot",
                _ => panic!("new scoped subject role"),
            };
            assert_eq!(
                observed.subjects[&subject.identity],
                (role.into(), subject.name.clone())
            );
        }
        exact_relationships(face, observed, id, &members);
    }
    assert_eq!(observed.actions.len(), face.actions.len());
    for a in &face.actions {
        let emitted = &observed.actions[&a.identity];
        assert_eq!(emitted.intent, a.intent);
        assert_eq!(emitted.target, a.target);
        assert_eq!(emitted.name, a.name);
        assert_eq!(
            emitted.disclosure,
            match a.disclosure {
                conduit_face::FaceDisclosureLevel::Primary => "Primary",
                conduit_face::FaceDisclosureLevel::CurrentAction => "CurrentAction",
                _ => panic!("new Birth action disclosure"),
            }
        );
        assert_eq!(emitted.arguments.len(), a.arguments.len());
        for (x, y) in emitted.arguments.iter().zip(&a.arguments) {
            assert_eq!(x.name, y.name);
            assert_eq!(x.label, y.value_name);
            x.contract.matches(&y.contract);
        }
        let expected = match &a.availability {
            conduit_face::FaceActionAvailability::Available => Availability::Available,
            conduit_face::FaceActionAvailability::Unavailable {
                reason_code,
                explanation,
            } => Availability::Unavailable(reason_code.clone(), explanation.clone()),
            conduit_face::FaceActionAvailability::Refused {
                reason_code,
                explanation,
            } => Availability::Refused(reason_code.clone(), explanation.clone()),
        };
        assert_eq!(emitted.availability, expected);
    }
}
fn source_order(owner: &BirthDraft, records: &Records) {
    inventory_owner(owner, records);
    let names = records.collection("/name-system/input-details");
    assert_eq!(records.count(names, "minimum-selected-count"), 1);
    assert_eq!(records.count(names, "maximum-selected-count"), 1);
    let items = records.members(names);
    assert_eq!(items.len(), owner.naming_systems().count());
    for (item, (id, label)) in items.iter().zip(owner.naming_systems()) {
        assert_eq!(*item, format!("birth-naming/{id}"));
        assert_eq!(records.subjects[*item], ("Item".into(), label.into()));
        assert_eq!(records.typed(item, "choice-value").1, id.as_bytes());
        assert_eq!(
            records.flag(item, "selected"),
            id == owner.requested_system()
        );
    }
    let action = records.identity(names, "selection-action");
    let value_source = records.identity(names, "value-source");
    assert_eq!(
        records.identity(value_source, "value-change-action"),
        action
    );
    assert_eq!(
        records.typed(value_source, "current-value").1,
        owner.requested_system().as_bytes()
    );
    let memberships: Vec<_> = records.actions[action].arguments[0]
        .contract
        .constraints
        .iter()
        .filter_map(|c| match c {
            Constraint::Membership(m, false) => Some(m),
            _ => None,
        })
        .collect();
    let mut expected: Vec<_> = owner
        .naming_systems()
        .map(|(id, _)| id.as_bytes().to_vec())
        .collect();
    expected.sort();
    assert_eq!(memberships, [&expected]);
    let review = records.collection("/reviewed-plots");
    let selected: Vec<_> = owner.choices().iter().filter(|c| c.selected).collect();
    let items = records.members(review);
    assert_eq!(items.len(), selected.len());
    for (item, choice) in items.iter().zip(selected) {
        assert_eq!(*item, format!("{}/review", independent_pair_id(choice)));
        assert_eq!(
            records.subjects[*item],
            ("Item".into(), choice.title.clone())
        );
        assert_eq!(
            records.identity(item, "checked-plot-id"),
            choice.plot.checked_plot_id.as_str()
        );
        assert!(records.actions.values().all(|a| a.target != *item));
    }
}
fn proposal(
    face: &Face,
    show: &MaskShow,
    records: &Records,
    action: &str,
    bytes: &[u8],
) -> Result<FaceInteraction, FaceInteractionRefusal> {
    let action_record = &records.actions[action];
    assert_eq!(action_record.arguments.len(), 1);
    let argument = &action_record.arguments[0];
    FaceInteraction::new(
        face,
        show,
        action,
        &action_record.target,
        vec![FaceInteractionArgument {
            name: argument.name.clone(),
            value_kind: argument.contract.kind.clone(),
            value: bytes.to_vec(),
        }],
        1,
    )
}
#[test]
fn actual_owner_generic_and_independent_linear_preserve_all_three_profiles() {
    let mut owner = draft();
    for query in ["", "Clock", "Speaker", "no-match"] {
        owner.search_plots(owner.revision(), query).unwrap();
        let before = owner.selection(owner.revision()).unwrap();
        let face = owner.host_owned_face(&host_owned_basis()).unwrap();
        let records = observe(&face);
        compare(&face, &records);
        source_order(&owner, &records);
        let inventory = records.collection("/initial-plots");
        assert_eq!(
            records.count(inventory, "inventory-count"),
            owner.choices().len() as u64
        );
        assert_eq!(
            records.count(inventory, "selected-count"),
            owner.choices().iter().filter(|c| c.selected).count() as u64
        );
        if query == "no-match" {
            assert!(records.members(inventory).is_empty());
            assert_eq!(
                records.members(records.collection("/reviewed-plots")).len(),
                1
            );
        }
        assert_eq!(owner.selection(owner.revision()).unwrap(), before);
    }
}
#[test]
fn linear_derived_exclusive_and_independent_values_commit_once_at_current_owner() {
    let mut owner = draft();
    let basis = host_owned_basis();
    let face = owner.host_owned_face(&basis).unwrap();
    let records = observe(&face);
    let show = fixtures::acknowledged_fixture_show(&face);
    let names = records.collection("/name-system/input-details");
    let item = records
        .members(names)
        .into_iter()
        .find(|i| !records.flag(i, "selected"))
        .unwrap();
    let bytes = records.typed(item, "choice-value").1;
    let before = owner.requested_system().to_owned();
    let input = proposal(
        &face,
        &show,
        &records,
        records.identity(names, "selection-action"),
        bytes,
    )
    .unwrap();
    assert_eq!(owner.requested_system(), before);
    assert_eq!(
        owner.apply_host_owned_face_interaction(&basis, &show, &input),
        Ok(BirthActionOutcome::Changed)
    );
    assert_eq!(owner.requested_system().as_bytes(), bytes);
    let revision = owner.revision();
    assert!(
        owner
            .apply_host_owned_face_interaction(&basis, &show, &input)
            .is_err()
    );
    assert_eq!(owner.revision(), revision);
    for selected in [false, true] {
        let face = owner.host_owned_face(&basis).unwrap();
        let records = observe(&face);
        let show = fixtures::acknowledged_fixture_show(&face);
        let inventory = records.collection("/initial-plots");
        let items = records.members(inventory);
        let item = items
            .iter()
            .find(|i| {
                records.actions[records.identity(i, "boolean-change-action")].availability
                    == Availability::Available
            })
            .unwrap();
        let action = records.identity(item, "boolean-change-action");
        let before = owner.selection(owner.revision()).unwrap();
        let input = proposal(&face, &show, &records, action, &[u8::from(selected)]).unwrap();
        assert_eq!(owner.selection(owner.revision()).unwrap(), before);
        disabled_owner_refusal(&mut owner, &basis, &face, &show, &records, &input);
        owner
            .apply_host_owned_face_interaction(&basis, &show, &input)
            .unwrap();
        let current = owner.host_owned_face(&basis).unwrap();
        let observed = observe(&current);
        assert_eq!(observed.flag(item, "selected"), selected);
        let after = owner.selection(owner.revision()).unwrap();
        assert!(
            owner
                .apply_host_owned_face_interaction(&basis, &show, &input)
                .is_err()
        );
        assert_eq!(owner.selection(owner.revision()).unwrap(), after);
        let disabled = items
            .iter()
            .find(|i| {
                records.actions[records.identity(i, "boolean-change-action")].availability
                    != Availability::Available
            })
            .unwrap();
        assert_eq!(
            proposal(
                &face,
                &show,
                &records,
                records.identity(disabled, "boolean-change-action"),
                &[1]
            )
            .unwrap_err(),
            FaceInteractionRefusal::UnavailableAction
        );
        assert_eq!(owner.selection(owner.revision()).unwrap(), after);
    }
    let face = owner.host_owned_face(&basis).unwrap();
    let records = observe(&face);
    let show = fixtures::acknowledged_fixture_show(&face);
    let names = records.collection("/name-system/input-details");
    let before = owner.selection(owner.revision()).unwrap();
    assert_eq!(
        proposal(
            &face,
            &show,
            &records,
            records.identity(names, "selection-action"),
            b"Surprise me"
        )
        .unwrap_err(),
        FaceInteractionRefusal::ViolatedConstraint
    );
    let review = records.collection("/reviewed-plots");
    let prepared = prepare_face_collection(&face, review, 64).unwrap();
    let id = prepared.items()[0].identity.clone();
    assert_eq!(
        prepared.interaction(&face, &show, &id, true, 1),
        Err(FaceCollectionRefusal::NoSelectionAction)
    );
    assert_eq!(owner.selection(owner.revision()).unwrap(), before);
}

#[path = "collection_linear_variants.rs"]
mod variants;

#[path = "collection_linear_controls.rs"]
mod controls;
use controls::{disabled_owner_refusal, exact_relationships, independent_pair_id, inventory_owner};
