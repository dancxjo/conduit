use conduit_core::{
    AttributionStrength, EntityPassport, EventAuthorship, FederationPolicy, Id, KeyProtection,
    MembershipCredential, PassportStatus, PassportStatusObservation, PinnedDescriptor,
    PublicKeyRef, REALM_SCHEMA_VERSION, RealmDescriptor, RealmReason, RoleBinding, SemanticHash,
    WorkloadDelegation, validate_delegation, validate_event_authorship, validate_federation,
    validate_passport, validate_passport_status, validate_realm,
};
use serde_json::Value;

const FIXTURE: &str = include_str!("../../../conformance/c2/realms-passports-v1.json");

const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);
const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 1,
        semantic_hash: SemanticHash::from_bytes([byte; 32]),
    }
}
const KEY: PublicKeyRef<'static> = PublicKeyRef {
    id: Id("fixture/member-key"),
    algorithm: Id("fixture/ed25519"),
    public_key_digest: SemanticHash::from_bytes([1; 32]),
};
const ROOT: PublicKeyRef<'static> = PublicKeyRef {
    id: Id("fixture/root-key"),
    algorithm: Id("fixture/ed25519"),
    public_key_digest: SemanticHash::from_bytes([2; 32]),
};
const ROLE: RoleBinding<'static> = RoleBinding {
    role: pin("fixture/role", 10),
    binding: Id("fixture/role-binding"),
    expires_at_tick: 30,
};

fn realm() -> RealmDescriptor<'static> {
    let mut value = RealmDescriptor {
        schema_version: REALM_SCHEMA_VERSION,
        identity: ZERO,
        id: Id("fixture/realm"),
        genesis_root: ROOT,
        accepted_roots: &[ROOT],
        root_epoch: 1,
        policy: pin("fixture/policy", 3),
        membership_profile: pin("fixture/membership", 4),
        revocation_profile: pin("fixture/revocation", 5),
        event_integrity_profile: pin("fixture/integrity", 6),
        federation_profile: pin("fixture/federation", 7),
        successions: &[],
    };
    let mut scratch = [ZERO; 1];
    value.identity = value.computed_semantic_hash(&mut scratch).unwrap();
    value
}

fn passport(realm: RealmDescriptor<'static>) -> EntityPassport<'static> {
    let credential = MembershipCredential {
        id: Id("fixture/credential"),
        realm: realm.id,
        entity: Id("fixture/member"),
        key: KEY.id,
        issuer_key: ROOT.id,
        issued_at_tick: 1,
        expires_at_tick: 30,
        time_basis: Id("fixture/clock"),
        receipt: SemanticHash::from_bytes([8; 32]),
    };
    let mut value = EntityPassport {
        schema_version: REALM_SCHEMA_VERSION,
        identity: ZERO,
        entity: Id("fixture/member"),
        profile: pin("fixture/member-profile", 9),
        realm: realm.id,
        credential,
        keys: &[KEY],
        roles: &[ROLE],
        key_protection: KeyProtection::ExportableSoftware,
        sensitivity: conduit_core::Sensitivity::Restricted,
        extensions: &[],
    };
    let mut scratch = [ZERO; 3];
    value.identity = value.computed_semantic_hash(&mut scratch).unwrap();
    value
}

#[test]
fn realm_passport_status_and_delegation_are_explicit_and_bounded() {
    let realm = realm();
    let passport = passport(realm);
    let mut realm_scratch = [ZERO; 1];
    let mut passport_scratch = [ZERO; 3];
    assert_eq!(validate_realm(&realm, &mut realm_scratch), Ok(()));
    assert_eq!(
        validate_passport(&passport, &realm, &mut passport_scratch),
        Ok(())
    );
    let status = PassportStatusObservation {
        passport: passport.identity,
        realm: realm.id,
        entity: passport.entity,
        reporter: pin("fixture/status", 11),
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 20,
        status: PassportStatus::Active,
    };
    assert_eq!(
        validate_passport_status(
            status,
            passport.identity,
            realm.id,
            passport.entity,
            Id("fixture/clock"),
            15
        ),
        Ok(())
    );
    assert_eq!(
        validate_passport_status(
            status,
            passport.identity,
            realm.id,
            passport.entity,
            Id("fixture/clock"),
            20
        ),
        Err(RealmReason::StatusUnavailable)
    );
    let delegation = WorkloadDelegation {
        id: Id("fixture/delegation"),
        realm: realm.id,
        entity: passport.entity,
        passport: passport.identity,
        plan: SemanticHash::from_bytes([12; 32]),
        run: Id("fixture/run"),
        epoch: 7,
        audience: Id("fixture/audience"),
        expires_at_tick: 20,
        depth: 0,
        receipt: SemanticHash::from_bytes([13; 32]),
    };
    assert_eq!(
        validate_delegation(
            delegation,
            passport.identity,
            realm.id,
            passport.entity,
            Id("fixture/run"),
            7,
            15
        ),
        Ok(())
    );
    assert_eq!(
        validate_delegation(
            delegation,
            passport.identity,
            realm.id,
            passport.entity,
            Id("fixture/run"),
            8,
            15
        ),
        Err(RealmReason::DelegationDenied)
    );
    let authorship = EventAuthorship {
        realm: realm.id,
        entity: passport.entity,
        key: KEY.id,
        credential: passport.credential.id,
        delegation: None,
        strength: AttributionStrength::DirectSignature,
        receipt: SemanticHash::from_bytes([14; 32]),
        bridge: Some(Id("fixture/bridge")),
        status,
    };
    assert_eq!(
        validate_event_authorship(authorship, &passport, &realm, Id("fixture/clock"), 15),
        Ok(())
    );
    let stream = pin("fixture/stream", 15);
    let federation = FederationPolicy {
        id: Id("fixture/federation-policy"),
        local_realm: realm.id,
        remote_realm: Id("fixture/remote-realm"),
        local_root_epoch: 1,
        remote_root_epoch: 4,
        time_basis: Id("fixture/clock"),
        expires_at_tick: 20,
        allow_identity: true,
        allow_event_verification: true,
        allow_transport_admission: true,
        allow_grant_delegation: false,
        allowed_streams: &[stream],
        receipt: SemanticHash::from_bytes([16; 32]),
    };
    assert_eq!(
        validate_federation(
            federation,
            realm.id,
            Id("fixture/remote-realm"),
            stream,
            Id("fixture/clock"),
            15,
            false
        ),
        Ok(())
    );
    assert_eq!(
        validate_federation(
            federation,
            Id("fixture/remote-realm"),
            realm.id,
            stream,
            Id("fixture/clock"),
            15,
            false
        ),
        Err(RealmReason::FederationDenied)
    );
}

#[test]
fn realm_fixture_names_identity_freshness_and_federation_boundaries() {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture["suite"], "conduit.realms-passports/v1");
    let ids = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    for required in [
        "entity-identity-survives-key-rotation",
        "replacement-part-has-new-identity-and-old-role",
        "membership-role-and-grant-are-distinct",
        "federation-is-non-transitive",
        "bridge-preserves-remote-authorship",
        "offline-credential-expiry-is-bounded",
        "resolver-never-enrolls-or-prompts",
    ] {
        assert!(ids.contains(&required));
    }
}
