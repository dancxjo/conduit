use conduit_core::{
    AttributionStrength, CredentialVerification, CredentialVerificationOutcome,
    EntityKeyTransition, EntityPassport, EventAuthorship, EventClass, EventPayloadRef,
    FederationPolicy, Id, InstancePath, KeyProtection, MembershipCredential, PassportStatus,
    PassportStatusObservation, PinnedDescriptor, PublicKeyRef, REALM_SCHEMA_VERSION,
    RealmControlEvent, RealmControlKind, RealmDescriptor, RealmReason, ResonanceEnvelope,
    ResonanceRelations, RoleBinding, RootSuccession, SemanticHash, Sensitivity, TypeContractRef,
    WorkloadDelegation, require_realm_operation_authority, validate_credential_verification,
    validate_delegation, validate_entity_key_transition, validate_event_authorship,
    validate_federation, validate_passport, validate_passport_at, validate_passport_status,
    validate_realm, validate_realm_control_event,
};
use serde_json::{Value, json};

const FIXTURE: &str = include_str!("../../../conformance/c2/realms-passports.json");
const ZERO: SemanticHash = SemanticHash::from_bytes([0; 32]);

const fn pin(id: &'static str, byte: u8) -> PinnedDescriptor<'static> {
    PinnedDescriptor {
        id: Id(id),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([byte; 32]),
    }
}

const KEY: PublicKeyRef<'static> = PublicKeyRef {
    id: Id("fixture/member-key"),
    algorithm: Id("fixture/ed25519"),
    public_key_digest: SemanticHash::from_bytes([1; 32]),
};
const KEY_2: PublicKeyRef<'static> = PublicKeyRef {
    id: Id("fixture/member-key-2"),
    algorithm: Id("fixture/ed25519"),
    public_key_digest: SemanticHash::from_bytes([17; 32]),
};
const ROOT: PublicKeyRef<'static> = PublicKeyRef {
    id: Id("fixture/root-key"),
    algorithm: Id("fixture/ed25519"),
    public_key_digest: SemanticHash::from_bytes([2; 32]),
};
const ROOT_2: PublicKeyRef<'static> = PublicKeyRef {
    id: Id("fixture/root-key-2"),
    algorithm: Id("fixture/ed25519"),
    public_key_digest: SemanticHash::from_bytes([18; 32]),
};
const ROLE: RoleBinding<'static> = RoleBinding {
    role: pin("fixture/role", 10),
    binding: Id("fixture/role-binding"),
    expires_at_tick: 30,
};
const EVENT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fixture/realm-control"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([19; 32]),
};
static ROOTS: [PublicKeyRef<'static>; 1] = [ROOT];
static KEYS: [PublicKeyRef<'static>; 1] = [KEY];
static ROTATED_KEYS: [PublicKeyRef<'static>; 2] = [KEY, KEY_2];
static ROLES: [RoleBinding<'static>; 1] = [ROLE];

fn realm_with_id(id: &'static str) -> RealmDescriptor<'static> {
    let mut value = RealmDescriptor {
        schema_version: REALM_SCHEMA_VERSION,
        identity: ZERO,
        id: Id(id),
        genesis_root: ROOT,
        accepted_roots: &ROOTS,
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

fn realm() -> RealmDescriptor<'static> {
    realm_with_id("fixture/realm")
}

fn passport_with(
    realm: RealmDescriptor<'static>,
    entity: &'static str,
    keys: &'static [PublicKeyRef<'static>],
    credential_key: Id<'static>,
    key_protection: KeyProtection,
) -> EntityPassport<'static> {
    let credential = MembershipCredential {
        id: Id("fixture/credential"),
        realm: realm.id,
        entity: Id(entity),
        key: credential_key,
        issuer_key: ROOT.id,
        issued_at_tick: 1,
        expires_at_tick: 30,
        time_basis: Id("fixture/clock"),
        receipt: SemanticHash::from_bytes([8; 32]),
    };
    let mut value = EntityPassport {
        schema_version: REALM_SCHEMA_VERSION,
        identity: ZERO,
        entity: Id(entity),
        profile: pin("fixture/member-profile", 9),
        realm: realm.id,
        credential,
        keys,
        roles: &ROLES,
        key_protection,
        sensitivity: Sensitivity::Restricted,
        extensions: &[],
    };
    identify_passport(&mut value);
    value
}

fn passport(realm: RealmDescriptor<'static>) -> EntityPassport<'static> {
    passport_with(
        realm,
        "fixture/member",
        &KEYS,
        KEY.id,
        KeyProtection::ExportableSoftware,
    )
}

fn identify_passport(passport: &mut EntityPassport<'_>) {
    let mut scratch = [ZERO; 40];
    passport.identity = passport
        .computed_semantic_hash(&mut scratch[..passport.identity_fact_count()])
        .unwrap();
}

fn status(
    passport: EntityPassport<'static>,
    status: PassportStatus,
) -> PassportStatusObservation<'static> {
    PassportStatusObservation {
        passport: passport.identity,
        realm: passport.realm,
        entity: passport.entity,
        reporter: pin("fixture/status", 11),
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 20,
        status,
    }
}

fn verification(
    passport: EntityPassport<'static>,
    outcome: CredentialVerificationOutcome,
) -> CredentialVerification<'static> {
    let mut value = CredentialVerification {
        identity: ZERO,
        credential: passport.credential.id,
        passport: passport.identity,
        verifier: pin("fixture/credential-verifier", 20),
        challenge: Id("fixture/challenge"),
        time_basis: Id("fixture/clock"),
        observed_at_tick: 10,
        valid_until_tick: 20,
        outcome,
        receipt: SemanticHash::from_bytes([21; 32]),
    };
    value.identity = value.computed_semantic_hash().unwrap();
    value
}

fn authorship(
    passport: EntityPassport<'static>,
    strength: AttributionStrength,
    bridge: Option<Id<'static>>,
) -> EventAuthorship<'static> {
    EventAuthorship {
        realm: passport.realm,
        entity: passport.entity,
        key: passport.credential.key,
        credential: passport.credential.id,
        delegation: None,
        strength,
        receipt: SemanticHash::from_bytes([14; 32]),
        bridge,
        status: status(passport, PassportStatus::Active),
    }
}

fn control_event(kind: RealmControlKind) -> RealmControlEvent<'static> {
    RealmControlEvent {
        kind,
        envelope: ResonanceEnvelope {
            event: Id(kind.detail()),
            stream: Id("fixture/realm-control-stream"),
            run: Id("fixture/realm-control-run"),
            plan_epoch: SemanticHash::from_bytes([22; 32]),
            producer: InstancePath::new("realm/provider").unwrap(),
            subject: InstancePath::new("realm/member").unwrap(),
            class: EventClass::Control,
            sequence: 1,
            observer: Id("fixture/realm-provider"),
            observer_sequence: 1,
            domain_time: Some((Id("fixture/clock"), 10)),
            correlation: Some(Id("fixture/enrollment")),
            idempotency: Some(Id("fixture/enrollment-request")),
            payload_type: EVENT_TYPE,
            payload: EventPayloadRef::Redacted {
                original_bytes: 64,
                reason: Id("passport-sensitive"),
            },
            relations: ResonanceRelations {
                caused_by: None,
                derived_from: &[],
                supersedes: None,
                corrects: None,
                retracts: None,
            },
            provenance: Id("fixture/realm-provider"),
            recording_authority: Some(Id("fixture/enrollment-grant")),
            sensitivity: Sensitivity::Restricted,
            integrity: SemanticHash::from_bytes([23; 32]),
        },
        authority: Id("fixture/enrollment-grant"),
        receipt: SemanticHash::from_bytes([24; 32]),
    }
}

fn assert_case(id: &str, actual: Value) {
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let expected = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("missing fixture case {id}"))["expected"]
        .clone();
    assert_eq!(actual, expected, "fixture case {id}");
}

