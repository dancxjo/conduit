//! Consume attribution and retain only the exact validated borrowed projection.
use super::*;
use crate::{
    check_host_preparation_delegation, AttributedHostCommand, HostPreparationDelegation,
    NativePreparationContext, PreparationDelegationRefusal,
};
use conduit_core as c;

/// Receiver-local pins and actual profile ceilings, never decoded from a sender.
pub struct AssignedBodyPreparationProfile<'a> {
    pub maxima: c::AssignedPlanMaxima,
    pub requirements: c::AssignedPlanRequirements<'a>,
    pub computations: c::AssignedComputationProfile<'a>,
    pub fore: Option<&'a [c::AssignedForeRequirement]>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelegatedFragmentRefusal {
    Policy(PreparationDelegationRefusal),
    Document(BodyFragmentDocumentRefusal),
    Scope,
    Assignment(c::AssignedPlanRefusal),
    Resources(DelegatedResourceRefusal),
    ForeApproval,
}

/// A checked delegated preparation, not a reservation, native capability or Play.
/// Not constructible/deserializable from public metadata. Root must retain its
/// one-attempt/old-execution custody and reserve before transferring providers.
pub struct DelegatedBodyFragment<'a> {
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    view: c::AssignedPlanView,
    receiver_profile: c::AssignedIdentity,
}
impl<'a> DelegatedBodyFragment<'a> {
    pub fn receiver_profile(&self) -> c::AssignedIdentity {
        self.receiver_profile
    }
    pub fn metadata(&self) -> &BodyFragmentApprovalMetadata<'a> {
        &self.metadata
    }
    /// Own the exact Wake identity already checked in this opaque fragment.
    /// This copies provenance only; it supplies no resource or execution grant.
    pub fn wake_id(&self) -> crate::WakeId {
        crate::WakeId::bound(self.metadata.wake.into())
    }
    pub fn projection(&self) -> &'a [u8] {
        self.projection
    }
    pub fn view(&self) -> c::AssignedPlanView {
        self.view
    }
    /// Use only after Core validates the candidate records against this header.
    /// Its digest pins every table byte, not just matching IDs/counts.
    pub fn matches_projection_header(&self, header: &[u8]) -> bool {
        self.projection.get(..c::ASSIGNED_PLAN_HEADER_BYTES)
            == header.get(..c::ASSIGNED_PLAN_HEADER_BYTES)
    }
}

pub fn receive_delegated_body_fragment<'a>(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'_, 'a>,
    profile: AssignedBodyPreparationProfile<'_>,
) -> Result<DelegatedBodyFragment<'a>, DelegatedFragmentRefusal> {
    use DelegatedFragmentRefusal as E;
    let actual = context.actual;
    let member = context.member;
    check_host_preparation_delegation(installed, context, &command).map_err(E::Policy)?;
    let policy = installed.expect("checked installed delegation");
    let (metadata, projection) =
        decode_body_fragment_document(command.payload()).map_err(E::Document)?;
    finish_receive(metadata, projection, actual, member, policy, profile)
}

pub(super) fn finish_receive<'a>(
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    actual: &c::HostAdvertisement,
    member: &crate::MembershipCredential,
    policy: &HostPreparationDelegation,
    profile: AssignedBodyPreparationProfile<'_>,
) -> Result<DelegatedBodyFragment<'a>, DelegatedFragmentRefusal> {
    finish_checked(
        metadata,
        projection,
        actual,
        member.body_id.as_str(),
        policy.maximum_assignment_bytes,
        policy.maximum_runtime_state_bytes,
        profile,
    )
}

// Shared validation below attribution. Callers must retain their ORIGINAL
// authenticated origin; these private inputs cannot construct an approval.
#[allow(clippy::too_many_arguments)]
pub(super) fn finish_checked<'a>(
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    actual: &c::HostAdvertisement,
    body: &str,
    maximum_assignment_bytes: u32,
    maximum_runtime_state_bytes: u32,
    profile: AssignedBodyPreparationProfile<'_>,
) -> Result<DelegatedBodyFragment<'a>, DelegatedFragmentRefusal> {
    finish_checked_with(
        metadata,
        projection,
        actual,
        body,
        maximum_assignment_bytes,
        maximum_runtime_state_bytes,
        profile,
        |profile| match profile.fore {
            Some(fore) => c::decode_assigned_plan_with_fore(
                projection,
                profile.maxima,
                profile.requirements,
                profile.computations,
                fore,
            ),
            None => c::decode_assigned_plan_with_computations(
                projection,
                profile.maxima,
                profile.requirements,
                profile.computations,
            ),
        },
    )
}

// Only representation selection varies. Attribution stays at the public
// receiving entrances; every decoder shares these exact scope/resource checks.
#[allow(clippy::too_many_arguments)]
pub(super) fn finish_checked_with<'a>(
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    actual: &c::HostAdvertisement,
    body: &str,
    maximum_assignment_bytes: u32,
    maximum_runtime_state_bytes: u32,
    mut profile: AssignedBodyPreparationProfile<'_>,
    decode: impl FnOnce(
        &AssignedBodyPreparationProfile<'_>,
    ) -> Result<c::AssignedPlanView, c::AssignedPlanRefusal>,
) -> Result<DelegatedBodyFragment<'a>, DelegatedFragmentRefusal> {
    use DelegatedFragmentRefusal as E;
    if metadata.body != body
        || metadata.host != actual.host_id.as_str()
        || metadata.boot != actual.boot_id.as_str()
        || metadata.offer_generation != actual.offer_generation.0
    {
        return Err(E::Scope);
    }
    let (fore_records, tail) = metadata
        .fore_ports
        .as_chunks::<{ c::ASSIGNED_LOCAL_FORE_BYTES }>();
    let fore_matches = tail.is_empty()
        && match profile.fore {
            None => fore_records.is_empty(),
            Some(fore) => {
                fore.len() == fore_records.len()
                    && fore
                        .iter()
                        .zip(fore_records)
                        .all(|(pin, bytes)| pin.payload() == bytes)
            }
        };
    if !fore_matches {
        return Err(E::ForeApproval);
    }
    // Policy may narrow an actual profile, never silently enlarge its grants.
    profile.maxima.encoded_bytes =
        u32::from(profile.maxima.encoded_bytes).min(maximum_assignment_bytes) as u16;
    profile.maxima.runtime_state_bytes = profile
        .maxima
        .runtime_state_bytes
        .min(maximum_runtime_state_bytes);
    let view = decode(&profile).map_err(E::Assignment)?;
    let id = c::AssignedIdentity::from_text;
    if view.plan != id(metadata.plot_plan)
        || view.fragment != id(metadata.fragment)
        || view.host != id(metadata.host)
        || view.boot != id(metadata.boot)
    {
        return Err(E::Scope);
    }
    let prepared = DelegatedBodyFragment {
        metadata,
        projection,
        view,
        receiver_profile: c::AssignedIdentity::from_text(actual.profile.as_str()),
    };
    prepared
        .check_plain_resource_projection()
        .map_err(E::Resources)?;
    Ok(prepared)
}

// Explicit CPU resource-bridge fixtures; these do not prove authenticated ingress.
#[cfg(test)]
#[path = "resources/resource_tests.rs"]
mod resource_tests;
