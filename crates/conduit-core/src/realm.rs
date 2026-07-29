//! Allocator-free realm, entity, membership, and authorship contracts.

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, FieldDisposition, Id, MapField,
    PinnedDescriptor, ResonanceEnvelope, ResonanceError, SemanticHash, Sensitivity,
    validate_envelope,
};

pub const REALM_SCHEMA_VERSION: u32 = 1;
pub const MAX_REALM_ROOT_KEYS: usize = 8;
pub const MAX_PASSPORT_KEYS: usize = 8;
pub const MAX_PASSPORT_ROLES: usize = 16;
pub const MAX_PASSPORT_EXTENSIONS: usize = 8;
pub const MAX_DELEGATION_DEPTH: u8 = 4;
pub const MAX_FEDERATION_STREAMS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicKeyRef<'a> {
    pub id: Id<'a>,
    pub algorithm: Id<'a>,
    /// Public key bytes are kept by a crypto provider; this is a public pin.
    pub public_key_digest: SemanticHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootSuccession<'a> {
    pub prior: Id<'a>,
    pub successor: Id<'a>,
    pub prior_epoch: u32,
    pub successor_epoch: u32,
    pub receipt: SemanticHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmDescriptor<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub genesis_root: PublicKeyRef<'a>,
    pub accepted_roots: &'a [PublicKeyRef<'a>],
    pub root_epoch: u32,
    pub policy: PinnedDescriptor<'a>,
    pub membership_profile: PinnedDescriptor<'a>,
    pub revocation_profile: PinnedDescriptor<'a>,
    pub event_integrity_profile: PinnedDescriptor<'a>,
    pub federation_profile: PinnedDescriptor<'a>,
    pub successions: &'a [RootSuccession<'a>],
    // Presentation-only callsigns are deliberately absent from identity.
}