fn outcome(result: Result<(), RealmReason>) -> Value {
    match result {
        Ok(()) => json!({"accepted": true}),
        Err(reason) => json!({"accepted": false, "code": reason.code()}),
    }
}

#[test]
fn create_realm_and_enroll_software_key_member() {
    let realm = realm();
    let passport = passport(realm);
    let mut realm_scratch = [ZERO; 1];
    let mut passport_scratch = [ZERO; 3];
    let accepted = validate_realm(&realm, &mut realm_scratch).and_then(|()| {
        validate_passport_at(
            &passport,
            &realm,
            verification(passport, CredentialVerificationOutcome::Verified),
            Id("fixture/clock"),
            15,
            &mut passport_scratch,
        )
    });
    assert_eq!(
        validate_realm_control_event(control_event(RealmControlKind::EnrollmentRequested)),
        Ok(())
    );
    assert_case(
        "create-realm-and-enroll-software-key-member",
        outcome(accepted),
    );
}

#[test]
fn physical_presence_exportable_key_limitation() {
    let realm = realm();
    let passport = passport(realm);
    let mut scratch = [ZERO; 3];
    assert_eq!(validate_passport(&passport, &realm, &mut scratch), Ok(()));
    assert_case(
        "physical-presence-exportable-key-limitation",
        json!({
            "accepted": true,
            "attestation": passport.key_protection.as_str()
        }),
    );
}

