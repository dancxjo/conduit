//! Independent owner mapping, complete scoped edges and disabled owner boundary.
extern crate std;
use super::*;

pub(super) fn independent_pair_id(choice: &crate::BirthPlotChoice) -> String {
    let mut framed = Vec::new();
    for value in [
        "conduit.birth/plot-choice@1",
        choice.plot.source_document_id.as_str(),
        choice.plot.checked_plot_id.as_str(),
    ] {
        framed.extend_from_slice(&(value.len() as u32).to_le_bytes());
        framed.extend_from_slice(value.as_bytes());
    }
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(&framed);
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut encoded = String::new();
    for chunk in digest.chunks(3) {
        let a = chunk[0];
        encoded.push(char::from(alphabet[usize::from(a >> 2)]));
        let b = chunk.get(1).copied().unwrap_or(0);
        encoded.push(char::from(alphabet[usize::from((a & 3) << 4 | b >> 4)]));
        if chunk.len() > 1 {
            let c = chunk.get(2).copied().unwrap_or(0);
            encoded.push(char::from(alphabet[usize::from((b & 15) << 2 | c >> 6)]));
            if chunk.len() > 2 {
                encoded.push(char::from(alphabet[usize::from(c & 63)]));
            }
        }
    }
    encoded
}
pub(super) fn inventory_owner(owner: &BirthDraft, records: &Records) {
    let inventory = records.collection("/initial-plots");
    assert_eq!(
        records.subjects[inventory],
        ("Collection".into(), "Plots to include".into())
    );
    let query = owner.search().to_lowercase();
    let expected: Vec<_> = owner
        .choices()
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            let searchable = format!("{} {}", c.title, c.search_text).to_lowercase();
            query
                .split_whitespace()
                .all(|term| searchable.contains(term))
        })
        .collect();
    let items = records.members(inventory);
    assert_eq!(items.len(), expected.len());
    for (item, (index, choice)) in items.iter().zip(expected) {
        assert_eq!(*item, independent_pair_id(choice));
        assert_eq!(
            records.subjects[*item],
            ("Plot".into(), choice.title.clone())
        );
        assert_eq!(records.flag(item, "selected"), choice.selected);
        assert_eq!(
            records.identity(item, "checked-plot-id"),
            choice.plot.checked_plot_id.as_str()
        );
        let id = records.identity(item, "boolean-change-action");
        assert_eq!(id, format!("creche.plot.{index}"));
        let action = &records.actions[id];
        assert_eq!(action.target, *item);
        assert_eq!(action.name, "Include Plot");
        assert_eq!(action.arguments.len(), 1);
        assert_eq!(action.arguments[0].name, "value");
        assert_eq!(action.arguments[0].contract.kind, "value/bool");
        assert_eq!(action.arguments[0].contract.maximum, 1);
        assert_eq!(
            action.availability,
            match &choice.refusal {
                None => Availability::Available,
                Some(detail) =>
                    Availability::Unavailable("birth/unavailable".into(), detail.clone()),
            }
        );
    }
}
pub(super) fn exact_relationships(face: &Face, records: &Records, id: &str, members: &[&str]) {
    let scoped = |source: &str, target: &str| {
        source == id || target == id || members.contains(&source) || members.contains(&target)
    };
    let mut expected: Vec<_> = face
        .relationships
        .iter()
        .filter(|r| scoped(&r.source, &r.target))
        .map(|r| {
            let kind = match &r.kind {
                FaceRelationshipKind::Contains => "Contains",
                FaceRelationshipKind::Connects => "Connects",
                FaceRelationshipKind::Describes => "Describes",
                FaceRelationshipKind::Realizes => "Realizes",
                FaceRelationshipKind::Observes => "Observes",
                FaceRelationshipKind::Semantic(k) if k.as_str() == "document/precedes" => {
                    "Semantic(KindId(\"document/precedes\"))"
                }
                _ => panic!("new scoped semantic relationship needs reader comparison"),
            };
            (kind.to_owned(), r.source.clone(), r.target.clone())
        })
        .collect();
    let mut actual: Vec<_> = records
        .relationships
        .iter()
        .filter(|(_, s, t)| scoped(s, t))
        .cloned()
        .collect();
    expected.sort();
    actual.sort();
    assert_eq!(actual, expected, "complete scoped relationship multiset");
}
pub(super) fn disabled_owner_refusal(
    owner: &mut BirthDraft,
    basis: &HostOwnedBirthFaceBasis,
    face: &Face,
    show: &MaskShow,
    records: &Records,
    valid: &FaceInteraction,
) {
    let inventory = records.collection("/initial-plots");
    let item = records
        .members(inventory)
        .into_iter()
        .find(|i| {
            records.actions[records.identity(i, "boolean-change-action")].availability
                != Availability::Available
        })
        .unwrap();
    let id = records.identity(item, "boolean-change-action");
    assert_eq!(
        proposal(face, show, records, id, &[1]).unwrap_err(),
        FaceInteractionRefusal::UnavailableAction
    );
    let action = &records.actions[id];
    assert_eq!(action.arguments.len(), 1);
    let mut forged = valid.clone();
    forged.action_id = id.into();
    forged.target = action.target.clone();
    forged.arguments = vec![FaceInteractionArgument {
        name: action.arguments[0].name.clone(),
        value_kind: action.arguments[0].contract.kind.clone(),
        value: vec![1],
    }];
    let revision = owner.revision();
    let naming = owner.requested_system().to_owned();
    let selection = owner.selection(revision).unwrap();
    assert_eq!(
        owner.apply_host_owned_face_interaction(basis, show, &forged),
        Err(BirthFaceRefusal::Interaction(
            FaceInteractionRefusal::UnavailableAction
        ))
    );
    assert_eq!(owner.revision(), revision);
    assert_eq!(owner.requested_system(), naming);
    assert_eq!(owner.selection(revision).unwrap(), selection);
}

#[test]
fn complete_scoped_edges_detect_extra_reverse_precedence() {
    let face = draft().host_owned_face(&host_owned_basis()).unwrap();
    let records = observe(&face);
    let id = records.collection("/name-system/input-details").to_owned();
    let members: Vec<_> = records
        .members(&id)
        .into_iter()
        .map(str::to_owned)
        .collect();
    for kind in ["Semantic(KindId(\"document/precedes\"))", "Connects"] {
        let mut lines = render_linear_face(&face).unwrap().lines;
        lines.push(format!(
            "RELATIONSHIP kind={kind} source={:?} target={:?}",
            members[1], members[0]
        ));
        let extra = Records::read(&lines);
        let members: Vec<_> = members.iter().map(String::as_str).collect();
        assert!(
            std::panic::catch_unwind(|| exact_relationships(&face, &extra, &id, &members)).is_err()
        );
    }
}
