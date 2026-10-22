//! Canonical protocol allocation in the real bitmap grant; CPU Show fixture,
//! not an acknowledgement of native/QEMU/physical effects or universal fit.
use super::*;
use conduit_face::{
    Face, FaceBasis, FaceContributionBasis, FaceInteraction, FaceInteractionArgument, FaceRole,
    FaceSubject,
};
use conduitos_application_core::data::{CanonicalRefusal, Proposal};

// This shared fixture is owned and formatted by the Face crate (edition 2021).
#[rustfmt::skip]
#[path = "../../../../../semantics/face/tests/common/mod.rs"]
mod mask_fixture;

pub(super) fn run(applications: &[Option<Initial>; 4], transfer: &mut [u8]) {
    STAGE.store(50, Ordering::Relaxed);
    let mut state = State::default();
    initialize::initialize(&mut state, applications, transfer);
    assert!(matches!(
        exchange(
            &mut state,
            &Command::PatchbayCanonical { slot: 0 },
            transfer
        ),
        Reply::Prepared
    ));
    let Some(Initial::Patchbay { targets, selected }) = &applications[0] else {
        panic!("Patchbay fixture");
    };
    let target = targets[usize::from(*selected)].as_ref().unwrap();
    let (body, basis) = on_caller(|| {
        let Some(Initial::Tutorial { evidence }) = &applications[1] else {
            panic!("Body fixture");
        };
        (
            evidence.body_id.clone(),
            FaceContributionBasis {
                checked_plot_id: format!("checked/{}", "a".repeat(64)).into(),
                plan_id: target.body.clone(),
                active_play_id: format!("play/{}", "b".repeat(64)).into(),
                required_interaction_context: None,
            },
        )
    });
    let query = on_caller(|| Command::PatchbayFragment {
        slot: 0,
        body: body.clone(),
        basis: basis.clone(),
        actions_admitted: true,
    });
    STAGE.store(51, Ordering::Relaxed);
    let Reply::PatchbayFragment {
        revision, fragment, ..
    } = exchange(&mut state, &query, transfer)
    else {
        panic!("canonical actual retained owner projection");
    };
    let (face, show, encoded_input) = on_caller(|| {
        let mut subjects = fragment.subjects;
        subjects.extend([
            FaceSubject {
                identity: format!("body/{}", body.as_str()),
                role: FaceRole::Body,
                name: "Owner".into(),
            },
            FaceSubject {
                identity: format!("plot/{}", target.graph.checked_plot_id.as_str()),
                role: FaceRole::Plot,
                name: "Selected Plot".into(),
            },
        ]);
        let face = Face::new_with_semantics_and_temporal(
            u64::from(revision),
            FaceBasis {
                body_id: Some(body.clone()),
                wake_id: None,
                source_document_id: Some("source/heap-canonical-owner".into()),
                checked_plot_id: Some(basis.checked_plot_id.clone()),
                expanded_plot_id: Some("expanded/heap-canonical-owner".into()),
                plan_id: Some(basis.plan_id.clone()),
                active_play_id: Some(basis.active_play_id.clone()),
                sign_ids: vec![],
            },
            subjects,
            fragment.relationships,
            fragment.properties,
            fragment.text,
            fragment.actions,
            fragment.disclosures,
            fragment.temporal_references,
            fragment.temporal_facts,
        )
        .unwrap()
        .with_composition(fragment.composition)
        .unwrap();
        let show = mask_fixture::available_mask_show(&face);
        let action = face
            .actions
            .iter()
            .find(|action| action.identity == patchbay_application::EDIT_CURRENT_ACTION_ID)
            .unwrap();
        let subject = target.graph.subject_identities().next().unwrap();
        let input = FaceInteraction::new(
            &face,
            &show,
            &action.identity,
            &action.target,
            vec![FaceInteractionArgument {
                name: "subject".into(),
                value_kind: action.arguments[0].contract.value_kind.as_str().into(),
                value: subject.as_bytes().to_vec(),
            }],
            1,
        )
        .unwrap();
        (face, show, input.encode())
    });
    let publish = on_caller(|| Command::PatchbayPublish {
        slot: 0,
        body,
        basis,
        face,
        show,
    });
    STAGE.store(52, Ordering::Relaxed);
    assert!(
        matches!(exchange(&mut state, &publish, transfer), Reply::PatchbayPublished { revision: current, .. } if current == revision)
    );
    let consume = on_caller(|| Command::PatchbayConsume {
        slot: 0,
        input: encoded_input,
    });
    STAGE.store(53, Ordering::Relaxed);
    assert!(
        matches!(exchange(&mut state, &consume, transfer), Reply::PatchbayConsumed {
        revision: current, proposal: Some(Proposal::EditCurrent(_)), refusal: None, .. } if current == revision + 1)
    );
    STAGE.store(54, Ordering::Relaxed);
    assert!(matches!(
        exchange(&mut state, &consume, transfer),
        Reply::PatchbayRefused {
            refusal: CanonicalRefusal::NoPublication,
            ..
        }
    ));
    assert!(matches!(
        exchange(&mut state, &publish, transfer),
        Reply::PatchbayRefused {
            refusal: CanonicalRefusal::ShowConsumed,
            ..
        }
    ));
    report(
        "canonical protocol rounded-live peak=",
        PEAK.load(Ordering::Relaxed),
    );
    drop(state);
    assert_eq!(LIVE.load(Ordering::Relaxed), 0);
}
