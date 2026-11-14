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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionDelegationRefusal {
    MissingDelegation,
    Bounds,
    Capacity,
    Preparation(PreparationDelegationRefusal),
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
    /// Check explicit execution intent against this same original signed set.
    /// Root supplies its actual active-workset count, never a carrier claim.
    /// This check admits no storage/resources or native possession, and does
    /// not refresh the command or permit an unchecked child fragment to Start.
    pub fn check_execution_delegation(
        &self,
        execution: Option<&crate::HostExecutionDelegation>,
        preparation: Option<&HostPreparationDelegation>,
        context: NativePreparationContext<'_>,
        active_worksets: u16,
    ) -> Result<(), ExecutionDelegationRefusal> {
        use ExecutionDelegationRefusal as E;
        let execution = execution.ok_or(E::MissingDelegation)?;
        if execution.maximum_active_worksets == 0 {
            return Err(E::Bounds);
        }
        match execution.authorizer {
            crate::HostExecutionAuthorizer::SeedOwner => {
                // Reuses the exact pinned Seed issuer and construction/member/
                // Host/Boot/profile/generation/contract/deadline checks.
                check_host_workset_preparation_delegation(preparation, context, &self.command)
                    .map_err(E::Preparation)?;
            }
        }
        if active_worksets >= execution.maximum_active_worksets {
            return Err(E::Capacity);
        }
        Ok(())
    }
    /// Retain only the original attribution after local checks. This consumes
    /// the complete-set receiver; it issues no new child command or authority.
    pub fn into_retained_stamp(
        self,
    ) -> Result<crate::RetainedHostCommandStamp, crate::HostCommandRetentionRefusal> {
        self.command.into_retained_stamp()
    }
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

    /// Validate a child using the receiver's independently installed Fore ABI.
    /// The signed complete set cannot substitute different local endpoints.
    pub fn receive_fragment_with_pinned_fore<const PORTS: usize>(
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
                .expect("checked delegation")
                .maximum_runtime_state_bytes
        {
            return Err(E::Policy(PreparationDelegationRefusal::Bounds));
        }
        let bytes = self.documents.document(index).ok_or(E::Index)?;
        let (metadata, projection) = decode_body_fragment_document(bytes)
            .map_err(|e| E::Document(BodyWorksetDocumentRefusal::Fragment(e)))?;
        approved_fore::finish_pinned_fore(
            metadata,
            projection,
            installed.expect("checked delegation"),
            context,
            profile,
        )
        .map_err(E::Fragment)
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
    /// Explicit schema-8 child preparation under this SAME complete-set command
    /// and deadline. Root retains independent Line pins and carrier authority;
    /// the returned descriptors approve only exact preparation representation.
    pub fn receive_fragment_with_line_fore<const PORTS: usize>(
        &self,
        index: usize,
        installed: Option<&HostPreparationDelegation>,
        context: NativePreparationContext<'_>,
        profile: crate::AssignedBodyLineForePreparationProfile<'_>,
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
        let policy = installed.expect("checked installed delegation");
        if self.documents.declared_runtime_state_bytes > policy.maximum_runtime_state_bytes {
            return Err(E::Policy(PreparationDelegationRefusal::Bounds));
        }
        let bytes = self.documents.document(index).ok_or(E::Index)?;
        let (metadata, projection) = decode_body_fragment_document(bytes)
            .map_err(|e| E::Document(BodyWorksetDocumentRefusal::Fragment(e)))?;
        line_fore::finish_line_fore(metadata, projection, policy, context, profile)
            .map_err(E::Fragment)
    }
}

#[path = "installed_receiving.rs"]
mod installed_resources;
