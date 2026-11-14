//! Signed member + exact three-Line native return. Shows are explicit CPU
//! contract fixtures; this test makes no physical scanout/input claim.
use super::*;

#[test]
fn admitted_native_body_navigation_at_rest_preserves_body_and_refuses_stale_scope() {
    let (mut owner, root) = owner_fixture();
    let (credential, key, ad) = native_member(&mut owner, &root);
    let (challenge, _) = begin(&mut owner, &credential, &ad);
    owner
        .complete_current_member_carrier(&challenge, signature(&challenge, &key))
        .unwrap();
    owner
        .renew_current_member_presence(&challenge.session_id, 1)
        .unwrap();
    let member = crate::durable_host::owner::MemberPresentationBasis {
        credential: credential.clone(),
        host_advertisement: ad,
    };
    let (face_line, show_line, interaction_line) =
        crate::durable_host::routed_join::native_lines::offer_graphical(
            owner.host.advertisement(),
            &member,
            challenge.session_id.as_str(),
        )
        .unwrap();
    let request = request(&credential);
    let original_truth = owner.truth();
    let mut prior = None;
    for (index, intent, context) in [
        (1, "conduit.intent/open-plot-library@1", "library"),
        (2, "conduit.intent/open-body-overview@1", "overview"),
    ] {
        let seal = owner
            .select_member_carrier_mask_route_with_interaction(
                &challenge.session_id,
                &face_line,
                &show_line,
                interaction_line.as_ref(),
            )
            .unwrap();
        let face = owner.local_face_snapshot().unwrap();
        let offered = face
            .actions
            .iter()
            .find(|action| action.intent == intent)
            .unwrap();
        assert!(offered.availability.is_available());
        let shown = show(&mut owner, &seal, index);
        owner
            .acknowledge_member_carrier_show(&challenge.session_id, &request, &shown)
            .unwrap();
        let interaction = conduit_face::FaceInteraction::new(
            &face,
            &shown,
            &offered.identity,
            &offered.target,
            vec![],
            index,
        )
        .unwrap();
        owner
            .validate_member_carrier_interaction(
                &challenge.session_id,
                &request,
                &shown,
                &interaction,
            )
            .unwrap();

        let unchanged = owner.local_face_snapshot().unwrap();
        let mut foreign_request = request.clone();
        foreign_request.body_id =
            serde_json::from_value(serde_json::json!("f".repeat(64))).unwrap();
        assert!(owner
            .validate_member_carrier_interaction(
                &challenge.session_id,
                &foreign_request,
                &shown,
                &interaction
            )
            .is_err());
        let mut wrong_target = interaction.clone();
        wrong_target.target = "body/foreign".into();
        assert!(owner.navigate_owner_context(&shown, &wrong_target).is_err());
        assert_eq!(owner.local_face_snapshot().unwrap(), unchanged);
        if let Some((old_show, old_interaction)) = &prior {
            assert!(owner
                .navigate_owner_context(old_show, old_interaction)
                .is_err());
            assert_eq!(owner.local_face_snapshot().unwrap(), unchanged);
        }

        let receipt = owner
            .navigate_owner_context(&shown, &interaction)
            .unwrap()
            .unwrap();
        assert_eq!(receipt["context"], context);
        let next = owner.local_face_snapshot().unwrap();
        assert!(next.revision > face.revision);
        assert_ne!(next.identity, face.identity);
        assert!(next
            .properties
            .iter()
            .any(|property| property.name == "operator-context"
                && property.value == conduit_face::FacePropertyValue::Text(context.into())));
        assert_eq!(
            owner.truth(),
            original_truth,
            "navigation cannot alter biography/Source/Plan/Play"
        );
        assert!(owner
            .validate_member_carrier_interaction(
                &challenge.session_id,
                &request,
                &shown,
                &interaction
            )
            .is_err());
        assert!(owner.navigate_owner_context(&shown, &interaction).is_err());
        assert_eq!(owner.local_face_snapshot().unwrap(), next);
        prior = Some((shown, interaction));
    }
    std::fs::remove_dir_all(root).unwrap();
}
