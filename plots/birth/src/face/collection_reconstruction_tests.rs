//! Independent Source/generic/linear collection correspondence, not owner effects.
use super::*;
use conduit_face::{FaceCollectionRefusal, prepare_face_collection};
use sha2::{Digest, Sha256};
#[path = "linear_collection_records.rs"]
mod records;
use records::Records;
#[path = "linear_collection_commit_tests.rs"]
mod commits;
#[path = "generic_collection_observation.rs"]
mod generic_observation;
#[path = "linear_collection_pair_identity_tests.rs"]
mod pair_identity;

fn independent_pair_digest(plot: &conduit_body::ResidentPlot) -> String {
    let mut framed = Vec::new();
    for value in [
        "conduit.birth/plot-choice@1",
        plot.source_document_id.as_str(),
        plot.checked_plot_id.as_str(),
    ] {
        framed.extend_from_slice(&u32::try_from(value.len()).unwrap().to_le_bytes());
        framed.extend_from_slice(value.as_bytes());
    }
    format!("{:x}", Sha256::digest(&framed))
}

fn rehash(face: Face) -> Face {
    Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap()
}
fn account(face: &Face) -> Records {
    Records::read(&render_linear_face(face).unwrap().lines)
}
fn compare_generic(face: &Face, linear: &Records, id: &str) {
    let generic = prepare_face_collection(face, id, 64).unwrap();
    assert_eq!(
        generic
            .items()
            .iter()
            .map(|s| s.identity.clone())
            .collect::<Vec<_>>(),
        linear.items(id)
    );
    let mode = generic.multiplicity().map(|m| match m {
        conduit_face::ChoiceMultiplicity::Exclusive => "exclusive",
        conduit_face::ChoiceMultiplicity::Independent => "independent",
    });
    assert_eq!(
        mode,
        linear
            .properties
            .get(&(id.into(), "choice-multiplicity".into()))
            .map(|s| s.strip_prefix("text:").map(records::string).unwrap())
            .as_deref()
    );
    assert_eq!(
        linear.count(id, "item-count") as usize,
        generic.items().len()
    );
    assert_eq!(generic.subject().identity, id);
    assert_eq!(generic.source(), face);
    for first in 0..=generic.items().len() {
        let page = generic.page(first, 3).unwrap();
        let end = (first + 3).min(generic.items().len());
        assert_eq!(page.collection_identity, id);
        assert_eq!(page.total, linear.items(id).len());
        assert_eq!(page.next, (end < page.total).then_some(end));
        assert_eq!(
            page.items
                .iter()
                .map(|s| s.identity.clone())
                .collect::<Vec<_>>(),
            linear.items(id)[first..end]
        );
    }
    let show = fixtures::acknowledged_fixture_show(face);
    for item in generic.items() {
        let proposal = generic.interaction(face, &show, &item.identity, true, 1);
        match generic.multiplicity() {
            None => assert_eq!(
                proposal.err(),
                Some(FaceCollectionRefusal::NoSelectionAction)
            ),
            Some(mode) => {
                let action = match mode {
                    conduit_face::ChoiceMultiplicity::Exclusive => {
                        linear.identity(id, "selection-action")
                    }
                    conduit_face::ChoiceMultiplicity::Independent => {
                        linear.identity(&item.identity, "boolean-change-action")
                    }
                };
                if linear.actions[&action]["availability"] != "available" {
                    assert!(proposal.is_err());
                    continue;
                }
                let proposal = proposal.unwrap();
                assert_eq!(proposal.action_id, action);
                assert_eq!(
                    proposal.target,
                    records::string(&linear.actions[&action]["target"])
                );
                assert_eq!(proposal.arguments.len(), 1);
                assert_eq!(proposal.arguments[0].name, "value");
                let (kind, bytes) = match mode {
                    conduit_face::ChoiceMultiplicity::Exclusive => {
                        ("value/text", linear.typed(&item.identity, "choice-value").1)
                    }
                    conduit_face::ChoiceMultiplicity::Independent => ("value/bool", vec![1]),
                };
                assert_eq!(proposal.arguments[0].value_kind, kind);
                assert_eq!(proposal.arguments[0].value, bytes);
            }
        }
    }
    for p in &face.properties {
        if p.subject != id && !linear.items(id).contains(&p.subject) {
            continue;
        }
        let expected = match &p.value {
            FacePropertyValue::Count(v) => format!("count:{v}"),
            FacePropertyValue::Flag(v) => format!("flag:{v}"),
            FacePropertyValue::Identity(v) => format!("identity:{v:?}"),
            FacePropertyValue::Text(v) => format!("text:{v:?}"),
            FacePropertyValue::TypedValue { contract, bytes } => {
                let (observed_contract, observed_bytes) = linear.typed(&p.subject, &p.name);
                assert_eq!(observed_contract, format!("{contract:?}"));
                assert_eq!(&observed_bytes, bytes);
                continue;
            }
            FacePropertyValue::ValueContract(v) => format!("value-contract:{v:?}"),
            _ => panic!("unexpected collection profile property"),
        };
        assert_eq!(linear.property(&p.subject, &p.name), expected);
    }
}
fn verify_owner(draft: &BirthDraft, face: &Face) -> Records {
    let linear = account(face);
    let naming = collection(face, "/name-system/input-details")
        .identity
        .clone();
    let inventory = collection(face, "/initial-plots").identity.clone();
    let review = collection(face, "/reviewed-plots").identity.clone();
    for id in [&naming, &inventory, &review] {
        compare_generic(face, &linear, id);
    }
    let expected_naming: Vec<_> = draft
        .naming_systems()
        .map(|(id, _)| {
            (
                format!("{naming}/{:x}", Sha256::digest(id.as_bytes())),
                id.as_bytes().to_vec(),
            )
        })
        .collect();
    assert_eq!(
        linear.items(&naming),
        expected_naming
            .iter()
            .map(|p| p.0.clone())
            .collect::<Vec<_>>()
    );
    let action_id = linear.identity(&naming, "selection-action");
    assert_eq!(action_id, "creche.naming");
    let action = &linear.actions[&action_id];
    assert_eq!(
        records::string(&action["target"]),
        linear.identity(&naming, "value-source")
    );
    assert_eq!(action["availability"], "available");
    assert_eq!(linear.count(&naming, "minimum-selected-count"), 1);
    assert_eq!(linear.count(&naming, "maximum-selected-count"), 1);
    let members = records::membership(&action["arguments"]);
    let mut expected_members: Vec<_> = expected_naming.iter().map(|p| p.1.clone()).collect();
    expected_members.sort();
    assert!(!members.is_empty());
    assert_eq!(members, expected_members);
    let current = linear
        .typed(&linear.identity(&naming, "value-source"), "current-value")
        .1;
    assert_eq!(current, draft.requested_system().as_bytes());
    for (id, value) in expected_naming {
        let (contract, actual) = linear.typed(&id, "choice-value");
        assert_eq!(actual, value);
        assert!(contract.contains("value/text") && contract.contains("constraints: []"));
        assert_eq!(linear.flag(&id, "selected"), value == current);
    }
    let expected_plot_ids: Vec<_> = draft
        .choices()
        .iter()
        .map(|c| format!("{inventory}/{}", independent_pair_digest(&c.plot)))
        .collect();
    assert_eq!(linear.items(&inventory), expected_plot_ids);
    let mut selected = Vec::new();
    for (index, (id, choice)) in expected_plot_ids.iter().zip(draft.choices()).enumerate() {
        assert_eq!(
            linear.identity(id, "checked-plot-id"),
            choice.plot.checked_plot_id.as_str()
        );
        assert_eq!(linear.flag(id, "selected"), choice.selected);
        let action_id = linear.identity(id, "boolean-change-action");
        assert_eq!(action_id, format!("creche.plot.{index}"));
        let observed = &linear.actions[&action_id];
        assert_eq!(records::string(&observed["target"]), *id);
        assert!(
            observed["arguments"].contains("name: \"value\"")
                && observed["arguments"].contains("value/bool")
        );
        if let Some(reason) = &choice.refusal {
            assert!(observed["availability"].starts_with("unavailable code="));
            assert!(observed["availability"].contains(&format!("explanation={reason:?}")));
        } else {
            assert_eq!(observed["availability"], "available");
        }
        if choice.selected {
            selected.push(format!(
                "{review}/{}",
                independent_pair_digest(&choice.plot)
            ));
        }
    }
    assert_eq!(linear.items(&review), selected);
    assert_eq!(
        linear.count(&inventory, "selected-count") as usize,
        selected.len()
    );
    assert!(
        linear
            .actions
            .values()
            .all(|a| records::string(&a["target"]) != review)
    );
    // Independently decoded action identity/target, full argument contract and
    // terminal availability wording must survive, including disabled actions.
    for action in &face.actions {
        let observed = &linear.actions[&action.identity];
        assert_eq!(records::string(&observed["intent"]), action.intent);
        assert_eq!(records::string(&observed["target"]), action.target);
        assert_eq!(observed["arguments"], format!("{:?}", action.arguments));
        let expected_availability = match &action.availability {
            conduit_face::FaceActionAvailability::Available => "available".into(),
            conduit_face::FaceActionAvailability::Unavailable {
                reason_code,
                explanation,
            } => format!("unavailable code={reason_code:?} explanation={explanation:?}"),
            conduit_face::FaceActionAvailability::Refused {
                reason_code,
                explanation,
            } => format!("refused code={reason_code:?} explanation={explanation:?}"),
        };
        assert_eq!(observed["availability"], expected_availability);
        assert_eq!(records::string(&observed["name"]), action.name);
    }
    linear
}
#[test]
fn owner_generic_and_actual_linear_reconstruct_all_three_collection_profiles() {
    extern crate std;
    let owner = draft();
    let specimen = owner.face(&basis()).unwrap();
    let reconstructed = verify_owner(&owner, &specimen);
    // Explicit test output permits retained independent inspection; no ambient
    // production export hook, installed owner or effect receipt is involved.
    std::println!(
        "COLLECTION_SOURCE_JSON {}",
        serde_json::to_string(&specimen).unwrap()
    );
    std::println!(
        "COLLECTION_LINEAR_JSON {}",
        serde_json::to_string(&render_linear_face(&specimen).unwrap().lines).unwrap()
    );
    std::println!("COLLECTION_RECONSTRUCTED {:?}", reconstructed);
    std::println!(
        "COLLECTION_GENERIC_JSON {}",
        generic_observation::observe(&specimen)
    );
    let choices = owner
        .choices()
        .iter()
        .cloned()
        .map(|mut c| {
            c.selected = false;
            c
        })
        .collect();
    let empty = BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
    let face = empty.face(&basis()).unwrap();
    let linear = verify_owner(&empty, &face);
    let review = collection(&face, "/reviewed-plots");
    assert!(linear.items(&review.identity).is_empty());
}
#[test]
fn duplicate_unicode_labels_and_reordered_storage_do_not_replace_stable_identity_or_order() {
    let owner = draft();
    let face = owner.face(&basis()).unwrap();
    let mut altered = face.clone();
    for subject in &mut altered.subjects {
        subject.name = format!("{}🌻", "界".repeat(20));
    }
    altered.subjects.reverse();
    altered.relationships.reverse();
    altered.properties.reverse();
    let altered = rehash(altered);
    let a = verify_owner(&owner, &face);
    let b = verify_owner(&owner, &altered);
    assert_eq!(a.properties, b.properties);
    assert_eq!(a.actions, b.actions);
    for suffix in [
        "/initial-plots",
        "/reviewed-plots",
        "/name-system/input-details",
    ] {
        let id = &collection(&face, suffix).identity;
        assert_eq!(a.items(id), b.items(id));
    }
    let id = collection(&face, "/name-system/input-details")
        .identity
        .clone();
    let original = a.items(&id);
    let mut reordered = face;
    for property in &mut reordered.properties {
        if property.name == "order" && original.contains(&property.subject) {
            let FacePropertyValue::Count(order) = &mut property.value else {
                panic!()
            };
            *order = original.len() as u64 - 1 - *order;
        }
    }
    let reordered = rehash(reordered);
    let linear = account(&reordered);
    compare_generic(&reordered, &linear, &id);
    assert_eq!(
        linear.items(&id),
        original.into_iter().rev().collect::<Vec<_>>()
    );
}
#[test]
fn missing_unselected_member_remains_visible_against_full_action_membership() {
    let owner = draft();
    let mut face = owner.face(&basis()).unwrap();
    let id = collection(&face, "/name-system/input-details")
        .identity
        .clone();
    let original = account(&face);
    let missing = original
        .items(&id)
        .into_iter()
        .find(|s| !original.flag(s, "selected"))
        .unwrap();
    face.relationships.retain(|r| {
        !(r.source == id && r.kind == FaceRelationshipKind::Contains && r.target == missing)
    });
    for p in &mut face.properties {
        if p.subject == id && p.name == "item-count" {
            let FacePropertyValue::Count(n) = &mut p.value else {
                panic!()
            };
            *n -= 1;
        }
    }
    let face = rehash(face);
    let linear = account(&face);
    let action = linear.identity(&id, "selection-action");
    let members = records::membership(&linear.actions[&action]["arguments"]);
    assert_eq!(members.len(), owner.naming_systems().count());
    assert_eq!(linear.items(&id).len() + 1, members.len());
    assert!(!linear.items(&id).contains(&missing));
    assert_eq!(
        prepare_face_collection(&face, &id, 64).err(),
        Some(FaceCollectionRefusal::Membership)
    );
}