impl RealmDescriptor<'_> {
    pub const fn identity_fact_count(&self) -> usize {
        self.accepted_roots.len() + self.successions.len()
    }

    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, RealmIdentityError> {
        let needed = self.identity_fact_count();
        if scratch.len() < needed {
            return Err(RealmIdentityError::ScratchTooSmall);
        }
        let mut at = 0;
        for key in self.accepted_roots {
            scratch[at] = hash_key(*key)?;
            at += 1;
        }
        for succession in self.successions {
            scratch[at] = hash_succession(*succession)?;
            at += 1;
        }
        let genesis = hash_key(self.genesis_root)?;
        let policy = hash_pin(self.policy)?;
        let membership = hash_pin(self.membership_profile)?;
        let revocation = hash_pin(self.revocation_profile)?;
        let event_integrity = hash_pin(self.event_integrity_profile)?;
        let federation = hash_pin(self.federation_profile)?;
        let fields = [
            field("id", CanonicalValue::Identifier(self.id)),
            field("genesis_root", CanonicalValue::Bytes(genesis.as_bytes())),
            field(
                "root_epoch",
                CanonicalValue::Integer(i128::from(self.root_epoch)),
            ),
            field("policy", CanonicalValue::Bytes(policy.as_bytes())),
            field(
                "membership_profile",
                CanonicalValue::Bytes(membership.as_bytes()),
            ),
            field(
                "revocation_profile",
                CanonicalValue::Bytes(revocation.as_bytes()),
            ),
            field(
                "event_integrity_profile",
                CanonicalValue::Bytes(event_integrity.as_bytes()),
            ),
            field(
                "federation_profile",
                CanonicalValue::Bytes(federation.as_bytes()),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/realm"),
            self.schema_version,
            &fields,
            Id("facts"),
            &scratch[..needed],
        )
        .map_err(RealmIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyProtection {
    HardwareProtected,
    ExportableSoftware,
    UnsupportedAttestation,
}
impl KeyProtection {
    const fn as_str(self) -> &'static str {
        match self {
            Self::HardwareProtected => "hardware-protected",
            Self::ExportableSoftware => "exportable-software-key",
            Self::UnsupportedAttestation => "unsupported-attestation",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MembershipCredential<'a> {
    pub id: Id<'a>,
    pub realm: Id<'a>,
    pub entity: Id<'a>,
    pub key: Id<'a>,
    pub issuer_key: Id<'a>,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub time_basis: Id<'a>,
    pub receipt: SemanticHash,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoleBinding<'a> {
    pub role: PinnedDescriptor<'a>,
    pub binding: Id<'a>,
    pub expires_at_tick: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntityPassport<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub entity: Id<'a>,
    pub profile: PinnedDescriptor<'a>,
    pub realm: Id<'a>,
    pub credential: MembershipCredential<'a>,
    pub keys: &'a [PublicKeyRef<'a>],
    pub roles: &'a [RoleBinding<'a>],
    pub key_protection: KeyProtection,
    pub sensitivity: Sensitivity,
    pub extensions: &'a [PinnedDescriptor<'a>],
}
impl EntityPassport<'_> {
    pub const fn identity_fact_count(&self) -> usize {
        1 + self.keys.len() + self.roles.len() + self.extensions.len()
    }
    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, RealmIdentityError> {
        let needed = self.identity_fact_count();
        if scratch.len() < needed {
            return Err(RealmIdentityError::ScratchTooSmall);
        }
        scratch[0] = hash_credential(self.credential)?;
        let mut at = 1;
        for key in self.keys {
            scratch[at] = hash_key(*key)?;
            at += 1;
        }
        for role in self.roles {
            scratch[at] = hash_role(*role)?;
            at += 1;
        }
        for extension in self.extensions {
            scratch[at] = hash_pin(*extension)?;
            at += 1;
        }
        let profile = hash_pin(self.profile)?;
        let fields = [
            field("entity", CanonicalValue::Identifier(self.entity)),
            field("profile", CanonicalValue::Bytes(profile.as_bytes())),
            field("realm", CanonicalValue::Identifier(self.realm)),
            field(
                "key_protection",
                CanonicalValue::Identifier(Id(self.key_protection.as_str())),
            ),
            field(
                "sensitivity",
                CanonicalValue::Identifier(Id(self.sensitivity.as_str())),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/entity-passport"),
            self.schema_version,
            &fields,
            Id("facts"),
            &scratch[..needed],
        )
        .map_err(RealmIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PassportStatus {
    Active,
    Suspended,
    Revoked,
    Retired,
    Compromised,
    Gap,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PassportStatusObservation<'a> {
    pub passport: SemanticHash,
    pub realm: Id<'a>,
    pub reporter: PinnedDescriptor<'a>,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub status: PassportStatus,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadDelegation<'a> {
    pub id: Id<'a>,
    pub realm: Id<'a>,
    pub entity: Id<'a>,
    pub passport: SemanticHash,
    pub plan: SemanticHash,
    pub run: Id<'a>,
    pub epoch: u64,
    pub audience: Id<'a>,
    pub expires_at_tick: u64,
    pub depth: u8,
    pub receipt: SemanticHash,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributionStrength {
    DirectSignature,
    SignedBatch,
    RecorderReceipt,
}
/// Stable `EventClass::Control` detail vocabulary for append-only realm facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealmControlKind {
    EnrollmentRequested,
    MembershipIssued,
    MembershipRenewed,
    MembershipExpired,
    KeyAdded,
    KeyRotated,
    KeyCompromised,
    KeyRetired,
    RoleBound,
    RoleUnbound,
    RoleReassigned,
    EntitySuspended,
    EntityReinstated,
    EntityRevoked,
    EntityRetired,
    RootAdded,
    RootRotated,
    RootEmergencyReplaced,
    RootRetired,
    FederationEstablished,
    FederationNarrowed,
    FederationSuspended,
    FederationRevoked,
    PassportProjectionRebuilt,
    PassportProjectionStale,
    PassportProjectionGap,
}
impl RealmControlKind {
    #[must_use]
    pub const fn detail(self) -> &'static str {
        match self {
            Self::EnrollmentRequested => "realm.enrollment-requested",
            Self::MembershipIssued => "realm.membership-issued",
            Self::MembershipRenewed => "realm.membership-renewed",
            Self::MembershipExpired => "realm.membership-expired",
            Self::KeyAdded => "realm.key-added",
            Self::KeyRotated => "realm.key-rotated",
            Self::KeyCompromised => "realm.key-compromised",
            Self::KeyRetired => "realm.key-retired",
            Self::RoleBound => "realm.role-bound",
            Self::RoleUnbound => "realm.role-unbound",
            Self::RoleReassigned => "realm.role-reassigned",
            Self::EntitySuspended => "realm.entity-suspended",
            Self::EntityReinstated => "realm.entity-reinstated",
            Self::EntityRevoked => "realm.entity-revoked",
            Self::EntityRetired => "realm.entity-retired",
            Self::RootAdded => "realm.root-added",
            Self::RootRotated => "realm.root-rotated",
            Self::RootEmergencyReplaced => "realm.root-emergency-replaced",
            Self::RootRetired => "realm.root-retired",
            Self::FederationEstablished => "realm.federation-established",
            Self::FederationNarrowed => "realm.federation-narrowed",
            Self::FederationSuspended => "realm.federation-suspended",
            Self::FederationRevoked => "realm.federation-revoked",
            Self::PassportProjectionRebuilt => "realm.passport-projection-rebuilt",
            Self::PassportProjectionStale => "realm.passport-projection-stale",
            Self::PassportProjectionGap => "realm.passport-projection-gap",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EventAuthorship<'a> {
    pub realm: Id<'a>,
    pub entity: Id<'a>,
    pub key: Id<'a>,
    pub credential: Id<'a>,
    pub delegation: Option<Id<'a>>,
    pub strength: AttributionStrength,
    pub receipt: SemanticHash,
    pub bridge: Option<Id<'a>>,
    pub status: PassportStatusObservation<'a>,
}

/// Additive protected authorship. The enclosed Resonance envelope retains its
/// own frozen identity through verification and any bridge hop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedResonanceEnvelope<'a> {
    pub envelope: ResonanceEnvelope<'a>,
    pub authorship: EventAuthorship<'a>,
}

/// A directional trust relation. `local -> remote` is never transitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FederationPolicy<'a> {
    pub id: Id<'a>,
    pub local_realm: Id<'a>,
    pub remote_realm: Id<'a>,
    pub local_root_epoch: u32,
    pub remote_root_epoch: u32,
    pub time_basis: Id<'a>,
    pub expires_at_tick: u64,
    pub allow_identity: bool,
    pub allow_event_verification: bool,
    pub allow_transport_admission: bool,
    pub allow_grant_delegation: bool,
    pub allowed_streams: &'a [PinnedDescriptor<'a>],
    pub receipt: SemanticHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealmIdentityError {
    ScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealmReason {
    UnsupportedSchema,
    InvalidDescriptor,
    IdentityMismatch,
    RootConflict,
    CredentialRejected,
    StatusUnavailable,
    DelegationDenied,
    FederationDenied,
    SensitiveDisclosure,
    AuthorityRequired,
    BoundExceeded,
}
impl RealmReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedSchema | Self::InvalidDescriptor => "CND-RLM-001",
            Self::IdentityMismatch => "CND-RLM-002",
            Self::RootConflict => "CND-RLM-003",
            Self::CredentialRejected => "CND-RLM-004",
            Self::StatusUnavailable => "CND-RLM-005",
            Self::DelegationDenied => "CND-RLM-006",
            Self::FederationDenied => "CND-RLM-007",
            Self::SensitiveDisclosure => "CND-RLM-008",
            Self::AuthorityRequired => "CND-RLM-009",
            Self::BoundExceeded => "CND-RLM-010",
        }
    }
}

pub fn validate_realm(
    realm: &RealmDescriptor<'_>,
    scratch: &mut [SemanticHash],
) -> Result<(), RealmReason> {
    if realm.schema_version != REALM_SCHEMA_VERSION {
        return Err(RealmReason::UnsupportedSchema);
    }
    if realm.accepted_roots.is_empty()
        || realm.accepted_roots.len() > MAX_REALM_ROOT_KEYS
        || realm.successions.len() > MAX_REALM_ROOT_KEYS
        || !valid_id(realm.id)
        || !valid_key(realm.genesis_root)
        || !valid_pin(realm.policy)
        || !valid_pin(realm.membership_profile)
        || !valid_pin(realm.revocation_profile)
        || !valid_pin(realm.event_integrity_profile)
        || !valid_pin(realm.federation_profile)
    {
        return Err(RealmReason::InvalidDescriptor);
    }
    if realm.accepted_roots.iter().enumerate().any(|(index, key)| {
        !valid_key(*key)
            || realm.accepted_roots[..index]
                .iter()
                .any(|prior| prior.id == key.id)
    }) || realm
        .successions
        .iter()
        .any(|entry| !valid_succession(*entry))
    {
        return Err(RealmReason::InvalidDescriptor);
    }
    if realm.computed_semantic_hash(scratch).ok() != Some(realm.identity) {
        return Err(RealmReason::IdentityMismatch);
    }
    let mut current_key = realm.genesis_root.id;
    let mut current_epoch = 1;
    for succession in realm.successions {
        if succession.prior != current_key || succession.prior_epoch != current_epoch {
            return Err(RealmReason::RootConflict);
        }
        current_key = succession.successor;
        current_epoch = succession.successor_epoch;
    }
    if realm.root_epoch != current_epoch
        || !realm.accepted_roots.iter().any(|key| key.id == current_key)
    {
        return Err(RealmReason::RootConflict);
    }
    Ok(())
}
pub fn validate_passport(
    passport: &EntityPassport<'_>,
    realm: &RealmDescriptor<'_>,
    scratch: &mut [SemanticHash],
) -> Result<(), RealmReason> {
    if passport.schema_version != REALM_SCHEMA_VERSION {
        return Err(RealmReason::UnsupportedSchema);
    }
    if passport.keys.is_empty()
        || passport.keys.len() > MAX_PASSPORT_KEYS
        || passport.roles.len() > MAX_PASSPORT_ROLES
        || passport.extensions.len() > MAX_PASSPORT_EXTENSIONS
        || !valid_id(passport.entity)
        || !valid_id(passport.realm)
        || passport.realm != realm.id
        || !valid_pin(passport.profile)
        || !valid_credential(passport.credential)
        || passport.credential.entity != passport.entity
        || passport.credential.realm != realm.id
    {
        return Err(RealmReason::InvalidDescriptor);
    }
    if passport.keys.iter().any(|key| !valid_key(*key))
        || passport.roles.iter().any(|role| !valid_role(*role))
        || passport
            .extensions
            .iter()
            .any(|extension| !valid_pin(*extension))
    {
        return Err(RealmReason::InvalidDescriptor);
    }
    if passport.computed_semantic_hash(scratch).ok() != Some(passport.identity) {
        return Err(RealmReason::IdentityMismatch);
    }
    if !passport
        .keys
        .iter()
        .any(|key| key.id == passport.credential.key)
    {
        return Err(RealmReason::CredentialRejected);
    }
    Ok(())
}
pub fn validate_passport_status(
    status: PassportStatusObservation<'_>,
    passport: SemanticHash,
    realm: Id<'_>,
    time_basis: Id<'_>,
    tick: u64,
) -> Result<(), RealmReason> {
    if status.passport != passport
        || status.realm != realm
        || status.time_basis != time_basis
        || !valid_id(status.realm)
        || !valid_id(status.time_basis)
        || !valid_pin(status.reporter)
        || status.observed_at_tick > status.valid_until_tick
        || tick < status.observed_at_tick
        || tick >= status.valid_until_tick
        || status.status != PassportStatus::Active
    {
        return Err(RealmReason::StatusUnavailable);
    }
    Ok(())
}
pub fn validate_delegation(
    delegation: WorkloadDelegation<'_>,
    passport: SemanticHash,
    realm: Id<'_>,
    entity: Id<'_>,
    run: Id<'_>,
    epoch: u64,
    tick: u64,
) -> Result<(), RealmReason> {
    if !valid_id(delegation.id)
        || !valid_id(delegation.realm)
        || !valid_id(delegation.entity)
        || !valid_id(delegation.run)
        || !valid_id(delegation.audience)
        || delegation.passport != passport
        || delegation.realm != realm
        || delegation.entity != entity
        || delegation.run != run
        || delegation.epoch != epoch
        || delegation.depth > MAX_DELEGATION_DEPTH
        || tick >= delegation.expires_at_tick
    {
        return Err(RealmReason::DelegationDenied);
    }
    Ok(())
}

/// Verifies only explicit, currently supplied membership/status pins. Actual
/// signature verification remains a selected crypto-provider result encoded
/// by the credential receipt; this portable layer never sees private keys.
pub fn validate_event_authorship(
    authorship: EventAuthorship<'_>,
    passport: &EntityPassport<'_>,
    realm: &RealmDescriptor<'_>,
    time_basis: Id<'_>,
    tick: u64,
) -> Result<(), RealmReason> {
    if authorship.realm != realm.id
        || authorship.entity != passport.entity
        || authorship.credential != passport.credential.id
        || !passport.keys.iter().any(|key| key.id == authorship.key)
    {
        return Err(RealmReason::CredentialRejected);
    }
    validate_passport_status(
        authorship.status,
        passport.identity,
        realm.id,
        time_basis,
        tick,
    )
}

pub fn validate_authenticated_resonance_envelope(
    value: AuthenticatedResonanceEnvelope<'_>,
    passport: &EntityPassport<'_>,
    realm: &RealmDescriptor<'_>,
    time_basis: Id<'_>,
    tick: u64,
) -> Result<(), RealmReason> {
    validate_envelope(&value.envelope)
        .map_err(|_error: ResonanceError| RealmReason::InvalidDescriptor)?;
    validate_event_authorship(value.authorship, passport, realm, time_basis, tick)
}

/// Validates one exact federation direction and stream. A caller must supply
/// a separate policy for the reverse relation; no path composition occurs.
pub fn validate_federation(
    policy: FederationPolicy<'_>,
    local_realm: Id<'_>,
    remote_realm: Id<'_>,
    stream: PinnedDescriptor<'_>,
    time_basis: Id<'_>,
    tick: u64,
    require_grant_delegation: bool,
) -> Result<(), RealmReason> {
    if !valid_id(policy.id)
        || !valid_id(policy.local_realm)
        || !valid_id(policy.remote_realm)
        || !valid_id(policy.time_basis)
        || policy.local_realm != local_realm
        || policy.remote_realm != remote_realm
        || policy.local_realm == policy.remote_realm
        || policy.time_basis != time_basis
        || tick >= policy.expires_at_tick
        || policy.allowed_streams.len() > MAX_FEDERATION_STREAMS
        || policy.allowed_streams.iter().any(|item| !valid_pin(*item))
        || !policy.allow_event_verification
        || !policy.allowed_streams.contains(&stream)
        || (require_grant_delegation && !policy.allow_grant_delegation)
    {
        return Err(RealmReason::FederationDenied);
    }
    Ok(())
}

fn field<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
fn hash(kind: &str, fields: &[MapField<'_>]) -> Result<SemanticHash, RealmIdentityError> {
    CanonicalDescriptor {
        kind: Id(kind),
        schema_version: 1,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
    .map_err(RealmIdentityError::Canonical)
}
fn hash_key(value: PublicKeyRef<'_>) -> Result<SemanticHash, RealmIdentityError> {
    hash(
        "conduit/public-key",
        &[
            field("id", CanonicalValue::Identifier(value.id)),
            field("algorithm", CanonicalValue::Identifier(value.algorithm)),
            field(
                "digest",
                CanonicalValue::Bytes(value.public_key_digest.as_bytes()),
            ),
        ],
    )
}
fn hash_pin(value: PinnedDescriptor<'_>) -> Result<SemanticHash, RealmIdentityError> {
    hash(
        "conduit/pin",
        &[
            field("id", CanonicalValue::Identifier(value.id)),
            field(
                "version",
                CanonicalValue::Integer(i128::from(value.schema_version)),
            ),
            field(
                "hash",
                CanonicalValue::Bytes(value.semantic_hash.as_bytes()),
            ),
        ],
    )
}
fn hash_succession(value: RootSuccession<'_>) -> Result<SemanticHash, RealmIdentityError> {
    hash(
        "conduit/root-succession",
        &[
            field("prior", CanonicalValue::Identifier(value.prior)),
            field("successor", CanonicalValue::Identifier(value.successor)),
            field(
                "prior_epoch",
                CanonicalValue::Integer(i128::from(value.prior_epoch)),
            ),
            field(
                "successor_epoch",
                CanonicalValue::Integer(i128::from(value.successor_epoch)),
            ),
            field("receipt", CanonicalValue::Bytes(value.receipt.as_bytes())),
        ],
    )
}
fn hash_credential(value: MembershipCredential<'_>) -> Result<SemanticHash, RealmIdentityError> {
    hash(
        "conduit/membership-credential",
        &[
            field("id", CanonicalValue::Identifier(value.id)),
            field("realm", CanonicalValue::Identifier(value.realm)),
            field("entity", CanonicalValue::Identifier(value.entity)),
            field("key", CanonicalValue::Identifier(value.key)),
            field("issuer_key", CanonicalValue::Identifier(value.issuer_key)),
            field(
                "issued",
                CanonicalValue::Integer(i128::from(value.issued_at_tick)),
            ),
            field(
                "expires",
                CanonicalValue::Integer(i128::from(value.expires_at_tick)),
            ),
            field("time_basis", CanonicalValue::Identifier(value.time_basis)),
            field("receipt", CanonicalValue::Bytes(value.receipt.as_bytes())),
        ],
    )
}
fn hash_role(value: RoleBinding<'_>) -> Result<SemanticHash, RealmIdentityError> {
    let role = hash_pin(value.role)?;
    hash(
        "conduit/role-binding",
        &[
            field("role", CanonicalValue::Bytes(role.as_bytes())),
            field("binding", CanonicalValue::Identifier(value.binding)),
            field(
                "expires",
                CanonicalValue::Integer(i128::from(value.expires_at_tick)),
            ),
        ],
    )
}
fn valid_id(value: Id<'_>) -> bool {
    Id::new(value.as_str()).is_ok()
}
fn valid_pin(value: PinnedDescriptor<'_>) -> bool {
    valid_id(value.id) && value.schema_version > 0
}
fn valid_key(value: PublicKeyRef<'_>) -> bool {
    valid_id(value.id) && valid_id(value.algorithm)
}
fn valid_succession(value: RootSuccession<'_>) -> bool {
    valid_id(value.prior)
        && valid_id(value.successor)
        && value.prior != value.successor
        && value.prior_epoch < value.successor_epoch
}
fn valid_credential(value: MembershipCredential<'_>) -> bool {
    valid_id(value.id)
        && valid_id(value.realm)
        && valid_id(value.entity)
        && valid_id(value.key)
        && valid_id(value.issuer_key)
        && valid_id(value.time_basis)
        && value.issued_at_tick < value.expires_at_tick
}
fn valid_role(value: RoleBinding<'_>) -> bool {
    valid_pin(value.role) && valid_id(value.binding)
}
