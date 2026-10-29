//! Root-owned control Play through the production fixed kernel. No direct
//! radio method is available; only exact selected owners dispatch Host Calls.
#[cfg(feature = "flat-network-appliance")]
#[path = "native_play_flat.rs"]
mod flat;
#[path = "native_network_install.rs"]
mod install_gate;
#[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
#[path = "native_network_journal.rs"]
pub mod journal;
#[path = "native_play_retirement.rs"]
mod retirement;
#[path = "native_play_verified.rs"]
mod verified;
use crate::native_authority::{Operation, RootIssuer};
use conduit_core as c;
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
use conduit_network_join_call::{
    self as n, credential_installation::ReadyCredentialBase, installation::ReadyNetworkJoinBase,
    PreparedNetworkKernel,
};
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;
pub use retirement::RetiredNetworkControl;
type Source = crate::native_credentials::RootCredentialSource;
type Provider = crate::native_network::Provider;
const CAP: &str = "conduit/esp32/network-attachment-receipt@1";
const IMPLEMENTATION: &str = "esp32/network-attachment-receipt@1";
const ARTIFACT: &str = "native/esp32-network-attachment-receipt@1";
#[path = "native_play_execution.rs"]
mod execution;
pub use execution::NativePlay;
impl NativePlay {
    /// Every fallible transfer uses retained slots. A partial installation stays
    /// owned here on refusal and never starts a partially selected kernel.
    pub fn install(
        &mut self,
        prepared: &crate::network_planning::PreparedNetworkSetup,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
    ) -> Result<(), ()> {
        self.install_held(prepared, source, join, issuer)?;
        self.release_held()
    }
    /// Select and retain the exact kernel and actual owners, without making
    /// execution runnable. Root must establish independent capacity/current
    /// scope admission before explicit release. Refusal keeps all custody here.
    pub fn install_held(
        &mut self,
        prepared: &crate::network_planning::PreparedNetworkSetup,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
    ) -> Result<(), ()> {
        let parts = self.bind_held(prepared, source, join, issuer)?;
        self.materialize_held(parts)
    }
    /// Transfer and retire all rich construction metadata before full storage.
    pub fn install_owned_held(
        &mut self,
        prepared: crate::network_planning::PreparedNetworkSetup,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
    ) -> Result<(), ()> {
        let parts = self.bind_held(&prepared, source, join, issuer)?;
        drop(prepared);
        self.materialize_held(parts)
    }
    pub fn install_owned(
        &mut self,
        prepared: crate::network_planning::PreparedNetworkSetup,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
    ) -> Result<(), ()> {
        self.install_owned_held(prepared, source, join, issuer)?;
        self.release_held()
    }
    fn materialize_held(&mut self, parts: n::PreparedNetworkKernelParts) -> Result<(), ()> {
        self.prepared = Some(alloc::boxed::Box::new(parts.finish().map_err(|_| ())?));
        self.phase = "installed-held";
        Ok(())
    }
    fn bind_held(
        &mut self,
        prepared: &crate::network_planning::PreparedNetworkSetup,
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
        let fragment = &prepared.plan().fragments[0];
        let lowered = prepared.lowered();
        let parts = prepared.kernel_parts().map_err(|_| ())?;
        let role = parts.roles();
        let sign = &fragment
            .placements
            .iter()
            .find(|g| {
                g.placement_id == lowered.nodes[usize::from(role.attachment_sign.0)].placement_id
            })
            .ok_or(())?;
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
        let active = issuer.reserve_play(fragment)?;
        let (table, handle, claim) = issuer.issue(
            source.as_ref().ok_or(())?.native_identity(),
            Operation::Credentials,
            fragment,
            &active,
        )?;
        self.source = Some(
            ReadyCredentialBase::bind_selected_retained(
                source,
                table,
                handle,
                claim,
                &n::PreparedCredentialContract::prepare(),
                SelectedOperationPlan {
                    fragment,
                    lowered,
                    active: &active,
                    placement_id: &lowered.nodes[usize::from(role.credentials.0)].placement_id,
                },
            )
            .map_err(|_| ())?,
        );
        let (table, handle, claim) = issuer.issue(
            join.as_ref().ok_or(())?.native_identity(),
            Operation::Join,
            fragment,
            &active,
        )?;
        self.join = Some(
            ReadyNetworkJoinBase::bind_selected_retained(
                join,
                table,
                handle,
                claim,
                &n::PreparedJoinContract::prepare(),
                SelectedOperationPlan {
                    fragment,
                    lowered,
                    active: &active,
                    placement_id: &lowered.nodes[usize::from(role.join.0)].placement_id,
                },
            )
            .map_err(|_| ())?,
        );
        self.active = Some(active);
        Ok(parts)
    }
    /// This scheduling release does not grant authority or attest capacity.
    pub fn release_held(&mut self) -> Result<(), ()> {
        let complete = self.installed();
        install_gate::release(&mut self.phase, complete)
    }
}
