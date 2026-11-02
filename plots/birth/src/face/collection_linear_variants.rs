use super::*;
use crate::BirthPlotChoice;
use conduit_body::{MAX_BODY_PLOT_IDENTITY_BYTES, ResidentPlot};

fn rebuild(face: Face) -> Face {
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
#[test]
fn duplicate_unicode_long_pairs_and_reordered_storage_keep_exact_observed_meaning() {
    let checked = "checked/same";
    let choices: Vec<_> = ["α", "β"]
        .into_iter()
        .map(|prefix| BirthPlotChoice {
            title: "同じ · \"é\"\\\n".into(),
            search_text: prefix.into(),
            plot: ResidentPlot::new(
                format!(
                    "{prefix}{}",
                    "s".repeat(MAX_BODY_PLOT_IDENTITY_BYTES / 2 - checked.len() - prefix.len())
                )
                .into(),
                checked.into(),
            ),
            refusal: None,
            selected: true,
        })
        .collect();
    for choices in [choices.clone(), choices.into_iter().rev().collect()] {
        let inventory =
            conduit_body::BodyWorkset::from_plots(choices.iter().map(|choice| choice.plot.clone()))
                .unwrap();
        assert_eq!(inventory.identity_bytes(), MAX_BODY_PLOT_IDENTITY_BYTES);
        let mut owner =
            BirthDraft::new("550e8400-e29b-41d4-a716-446655440000".into(), choices).unwrap();
        let face = owner.host_owned_face(&host_owned_basis()).unwrap();
        let original = observe(&face);
        compare(&face, &original);
        source_order(&owner, &original);
        let review = original.collection("/reviewed-plots");
        let ids = original.members(review);
        assert_ne!(ids[0], ids[1]);
        assert_eq!(original.subjects[ids[0]].1, original.subjects[ids[1]].1);
        let mut reversed = face.clone();
        reversed.subjects.reverse();
        reversed.relationships.reverse();
        reversed.properties.reverse();
        let reversed = rebuild(reversed);
        let observed = observe(&reversed);
        compare(&reversed, &observed);
        source_order(&owner, &observed);
        assert_eq!(original.properties, observed.properties);
        assert_eq!(original.members(review), observed.members(review));
        owner.search_plots(owner.revision(), "absent").unwrap();
        let face = owner.host_owned_face(&host_owned_basis()).unwrap();
        let filtered = observe(&face);
        compare(&face, &filtered);
        source_order(&owner, &filtered);
        assert!(
            filtered
                .members(filtered.collection("/initial-plots"))
                .is_empty()
        );
        assert_eq!(
            filtered
                .members(filtered.collection("/reviewed-plots"))
                .len(),
            2
        );
        owner.select(owner.revision(), 0, false).unwrap();
        owner.select(owner.revision(), 1, false).unwrap();
        let empty = owner.host_owned_face(&host_owned_basis()).unwrap();
        let observed = observe(&empty);
        compare(&empty, &observed);
        source_order(&owner, &observed);
        assert!(
            observed
                .members(observed.collection("/reviewed-plots"))
                .is_empty()
        );
    }
}
#[test]
fn linear_membership_exposes_missing_alternative_and_refuses_forged_owner_effects() {
    let mut owner = draft();
    let basis = host_owned_basis();
    let mut face = owner.host_owned_face(&basis).unwrap();
    let original = observe(&face);
    let names = original.collection("/name-system/input-details");
    let missing = original
        .members(names)
        .into_iter()
        .find(|id| !original.flag(id, "selected"))
        .unwrap()
        .to_owned();
    face.relationships.retain(|r| {
        !(r.source == names && r.target == missing && r.kind == FaceRelationshipKind::Contains)
    });
    let property = face
        .properties
        .iter_mut()
        .find(|p| p.subject == names && p.name == "item-count")
        .unwrap();
    let FacePropertyValue::Count(count) = &mut property.value else {
        panic!()
    };
    *count -= 1;
    let face = rebuild(face);
    let observed = observe(&face);
    let action = observed.identity(names, "selection-action");
    let allowed = observed.actions[action].arguments[0]
        .contract
        .constraints
        .iter()
        .find_map(|c| match c {
            Constraint::Membership(m, false) => Some(m),
            _ => None,
        })
        .unwrap();
    assert_eq!(allowed.len(), observed.members(names).len() + 1);
    assert!(!observed.members(names).contains(&missing.as_str()));
    assert_eq!(
        prepare_face_collection(&face, names, 64).unwrap_err(),
        FaceCollectionRefusal::Membership
    );
    let current = owner.host_owned_face(&basis).unwrap();
    let records = observe(&current);
    let show = fixtures::acknowledged_fixture_show(&current);
    let names = records.collection("/name-system/input-details");
    let bytes = records.typed(records.members(names)[0], "choice-value").1;
    let valid = proposal(
        &current,
        &show,
        &records,
        records.identity(names, "selection-action"),
        bytes,
    )
    .unwrap();
    let before = owner.selection(owner.revision()).unwrap();
    let naming = owner.requested_system().to_owned();
    for mutation in 0..4 {
        let mut input = valid.clone();
        match mutation {
            0 => input.arguments[0].value = b"forged/unknown".to_vec(),
            1 => input.arguments[0].value_kind = "value/bool".into(),
            2 => input.target = "foreign/target".into(),
            _ => input.action_id = "foreign/action".into(),
        };
        assert!(
            owner
                .apply_host_owned_face_interaction(&basis, &show, &input)
                .is_err()
        );
        assert_eq!(owner.selection(owner.revision()).unwrap(), before);
        assert_eq!(owner.requested_system(), naming);
    }
}
