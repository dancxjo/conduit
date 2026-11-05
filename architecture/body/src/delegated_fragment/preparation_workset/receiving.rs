//! One original attribution/deadline covers every fragment in the local set.
use super::*;
use crate::{
    check_host_workset_preparation_delegation, AssignedBodyPreparationProfile,
    AttributedHostCommand, DelegatedBodyFragment, DelegatedFragmentRefusal,
    HostPreparationDelegation, NativePreparationContext, PreparationDelegationRefusal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelegatedWorksetRefusal {
    Policy(PreparationDelegationRefusal),
    Document(BodyWorksetDocumentRefusal),
    Index,
    Scope,
    Fragment(DelegatedFragmentRefusal),
}

/// Complete signed local set, not complete profile validation or admission.
/// Not clonable/deserializable. Child approval rechecks this original command;
/// no new signed child command, nonce, lifetime or authority is manufactured.
pub struct DelegatedBodyWorkset<'context, 'payload, const N: usize> {
    command: AttributedHostCommand<'context, 'payload>,
    documents: WorksetDocuments<'payload, N>,
}
pub fn receive_delegated_body_workset<'context, 'payload, const N: usize>(
    installed: Option<&HostPreparationDelegation>,
    context: NativePreparationContext<'_>,
    command: AttributedHostCommand<'context, 'payload>,
) -> Result<DelegatedBodyWorkset<'context, 'payload, N>, DelegatedWorksetRefusal> {
    check_host_workset_preparation_delegation(installed, context, &command)
        .map_err(DelegatedWorksetRefusal::Policy)?;
    let documents =
        WorksetDocuments::decode(command.payload()).map_err(DelegatedWorksetRefusal::Document)?;
    if documents.declared_runtime_state_bytes
        > installed
            .expect("checked installed delegation")
            .maximum_runtime_state_bytes
    {
        return Err(DelegatedWorksetRefusal::Policy(
            PreparationDelegationRefusal::Bounds,
        ));
    }
    let (metadata, _) =
        decode_body_fragment_document(documents.document(0).expect("nonempty checked set"))
            .map_err(|e| {
                DelegatedWorksetRefusal::Document(BodyWorksetDocumentRefusal::Fragment(e))
            })?;
    if metadata.body != context.member.body_id.as_str()
        || metadata.host != context.actual.host_id.as_str()
        || metadata.boot != context.actual.boot_id.as_str()
        || metadata.offer_generation != context.actual.offer_generation.0
    {
        return Err(DelegatedWorksetRefusal::Scope);
    }
    Ok(DelegatedBodyWorkset { command, documents })
}
impl<'payload, const N: usize> DelegatedBodyWorkset<'_, 'payload, N> {
    /// Declared structural bytes only, not total funded execution storage.
    pub fn declared_runtime_state_bytes(&self) -> u32 {
        self.documents.declared_runtime_state_bytes
    }
    pub fn fragment_count(&self) -> usize {
        self.documents.count
    }

    /// Public bytes only, for independent initialized-inventory/profile lookup.
    pub fn document(&self, index: usize) -> Option<&'payload [u8]> {
        self.documents.document(index)
    }

    /// Approve only a child of this exact signed complete set. Root still needs
    /// every child's full local profile, complete storage and atomic resources
    /// before any provider transfer or start. An individual result is not that.
    pub fn receive_fragment_with_approved_fore<const PORTS: usize>(
        &self,
        index: usize,
        installed: Option<&HostPreparationDelegation>,
        context: NativePreparationContext<'_>,
        profile: AssignedBodyPreparationProfile<'_>,
    ) -> Result<
        (
            DelegatedBodyFragment<'payload>,
            ApprovedBodyForePorts<PORTS>,
        ),
        DelegatedWorksetRefusal,
    > {
        use DelegatedWorksetRefusal as E;
        check_host_workset_preparation_delegation(installed, context, &self.command)
            .map_err(E::Policy)?;
        if self.documents.declared_runtime_state_bytes
            > installed
                .expect("checked installed delegation")
                .maximum_runtime_state_bytes
        {
            return Err(E::Policy(PreparationDelegationRefusal::Bounds));
        }
        let bytes = self.documents.document(index).ok_or(E::Index)?;
        let (metadata, projection) = decode_body_fragment_document(bytes)
            .map_err(|e| E::Document(BodyWorksetDocumentRefusal::Fragment(e)))?;
        approved_fore::finish_approved_fore(
            metadata,
            projection,
            installed.expect("checked installed delegation"),
            context,
            profile,
        )
        .map_err(E::Fragment)
    }
}
