//! Recheck a signed preparation attempt against installed explicit delegation.
use super::{HostPreparationAuthorizer, HostPreparationDelegation};
use crate::{AttributedHostCommand, MemberBootstrap, MembershipCredential};
use conduit_core::HostAdvertisement;

pub const HOST_PREPARED_FRAGMENT_CONTRACT: &str = "conduit.body/prepared-fragment@1";
pub const HOST_PREPARED_WORKSET_CONTRACT: &str = "conduit.body/prepared-workset@1";

/// Independently retained current Root facts, never supplied by the command.
/// Root must already have verified membership and installed the exact policy
/// from the checked configuration identified by its compiled image/config IDs.
#[derive(Clone, Copy)]
pub struct NativePreparationContext<'a> {
    pub bootstrap: &'a MemberBootstrap<'a>,
    pub member: &'a MembershipCredential,
    pub actual: &'a HostAdvertisement,
    pub image_id: &'a str,
    pub configuration_id: &'a str,
    pub now_millis: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationDelegationRefusal {
    MissingDelegation,
    Bounds,
    Construction,
    Membership,
    Receiver,
    Issuer,
    Contract,
    Clock,
}

/// Permission to consider this preparation attempt, not approval of its Plan.
/// Check before decoding. Root must then validate the exact approved projection
/// using policy ceilings, reserve actual resources and obtain local native
/// capability possession before any provider transfer or kernel start.
/// No lease, admission, membership change, effect or acknowledgement is issued.
pub fn check_host_preparation_delegation(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: &AttributedHostCommand<'_, '_>,
) -> Result<(), PreparationDelegationRefusal> {
    check_contract(installed, context, command, HOST_PREPARED_FRAGMENT_CONTRACT)
}

/// Check only attribution and installed receive/preparation policy for a
/// payload declared as a workset. This does not decode or approve the workset, reserve
/// resources, issue native possession or authorize Start.
pub fn check_host_workset_preparation_delegation(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: &AttributedHostCommand<'_, '_>,
) -> Result<(), PreparationDelegationRefusal> {
    check_contract(installed, context, command, HOST_PREPARED_WORKSET_CONTRACT)
}

fn check_contract(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: &AttributedHostCommand<'_, '_>,
    expected_contract: &str,
) -> Result<(), PreparationDelegationRefusal> {
    use PreparationDelegationRefusal as E;
    let policy = installed.ok_or(E::MissingDelegation)?;
    if !policy.has_finite_bounds()
        || command.payload().len() as u64 > u64::from(policy.maximum_assignment_bytes)
        || command.challenge().maximum_payload_bytes > policy.maximum_assignment_bytes
    {
        return Err(E::Bounds);
    }
    let NativePreparationContext {
        bootstrap,
        member,
        actual,
        image_id,
        configuration_id,
        now_millis,
    } = context;
    if !bootstrap.matches_native(actual.host_id.as_str(), image_id, configuration_id) {
        return Err(E::Construction);
    }
    let challenge = command.challenge();
    if bootstrap.body_id != member.body_id
        || member.host_id != actual.host_id
        || member.boot_id != actual.boot_id
        || challenge.body_id != member.body_id.as_str()
        || challenge.part_id != member.part_id.as_str()
        || challenge.credential_id != member.credential_id.as_str()
    {
        return Err(E::Membership);
    }
    if actual.protocol_version != conduit_core::PROTOCOL_VERSION
        || actual.offer_generation.0 == 0
        || challenge.receiver_host != actual.host_id.as_str()
        || challenge.receiver_boot != actual.boot_id.as_str()
        || challenge.receiver_generation != actual.offer_generation.0
        || challenge.receiver_profile != actual.profile.as_str()
    {
        return Err(E::Receiver);
    }
    let owner = match policy.authorizer {
        HostPreparationAuthorizer::SeedOwner => &bootstrap.owner,
    };
    if challenge.issuer_host != owner.host_id.as_str()
        || challenge.issuer_boot != owner.boot_id.as_str()
        || challenge.issuer_generation != owner.offer_generation.0
        || challenge.issuer_key != owner.verifying_key
    {
        return Err(E::Issuer);
    }
    if challenge.payload_contract != expected_contract {
        return Err(E::Contract);
    }
    if now_millis < challenge.issued_at_millis || now_millis >= challenge.expires_at_millis {
        return Err(E::Clock);
    }
    Ok(())
}
