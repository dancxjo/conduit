use super::*;
use conduit_core::{BootId, HostId, OfferGeneration};

fn retained() -> MemberBootstrap<'static> {
    let credential: MembershipCredential = serde_json::from_str(
        r#"{
        "credential_id":"credential/1","body_id":"body/1","part_id":"part/1",
        "host_id":"host/1","boot_id":"boot/old","issued_at_millis":7
    }"#,
    )
    .unwrap();
    MemberBootstrap {
        schema: MEMBER_BOOTSTRAP_SCHEMA.into(),
        body_id: credential.body_id.clone(),
        host_id: "host/1".into(),
        image_id: "image/new".into(),
        configuration_id: "configuration/new".into(),
        owner: FederationPeer {
            host_id: HostId::from("owner/1"),
            boot_id: BootId::from("owner/boot/2"),
            offer_generation: OfferGeneration(1),
            verifying_key: [9; 32],
        },
        admission: MemberBootstrapMaterial::Retained {
            credential,
            secret: [3; 32],
        },
    }
}
#[test]
fn retained_bootstrap_roundtrips_without_invitation_or_current_boot_authority() {
    let seed = retained();
    assert!(seed.matches_native("host/1", "image/new", "configuration/new"));
    let bytes = serde_json::to_vec(&seed).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json["admission"].get("retained").is_some());
    assert!(json["admission"]["retained"].get("provision").is_none());
    let decoded: MemberBootstrap<'_> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded.admission.secret(), &[3; 32]);
    assert_eq!(
        decoded.admission.credential().unwrap().part_id.as_str(),
        "part/1"
    );
    assert_eq!(
        decoded.admission.credential().unwrap().boot_id.as_str(),
        "boot/old"
    );
    assert!(decoded.admission.spawn().is_none());
}
#[test]
fn native_scope_and_invalid_material_refuse_without_a_fresh_admission_fallback() {
    let seed = retained();
    assert!(!seed.matches_native("host/other", "image/new", "configuration/new"));
    assert!(!seed.matches_native("host/1", "image/old", "configuration/new"));
    assert!(!seed.matches_native("host/1", "image/new", "configuration/old"));
    let mut seed = seed;
    seed.owner.offer_generation = OfferGeneration(0);
    assert!(!seed.matches_native("host/1", "image/new", "configuration/new"));
    seed.owner.offer_generation = OfferGeneration(1);
    seed.body_id = BodyId::bound("body/other".into());
    assert!(!seed.matches_native("host/1", "image/new", "configuration/new"));
    seed.body_id = BodyId::bound("body/1".into());
    if let MemberBootstrapMaterial::Retained { secret, .. } = &mut seed.admission {
        *secret = [0; 32];
    }
    assert!(!seed.matches_native("host/1", "image/new", "configuration/new"));
}
#[test]
fn mixed_and_unknown_entrances_are_rejected() {
    let mut value = serde_json::to_value(retained()).unwrap();
    value["admission"]["retained"]["provision"] = serde_json::json!({});
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(serde_json::from_slice::<MemberBootstrap<'_>>(&bytes).is_err());
    let mut value = serde_json::to_value(retained()).unwrap();
    value["admission"] = serde_json::json!({"automatic-new-part":{}});
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(serde_json::from_slice::<MemberBootstrap<'_>>(&bytes).is_err());
}
