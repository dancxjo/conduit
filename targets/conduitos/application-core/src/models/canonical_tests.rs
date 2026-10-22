//! Actual retained actor query and semantic refusal, not an acknowledged Mask proof.
use super::*;
use crate::data::{Initial, Workspace, decode, encode};

fn models() -> Models {
    #[derive(serde::Deserialize)]
    enum Fixture {
        Init {
            applications: [Option<Initial>; crate::SLOTS],
        },
    }
    let Fixture::Init { applications } =
        decode(include_bytes!("../../tests/fixtures/startup.catz")).unwrap();
    Models::new(applications).unwrap()
}
fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "checked/native-patchbay-owner".into(),
        plan_id: "plan/native-body".into(),
        active_play_id: "play/native-body".into(),
        required_interaction_context: None,
    }
}
#[test]
fn canonical_query_serializes_actual_fragment_and_refusal_preserves_actor_revision() {
    let mut models = models();
    let first = models
        .patchbay_fragment(0, "body/native".into(), basis(), true)
        .unwrap();
    let (revision, expected) = match &first {
        Reply::PatchbayFragment {
            revision, fragment, ..
        } => (*revision, fragment.clone()),
        _ => panic!("actual retained Patchbay projects canonical fragment"),
    };
    expected.validate_bounds().unwrap();
    let mut workspace = Workspace::new().unwrap();
    let mut output = alloc::vec::Vec::new();
    output.reserve_exact(crate::MAX_BLOB);
    workspace.encode_reply_into(&first, &mut output).unwrap();
    let returned = crate::data::decode_reply(&output).unwrap();
    assert_eq!(encode(&first).unwrap(), encode(&returned).unwrap());
    assert!(matches!(
        models.patchbay_consume(0, b"malformed").unwrap(),
        Reply::PatchbayRefused {
            refusal: CanonicalRefusal::NoPublication,
            ..
        }
    ));
    match models
        .patchbay_fragment(0, "body/native".into(), basis(), true)
        .unwrap()
    {
        Reply::PatchbayFragment {
            revision: after,
            fragment,
            ..
        } => {
            assert_eq!(revision, after);
            assert_eq!(expected, fragment);
        }
        _ => panic!("semantic refusal must retain the existing actor"),
    }
}
#[test]
fn canonical_refusal_keeps_interaction_and_actor_domain_categories_typed() {
    let interaction = conduit_face::FaceInteractionRefusal::StaleShow;
    assert_eq!(
        map_refusal(PatchbayCommandRefusal::InvalidInteraction(interaction)),
        CanonicalRefusal::InvalidInteraction(interaction)
    );
    assert_ne!(
        map_refusal(PatchbayCommandRefusal::Domain(
            PatchbayApplicationRefusal::RevisionExhausted
        )),
        map_refusal(PatchbayCommandRefusal::Domain(
            PatchbayApplicationRefusal::NoSubject
        ))
    );
}
