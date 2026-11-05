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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelegatedFragmentRefusal {
    Policy(PreparationDelegationRefusal),
    Document(BodyFragmentDocumentRefusal),
    Scope,
    Assignment(c::AssignedPlanRefusal),
}

/// A checked delegated preparation, not a reservation, native capability or Play.
/// Not constructible/deserializable from public metadata. Root must retain its
/// one-attempt/old-execution custody and reserve before transferring providers.
pub struct DelegatedBodyFragment<'a> {
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    view: c::AssignedPlanView,
}
impl<'a> DelegatedBodyFragment<'a> {
    pub fn metadata(&self) -> &BodyFragmentApprovalMetadata<'a> {
        &self.metadata
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
    mut profile: AssignedBodyPreparationProfile<'_>,
) -> Result<DelegatedBodyFragment<'a>, DelegatedFragmentRefusal> {
    use DelegatedFragmentRefusal as E;
    let actual = context.actual;
    let member = context.member;
    check_host_preparation_delegation(installed, context, &command).map_err(E::Policy)?;
    let policy = installed.expect("checked installed delegation");
    let (metadata, projection): (BodyFragmentApprovalMetadata<'a>, _) =
        postcard::take_from_bytes(command.payload())
            .map_err(|_| E::Document(BodyFragmentDocumentRefusal::Metadata))?;
    metadata.check().map_err(E::Document)?;
    if metadata.body != member.body_id.as_str()
        || metadata.host != actual.host_id.as_str()
        || metadata.boot != actual.boot_id.as_str()
        || metadata.offer_generation != actual.offer_generation.0
    {
        return Err(E::Scope);
    }
    if projection.len() as u64 != u64::from(metadata.projection_length)
        || Sha256::digest(projection).as_slice() != metadata.projection_digest
    {
        return Err(E::Document(BodyFragmentDocumentRefusal::Projection));
    }
    // Policy may narrow an actual profile, never silently enlarge its grants.
    profile.maxima.encoded_bytes =
        u32::from(profile.maxima.encoded_bytes).min(policy.maximum_assignment_bytes) as u16;
    profile.maxima.runtime_state_bytes = u32::from(profile.maxima.runtime_state_bytes)
        .min(policy.maximum_runtime_state_bytes) as u16;
    let view = match profile.fore {
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
    }
    .map_err(E::Assignment)?;
    let id = c::AssignedIdentity::from_text;
    if view.plan != id(metadata.plot_plan)
        || view.fragment != id(metadata.fragment)
        || view.host != id(metadata.host)
        || view.boot != id(metadata.boot)
    {
        return Err(E::Scope);
    }
    Ok(DelegatedBodyFragment {
        metadata,
        projection,
        view,
    })
}