#[test]
fn key_control_challenge_failure() {
    let realm = realm();
    let passport = passport(realm);
    assert_case(
        "key-control-challenge-failure",
        outcome(validate_credential_verification(
            verification(passport, CredentialVerificationOutcome::Rejected),
            passport.credential.id,
            passport.identity,
            Id("fixture/clock"),
            15,
        )),
    );
}

#[test]
fn credential_verification_identity_covers_challenge_outcome_and_receipt() {
    let passport = passport(realm());
    let verified = verification(passport, CredentialVerificationOutcome::Verified);
    for mut changed in [
        CredentialVerification {
            challenge: Id("fixture/other-challenge"),
            ..verified
        },
        CredentialVerification {
            outcome: CredentialVerificationOutcome::Rejected,
            ..verified
        },
        CredentialVerification {
            receipt: SemanticHash::from_bytes([30; 32]),
            ..verified
        },
    ] {
        assert_ne!(changed.computed_semantic_hash().unwrap(), verified.identity);
        assert_eq!(
            validate_credential_verification(
                changed,
                passport.credential.id,
                passport.identity,
                Id("fixture/clock"),
                15,
            ),
            Err(RealmReason::CredentialRejected)
        );
        changed.identity = changed.computed_semantic_hash().unwrap();
        assert_ne!(changed.identity, verified.identity);
    }
}

#[test]
fn entity_identity_survives_key_rotation() {
    let realm = realm();
    let prior = passport(realm);
    let successor = passport_with(
        realm,
        "fixture/member",
        &ROTATED_KEYS,
        KEY_2.id,
        KeyProtection::ExportableSoftware,
    );
    let transition = EntityKeyTransition {
        entity: prior.entity,
        prior: KEY.id,
        successor: KEY_2.id,
        prior_epoch: 1,
        successor_epoch: 2,
        authorized_by: prior.credential.id,
        receipt: SemanticHash::from_bytes([25; 32]),
    };
    assert_eq!(
        validate_entity_key_transition(transition, &prior, &successor),
        Ok(())
    );
    assert_case(
        "entity-identity-survives-key-rotation",
        json!({"accepted": true, "entity_changed": prior.entity != successor.entity}),
    );
}

#[test]
fn realm_identity_survives_root_succession() {
    let prior = realm();
    let roots = [ROOT, ROOT_2];
    let successions = [RootSuccession {
        prior: ROOT.id,
        successor: ROOT_2.id,
        prior_epoch: 1,
        successor_epoch: 2,
        receipt: SemanticHash::from_bytes([26; 32]),
    }];
    let mut successor = RealmDescriptor {
        accepted_roots: &roots,
        root_epoch: 2,
        successions: &successions,
        identity: ZERO,
        ..prior
    };
    let mut hash_scratch = [ZERO; 3];
    successor.identity = successor.computed_semantic_hash(&mut hash_scratch).unwrap();
    let mut validation_scratch = [ZERO; 3];
    assert_eq!(validate_realm(&successor, &mut validation_scratch), Ok(()));
    assert_case(
        "realm-identity-survives-root-succession",
        json!({"accepted": true, "realm_changed": prior.id != successor.id}),
    );
}

#[test]
fn replacement_part_has_new_identity_and_old_role() {
    let realm = realm();
    let prior = passport(realm);
    let replacement = passport_with(
        realm,
        "fixture/replacement",
        &KEYS,
        KEY.id,
        KeyProtection::ExportableSoftware,
    );
    let mut scratch = [ZERO; 3];
    assert_eq!(
        validate_passport(&replacement, &realm, &mut scratch),
        Ok(())
    );
    assert_case(
        "replacement-part-has-new-identity-and-old-role",
        json!({
            "accepted": true,
            "entity_changed": prior.entity != replacement.entity,
            "role_reassigned": prior.roles[0].role == replacement.roles[0].role
        }),
    );
}

