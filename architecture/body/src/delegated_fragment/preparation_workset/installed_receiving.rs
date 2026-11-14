//! Resolve only an authenticated child under independently installed plain rows.
use super::*;
impl<'payload, const N: usize> DelegatedBodyWorkset<'_, 'payload, N> {
    /// The original whole-set signature/deadline and execution delegation remain
    /// separate. This profile requires pinned Fore and schema4, with no fallback
    /// to author-approved Fore, Line-Fore, continuation or inline metadata.
    pub fn receive_fragment_with_installed_plain_resources<const PORTS: usize>(
        &self,
        index: usize,
        installed: Option<&HostPreparationDelegation>,
        context: NativePreparationContext<'_>,
        profile: crate::AssignedBodyInstalledPlainResourcesProfile<'_>,
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
        super::super::super::installed_receiving::finish(
            metadata,
            projection,
            installed.expect("checked delegation"),
            context,
            profile,
        )
        .map_err(E::Fragment)
    }
}
