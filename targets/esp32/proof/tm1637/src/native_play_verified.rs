//! Bind actual owners while verified metadata exists; materialize only after retirement.
use super::*;
use conduit_plan_lowering::selected_operation::SelectedDirectOperationPlan;
impl NativePlay {
    pub fn install_verified_owned(
        &mut self,
        prepared: crate::network_planning::PreparedVerifiedNetworkSetup,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
    ) -> Result<(), ()> {
        self.install_verified_owned_held(prepared, source, join, issuer)?;
        self.release_held()
    }
    pub fn install_verified_owned_held(
        &mut self,
        prepared: crate::network_planning::PreparedVerifiedNetworkSetup,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
    ) -> Result<(), ()> {
        let parts = self.bind_verified_held(&prepared, source, join, issuer)?;
        drop(prepared);
        self.materialize_held(parts)
    }
    fn bind_verified_held(
        &mut self,
        prepared: &crate::network_planning::PreparedVerifiedNetworkSetup,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
    ) -> Result<n::PreparedNetworkKernelParts, ()> {
        if self.attempted {
            return Err(());
        }
        self.attempted = true;
        self.configuration_revision = crate::native_credentials::committed_revision();
        self.phase = "installation-refused";
        let plan = prepared.metadata();
        let fragment = &plan.preparation_metadata().fragments[0];
        let tables = prepared.tables().map_err(|_| ())?;
        let roles = n::PreparedNetworkKernelParts::check_direct(&tables).map_err(|_| ())?;
        let sign = &fragment.placements[usize::from(roles.attachment_sign.0)];
        if sign.capability_id.as_str() != CAP
            || sign.implementation_id.as_str() != IMPLEMENTATION
            || sign.artifact_id.as_str() != ARTIFACT
            || sign.execution_profile_id.as_str() != conduit_net::NETWORK_ATTACHMENT_LOCAL_PROFILE
            || sign.host_calls.len() != 1
            || sign.host_calls[0].contract_id.as_str()
                != conduit_net::NETWORK_ATTACHMENT_SIGN_HOST_CALL
        {
            return Err(());
        }
        let active = issuer.reserve_verified_network_play(plan, &fragment.fragment_id)?;
        let (table, handle, claim) = issuer.issue_verified_network(
            source.as_ref().ok_or(())?.native_identity(),
            Operation::Credentials,
            plan,
            &fragment.fragment_id,
            &active,
        )?;
        self.source = Some(
            ReadyCredentialBase::bind_direct_retained(
                source,
                table,
                handle,
                claim,
                &n::PreparedCredentialContract::prepare(),
                SelectedDirectOperationPlan {
                    tables: &tables,
                    active: &active,
                    placement_id: &fragment.placements[usize::from(roles.credentials.0)]
                        .placement_id,
                },
            )
            .map_err(|_| ())?,
        );
        let (table, handle, claim) = issuer.issue_verified_network(
            join.as_ref().ok_or(())?.native_identity(),
            Operation::Join,
            plan,
            &fragment.fragment_id,
            &active,
        )?;
        self.join = Some(
            ReadyNetworkJoinBase::bind_direct_retained(
                join,
                table,
                handle,
                claim,
                &n::PreparedJoinContract::prepare(),
                SelectedDirectOperationPlan {
                    tables: &tables,
                    active: &active,
                    placement_id: &fragment.placements[usize::from(roles.join.0)].placement_id,
                },
            )
            .map_err(|_| ())?,
        );
        self.active = Some(active);
        n::PreparedNetworkKernelParts::from_direct(tables).map_err(|_| ())
    }
}