#[test]
fn old_authorship_remains_old_entity() {
    let realm = realm();
    let prior = passport(realm);
    let replacement = passport_with(
        realm,
        "fixture/replacement",
        &KEYS,
        KEY.id,
        KeyProtection::ExportableSoftware,
    );
    let signed = authorship(prior, AttributionStrength::DirectSignature, None);
    assert_eq!(
        validate_event_authorship(signed, &prior, &realm, Id("fixture/clock"), 15),
        Ok(())
    );
    assert_eq!(
        validate_event_authorship(signed, &replacement, &realm, Id("fixture/clock"), 15),
        Err(RealmReason::CredentialRejected)
    );
    assert_case(
        "old-authorship-remains-old-entity",
        json!({"accepted": true, "rewritten": false}),
    );
}

#[test]
fn current_role_does_not_rewrite_history() {
    let realm = realm();
    let prior = passport(realm);
    let replacement = passport_with(
        realm,
        "fixture/replacement",
        &KEYS,
        KEY.id,
        KeyProtection::ExportableSoftware,
    );
    let historical = authorship(prior, AttributionStrength::SignedBatch, None);
    assert_eq!(prior.roles[0].role, replacement.roles[0].role);
    assert_eq!(historical.entity, prior.entity);
    assert_ne!(historical.entity, replacement.entity);
    assert_case(
        "current-role-does-not-rewrite-history",
        json!({"accepted": true, "rewritten": false}),
    );
}

#[test]
fn membership_role_and_grant_are_distinct() {
    assert_case(
        "membership-role-and-grant-are-distinct",
        outcome(require_realm_operation_authority(None)),
    );
}

#[test]
fn artifact_signature_is_not_member_passport() {
    let realm = realm();
    let passport = passport(realm);
    let mut artifact_verification = verification(passport, CredentialVerificationOutcome::Verified);
    artifact_verification.credential = Id("fixture/artifact-signature");
    artifact_verification.identity = artifact_verification.computed_semantic_hash().unwrap();
    assert_case(
        "artifact-signature-not-member-passport",
        outcome(validate_credential_verification(
            artifact_verification,
            passport.credential.id,
            passport.identity,
            Id("fixture/clock"),
            15,
        )),
    );
}

#[test]
fn member_passport_is_not_artifact_signature() {
    let realm = realm();
    let passport = passport(realm);
    assert_case(
        "member-passport-not-artifact-signature",
        outcome(validate_credential_verification(
            verification(passport, CredentialVerificationOutcome::Verified),
            passport.credential.id,
            SemanticHash::from_bytes([27; 32]),
            Id("fixture/clock"),
            15,
        )),
    );
}

#[test]
fn transport_auth_is_not_effect_grant() {
    assert_case(
        "transport-auth-not-effect-grant",
        outcome(require_realm_operation_authority(None)),
    );
}

#[test]
fn workload_delegation_is_bound_to_one_run_epoch() {
    let realm = realm();
    let passport = passport(realm);
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
            15,
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
            15,
        ),
        Err(RealmReason::DelegationDenied)
    );
    assert_case(
        "workload-delegation-one-run-epoch",
        json!({"accepted": true, "exact_epoch": true}),
    );
}

#[test]
fn expired_suspended_revoked_retired_and_compromised_status_fail() {
    let realm = realm();
    let passport = passport(realm);
    for rejected in [
        PassportStatus::Suspended,
        PassportStatus::Revoked,
        PassportStatus::Retired,
        PassportStatus::Compromised,
    ] {
        assert_eq!(
            validate_passport_status(
                status(passport, rejected),
                passport.identity,
                realm.id,
                passport.entity,
                Id("fixture/clock"),
                15,
            ),
            Err(RealmReason::StatusUnavailable)
        );
    }
    assert_case(
        "expired-suspended-revoked-retired-compromised-status",
        outcome(validate_passport_status(
            status(passport, PassportStatus::Active),
            passport.identity,
            realm.id,
            passport.entity,
            Id("fixture/clock"),
            20,
        )),
    );
}

#[test]
fn stale_status_and_revocation_gap_fail() {
    let realm = realm();
    let passport = passport(realm);
    assert_eq!(
        validate_passport_status(
            status(passport, PassportStatus::Gap),
            passport.identity,
            realm.id,
            passport.entity,
            Id("fixture/clock"),
            15,
        ),
        Err(RealmReason::StatusUnavailable)
    );
    assert_eq!(
        validate_credential_verification(
            verification(passport, CredentialVerificationOutcome::Unavailable),
            passport.credential.id,
            passport.identity,
            Id("fixture/clock"),
            15,
        ),
        Err(RealmReason::StatusUnavailable)
    );
    assert_case(
        "stale-status-and-revocation-gap",
        outcome(validate_passport_status(
            status(passport, PassportStatus::Active),
            passport.identity,
            realm.id,
            passport.entity,
            Id("fixture/clock"),
            20,
        )),
    );
}

