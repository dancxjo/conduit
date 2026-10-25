//! CPU lifecycle/Line and admission metadata fixtures; no live authorization or transport.
use super::*;
use crate::native_face_mask::history_tests::setup_with_look;
use crate::native_face_scene::raster_layout_tests::Pixels;
use alloc::boxed::Box;
use conduit_body::{MembershipCredential, SPAWN_ADMISSION_RECEIPT_SCHEMA};
use conduit_face::*;

fn receipt(
    route: &RemoteOwnerMaskRouteSeal,
    provider: &crate::native_surface_provider::NativeSurfaceProvider,
) -> PortableAdmissionReceipt {
    PortableAdmissionReceipt {
        schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential: MembershipCredential {
            credential_id: serde_json::from_str("\"credential/history-refresh-cpu\"").unwrap(),
            body_id: route.body_id.clone(),
            part_id: serde_json::from_str("\"part/history-refresh-cpu\"").unwrap(),
            host_id: route.mask_host.host_id.clone(),
            boot_id: route.mask_host.boot_id.clone(),
            issued_at_millis: 1,
        },
        host_advertisement: crate::product_bases::native_mask_host_advertisement(
            &route.mask_host.host_id,
            &route.mask_host.boot_id,
            route.mask_host.offer_generation.0,
            provider,
        ),
        membership_admitted: true,
        current_offers_available: true,
        plan_created: false,
        play_created: false,
    }
}
fn grant(receipt: &PortableAdmissionReceipt) -> NativeOwnerReturnGrant {
    let c = &receipt.credential;
    NativeOwnerReturnGrant {
        schema: "conduit.body/native-owner-return-grant@1".into(),
        token: [9; 32],
        credential_id: c.credential_id.as_str().into(),
        body_id: c.body_id.as_str().into(),
        part_id: c.part_id.as_str().into(),
        host_id: c.host_id.as_str().into(),
        boot_id: c.boot_id.as_str().into(),
        remaining_millis: 60_000,
        maximum_actions: 4,
    }
}
fn snapshot(face: &Face, route: &RemoteOwnerMaskRouteSeal) -> OwnerFaceSnapshotResponse {
    OwnerFaceSnapshotResponse::Snapshot {
        schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
        face: Box::new(face.clone()),
        interactions_admitted: true,
        route: Some(Box::new(route.clone())),
    }
}
fn response(
    face: &Face,
    route: &RemoteOwnerMaskRouteSeal,
    receipt: &PortableAdmissionReceipt,
) -> RefreshResponse {
    RefreshResponse {
        schema: REFRESH_RESPONSE_SCHEMA.into(),
        face: snapshot(face, route),
        grant: Some(grant(receipt)),
    }
}

#[test]
fn complete_history_look_survives_bounded_selection_and_ordinary_refresh() {
    let (mut mask, face, default_route, provider) = setup_with_look(None);
    let receipt = receipt(&default_route, &provider);
    let prior =
        NativeGuestFace::from_owner_response(&receipt, snapshot(&face, &default_route)).unwrap();
    let base = mask
        .component_configuration()
        .unwrap()
        .requested_look()
        .unwrap();
    mask.present(face.clone(), 1, 1, &mut Pixels::new(640, 480))
        .unwrap();
    let show = mask.show().unwrap();
    for unit in [
        LookTemperatureUnit::Celsius,
        LookTemperatureUnit::Kelvin,
        LookTemperatureUnit::Fahrenheit,
    ] {
        let look = LookInfo::new(
            base.generative().clone(),
            base.identity().into(),
            LookQuantityFacetOption::some(unit).unwrap(),
            base.spoken().clone(),
            3,
            base.visual().clone(),
        )
        .unwrap();
        let (_, selected_face, selected_route, _) = setup_with_look(Some(&look));
        assert_eq!(selected_face, face);
        let bytes = look.clone().encode().unwrap();
        let mut storage = vec![0; MAX_RETURN_ACTION_BYTES];
        let length = serde_json_core::to_slice(
            &HistorySelection {
                schema: "conduit.body/native-owner-history-selection@1",
                token: [9; 32],
                sequence: 0,
                request: owner_request(&receipt),
                show,
                look: &bytes,
            },
            &mut storage,
        )
        .unwrap();
        assert!(length <= 64 * 1024);
        let wire: serde_json::Value = serde_json::from_slice(&storage[..length]).unwrap();
        let transported: Vec<u8> = serde_json::from_value(wire["look"].clone()).unwrap();
        assert_eq!(transported, bytes);
        assert_eq!(LookInfo::decode(&transported).unwrap(), look);
        let (selected, _) = validated_history_selection(
            &receipt,
            &prior,
            response(&face, &selected_route, &receipt),
            &look,
        )
        .unwrap();
        assert_eq!(selected.history_look().unwrap(), Some(look.clone()));
        let (refreshed, _) = validated_refresh(
            &receipt,
            &selected,
            response(&face, &selected_route, &receipt),
        )
        .unwrap();
        assert_eq!(refreshed.face(), &face);
        assert_eq!(refreshed.history_look().unwrap(), Some(look.clone()));
        assert!(
            validated_history_selection(
                &receipt,
                &prior,
                response(&face, &selected_route, &receipt),
                &base
            )
            .is_err()
        );
        assert!(
            validated_refresh(&receipt, &prior, response(&face, &selected_route, &receipt))
                .is_err()
        );
        let mut changed = face.clone();
        changed.revision += 1;
        changed.reseal_prepared().unwrap();
        assert!(
            validated_history_selection(
                &receipt,
                &prior,
                response(&changed, &selected_route, &receipt),
                &look
            )
            .is_err()
        );
        let mut foreign = response(&face, &selected_route, &receipt);
        foreign.grant.as_mut().unwrap().boot_id = "boot/foreign".into();
        assert!(validated_history_selection(&receipt, &prior, foreign, &look).is_err());
        assert_eq!(prior.face(), &face);
        assert_eq!(prior.history_look().unwrap(), None);
        assert_eq!(selected.history_look().unwrap(), Some(look));
    }
}