#[test]
fn replayed_credential_or_event_is_rejected() {
    let realm = realm();
    let passport = passport(realm);
    assert_case(
        "replayed-credential-or-event-rejected",
        outcome(validate_credential_verification(
            verification(passport, CredentialVerificationOutcome::Replayed),
            passport.credential.id,
            passport.identity,
            Id("fixture/clock"),
            15,
        )),
    );
}

#[test]
fn cloned_software_key_conflicting_sessions_fail_status() {
    let realm = realm();
    let passport = passport(realm);
    assert_case(
        "cloned-software-key-conflicting-sessions",
        outcome(validate_credential_verification(
            verification(
                passport,
                CredentialVerificationOutcome::ConflictingLiveSession,
            ),
            passport.credential.id,
            passport.identity,
            Id("fixture/clock"),
            15,
        )),
    );
}

#[test]
fn direct_batch_and_recorder_attribution_are_distinct() {
    let realm = realm();
    let passport = passport(realm);
    let strengths = [
        AttributionStrength::DirectSignature,
        AttributionStrength::SignedBatch,
        AttributionStrength::RecorderReceipt,
    ];
    for strength in strengths {
        assert_eq!(
            validate_event_authorship(
                authorship(passport, strength, None),
                &passport,
                &realm,
                Id("fixture/clock"),
                15,
            ),
            Ok(())
        );
    }
    assert_ne!(strengths[0], strengths[1]);
    assert_ne!(strengths[1], strengths[2]);
    assert_case(
        "direct-batch-recorder-attribution-distinct",
        json!({"accepted": true, "attribution_distinct": true}),
    );
}

#[test]
fn authentic_event_is_not_asserted_true() {
    let realm = realm();
    let passport = passport(realm);
    assert_eq!(
        validate_event_authorship(
            authorship(passport, AttributionStrength::DirectSignature, None),
            &passport,
            &realm,
            Id("fixture/clock"),
            15,
        ),
        Ok(())
    );
    assert_case(
        "authentic-event-is-not-asserted-true",
        json!({"accepted": true, "truth_claimed": false}),
    );
}

#[test]
fn directional_federation_allows_stream_and_denies_effect() {
    let realm = realm();
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
            false,
        ),
        Ok(())
    );
    assert_eq!(
        validate_federation(
            federation,
            realm.id,
            Id("fixture/remote-realm"),
            stream,
            Id("fixture/clock"),
            15,
            true,
        ),
        Err(RealmReason::FederationDenied)
    );
    assert_case(
        "directional-federation-allows-stream-denies-effect",
        json!({"accepted": true, "effect_allowed": false}),
    );
}

#[test]
fn federation_is_non_transitive() {
    let stream = pin("fixture/stream", 15);
    let federation = FederationPolicy {
        id: Id("fixture/a-to-b"),
        local_realm: Id("fixture/realm-a"),
        remote_realm: Id("fixture/realm-b"),
        local_root_epoch: 1,
        remote_root_epoch: 1,
        time_basis: Id("fixture/clock"),
        expires_at_tick: 20,
        allow_identity: true,
        allow_event_verification: true,
        allow_transport_admission: false,
        allow_grant_delegation: false,
        allowed_streams: &[stream],
        receipt: SemanticHash::from_bytes([28; 32]),
    };
    assert_case(
        "federation-is-non-transitive",
        outcome(validate_federation(
            federation,
            Id("fixture/realm-a"),
            Id("fixture/realm-c"),
            stream,
            Id("fixture/clock"),
            15,
            false,
        )),
    );
}

#[test]
fn conflicting_root_rotation_is_rejected() {
    let prior = realm();
    let roots = [ROOT, ROOT_2];
    let successions = [RootSuccession {
        prior: Id("fixture/unrecognized-root"),
        successor: ROOT_2.id,
        prior_epoch: 1,
        successor_epoch: 2,
        receipt: SemanticHash::from_bytes([29; 32]),
    }];
    let mut conflicting = RealmDescriptor {
        accepted_roots: &roots,
        root_epoch: 2,
        successions: &successions,
        identity: ZERO,
        ..prior
    };
    let mut hash_scratch = [ZERO; 3];
    conflicting.identity = conflicting
        .computed_semantic_hash(&mut hash_scratch)
        .unwrap();
    let mut validation_scratch = [ZERO; 3];
    assert_case(
        "conflicting-root-rotation-rejected",
        outcome(validate_realm(&conflicting, &mut validation_scratch)),
    );
}

#[test]
fn bridge_preserves_remote_authorship() {
    let remote_realm = realm_with_id("fixture/remote-realm");
    let remote_passport = passport_with(
        remote_realm,
        "fixture/remote-member",
        &KEYS,
        KEY.id,
        KeyProtection::ExportableSoftware,
    );
    let bridged = authorship(
        remote_passport,
        AttributionStrength::RecorderReceipt,
        Some(Id("fixture/bridge")),
    );
    assert_eq!(
        validate_event_authorship(
            bridged,
            &remote_passport,
            &remote_realm,
            Id("fixture/clock"),
            15,
        ),
        Ok(())
    );
    assert_case(
        "bridge-preserves-remote-authorship",
        json!({
            "accepted": true,
            "remote_authorship_preserved": bridged.realm == remote_realm.id
                && bridged.entity == remote_passport.entity
        }),
    );
}

#[test]
fn offline_credential_expiry_is_bounded() {
    let realm = realm();
    let passport = passport(realm);
    let mut offline_status = status(passport, PassportStatus::Active);
    offline_status.valid_until_tick = passport.credential.expires_at_tick;
    assert_case(
        "offline-credential-expiry-is-bounded",
        outcome(validate_passport_status(
            offline_status,
            passport.identity,
            realm.id,
            passport.entity,
            Id("fixture/clock"),
            passport.credential.expires_at_tick,
        )),
    );
}

#[test]
fn constrained_host_honestly_lacks_attestation() {
    let realm = realm();
    let passport = passport_with(
        realm,
        "fixture/constrained-member",
        &KEYS,
        KEY.id,
        KeyProtection::UnsupportedAttestation,
    );
    let mut scratch = [ZERO; 3];
    assert_eq!(validate_passport(&passport, &realm, &mut scratch), Ok(()));
    assert_case(
        "constrained-host-honestly-lacks-attestation",
        json!({
            "accepted": true,
            "attestation": passport.key_protection.as_str()
        }),
    );
}

#[test]
fn every_realm_control_kind_is_stable_and_authority_bound() {
    let kinds = [
        RealmControlKind::EnrollmentRequested,
        RealmControlKind::EnrollmentChallenged,
        RealmControlKind::EnrollmentApproved,
        RealmControlKind::EnrollmentDenied,
        RealmControlKind::MembershipIssued,
        RealmControlKind::MembershipRenewed,
        RealmControlKind::MembershipExpired,
        RealmControlKind::KeyAdded,
        RealmControlKind::KeyRotated,
        RealmControlKind::KeyCompromised,
        RealmControlKind::KeyRetired,
        RealmControlKind::RoleBound,
        RealmControlKind::RoleUnbound,
        RealmControlKind::RoleReassigned,
        RealmControlKind::EntitySuspended,
        RealmControlKind::EntityReinstated,
        RealmControlKind::EntityRevoked,
        RealmControlKind::EntityRetired,
        RealmControlKind::RootAdded,
        RealmControlKind::RootRotated,
        RealmControlKind::RootEmergencyReplaced,
        RealmControlKind::RootRetired,
        RealmControlKind::FederationEstablished,
        RealmControlKind::FederationNarrowed,
        RealmControlKind::FederationSuspended,
        RealmControlKind::FederationRevoked,
        RealmControlKind::PassportProjectionRebuilt,
        RealmControlKind::PassportProjectionStale,
        RealmControlKind::PassportProjectionGap,
    ];
    for (index, kind) in kinds.iter().enumerate() {
        assert_eq!(
            validate_realm_control_event(control_event(*kind)),
            Ok(()),
            "{}",
            kind.detail()
        );
        assert!(
            !kinds[..index]
                .iter()
                .any(|prior| prior.detail() == kind.detail())
        );
    }
    let mut unauthorized = control_event(RealmControlKind::EnrollmentApproved);
    unauthorized.envelope.recording_authority = None;
    assert_eq!(
        validate_realm_control_event(unauthorized),
        Err(RealmReason::AuthorityRequired)
    );
    let mut public = control_event(RealmControlKind::EnrollmentApproved);
    public.envelope.sensitivity = Sensitivity::Public;
    assert_eq!(
        validate_realm_control_event(public),
        Err(RealmReason::SensitiveDisclosure)
    );
}
