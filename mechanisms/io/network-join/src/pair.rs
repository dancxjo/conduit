//! Affine initialized owners and a generative checked-binding handoff.
use crate::{credential_installation::ReadyCredentialBase, installation::ReadyNetworkJoinBase, *};
use conduit_core::{
    ActivePlayIdentity, BaseCapabilityHandle, BaseCapabilityTable, BaseOperationClaim,
};
use conduit_plan_lowering::{
    local_tables::FlatDirectLocalKernelTables, selected_operation::SelectedFlatDirectOperationPlan,
};
use core::marker::PhantomData;
type Brand<'a> = PhantomData<fn(&'a mut ()) -> &'a mut ()>;
/// Successful checked transfer of both actual initialized providers.
pub type PreparedNetworkOwners<P, S, R> =
    (NetworkCredentialsHostCall<S>, NetworkJoinHostCall<P>, R);
/// Every refusal returns the original owner pair and caller's evidence custody.
pub type NetworkPairPreparation<P, S, R, E> =
    Result<PreparedNetworkOwners<P, S, R>, (ReadyNetworkPair<P, S>, E)>;
/// Actual issued custody; constructing this record does not validate it.
pub struct NetworkOperationLease {
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
}
pub struct ReadyNetworkPair<P, S> {
    credentials: ReadyCredentialBase<S>,
    join: ReadyNetworkJoinBase<P>,
    credential_contract: conduit_net::CheckedNetworkContract,
    join_contract: conduit_net::CheckedNetworkContract,
}
pub struct ReadyNetworkPairView<'a, P, S> {
    pair: &'a ReadyNetworkPair<P, S>,
    brand: Brand<'a>,
}
pub struct PreparedNetworkPairBindings<'a> {
    credentials: conduit_core::BoundBaseOperation,
    join: conduit_core::BoundBaseOperation,
    credential_node: conduit_kernel::NodeId,
    join_node: conduit_kernel::NodeId,
    brand: Brand<'a>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum NetworkPairRefusal {
    Credentials(CredentialCallRefusal),
    Join(JoinCallRefusal),
}
impl<'a> PreparedNetworkPairBindings<'a> {
    /// Return every issued lease if a later kernel/allocation stage refuses.
    pub fn into_operations(
        self,
    ) -> (
        conduit_core::BoundBaseOperation,
        conduit_core::BoundBaseOperation,
    ) {
        (self.credentials, self.join)
    }
}
impl<P: NetworkJoinProvider, S: CredentialSource> ReadyNetworkPair<P, S> {
    pub fn new(credentials: ReadyCredentialBase<S>, join: ReadyNetworkJoinBase<P>) -> Self {
        let c = credentials.native_identity();
        let j = join.native_identity();
        let credential_contract = conduit_net::PreparedCanonicalNetworkOffer::credentials(
            credential_installation::CREDENTIAL_CAPABILITY,
            c.implementation.as_str(),
            c.artifact.as_str(),
        )
        .into_contract();
        let join_contract = conduit_net::PreparedCanonicalNetworkOffer::join(
            installation::JOIN_CAPABILITY,
            j.implementation.as_str(),
            j.artifact.as_str(),
        )
        .into_contract();
        Self {
            credentials,
            join,
            credential_contract,
            join_contract,
        }
    }
    /// The callback cannot export the private owner brand or substitute a binding
    /// checked against a different pair. No provider is moved before success.
    #[allow(
        clippy::result_large_err,
        reason = "Refusal returns unique owners and issued evidence without another allocation."
    )]
    pub fn prepare<R, E>(
        self,
        check: impl for<'a> FnOnce(
            ReadyNetworkPairView<'a, P, S>,
        ) -> Result<(PreparedNetworkPairBindings<'a>, R), E>,
    ) -> NetworkPairPreparation<P, S, R, E> {
        let result = check(ReadyNetworkPairView {
            pair: &self,
            brand: PhantomData,
        });
        let (checked, result) = match result {
            Ok(value) => value,
            Err(error) => return Err((self, error)),
        };
        let PreparedNetworkPairBindings {
            credentials,
            join,
            credential_node,
            join_node,
            brand: _,
        } = checked;
        Ok((
            self.credentials
                .into_checked_call(credentials, credential_node),
            self.join.into_checked_call(join, join_node),
            result,
        ))
    }
    pub fn into_ready(self) -> (ReadyCredentialBase<S>, ReadyNetworkJoinBase<P>) {
        (self.credentials, self.join)
    }
}
impl<'a, P: NetworkJoinProvider, S: CredentialSource> ReadyNetworkPairView<'a, P, S> {
    pub fn credential_identity(&self) -> &'a installation::NetworkNativeIdentity {
        self.pair.credentials.native_identity()
    }
    pub fn join_identity(&self) -> &'a installation::NetworkNativeIdentity {
        self.pair.join.native_identity()
    }
}
impl<'a, P: NetworkJoinProvider, S: CredentialSource> ReadyNetworkPairView<'a, P, S> {
    /// Project the same complete initialized-owner facts as ordinary registry
    /// append. The caller supplies only its empty current Root header and actual
    /// checked pure attachment implementation; no owner strings are cloned.
    pub fn with_advertisement<R>(
        self,
        header: conduit_core::HostAdvertisementRef<'_>,
        attachment: &conduit_net::PreparedCanonicalNetworkOffer<'_>,
        use_advertisement: impl FnOnce(Self, conduit_core::HostAdvertisementRef<'_>) -> R,
    ) -> Result<R, installation::NetworkInstallationRefusal> {
        use conduit_core::*;
        use installation::NetworkInstallationRefusal as E;
        let c = self.credential_identity();
        let j = self.join_identity();
        if header.host_id != c.host.as_str()
            || header.host_id != j.host.as_str()
            || header.boot_id != c.boot.as_str()
            || header.boot_id != j.boot.as_str()
        {
            return Err(E::WrongBoot);
        }
        if !header.bases.is_empty()
            || !header.resources.is_empty()
            || !header.capabilities.is_empty()
            || !header.planner_capabilities.is_empty()
            || c.base == j.base
            || c.instance == j.instance
            || c.resource == j.resource
        {
            return Err(E::Advertisement);
        }
        let credential_requirement = [ResourceRequirementRef {
            class_id: CREDENTIAL_RESOURCE_CLASS,
            units: 1,
            protected_role: None,
            compute: None,
            content: None,
        }];
        let mut credential = self.pair.credential_contract.offer(
            credential_installation::CREDENTIAL_CAPABILITY,
            c.implementation.as_str(),
            c.artifact.as_str(),
        );
        credential.resource_requirements =
            ResourceRequirementRefs::Borrowed(&credential_requirement);
        let join = self.pair.join_contract.offer(
            installation::JOIN_CAPABILITY,
            j.implementation.as_str(),
            j.artifact.as_str(),
        );
        let attachment = attachment.view();
        if [credential.capability_id, join.capability_id].contains(&attachment.capability_id) {
            return Err(E::Advertisement);
        }
        let capabilities = [credential, attachment, join];

        let credential_resources = [resource_advertisement(c, CREDENTIAL_RESOURCE_CLASS)];
        let join_resources = [resource_advertisement(
            j,
            conduit_net::WIFI_STATION_RESOURCE_CLASS,
        )];
        let credential_capability = [credential_installation::CREDENTIAL_CAPABILITY];
        let join_capability = [installation::JOIN_CAPABILITY];
        let credential_pool = [c.resource.as_str()];
        let join_pool = [j.resource.as_str()];
        let mut bases = [
            base_advertisement(
                c,
                credential_installation::CREDENTIAL_BASE_KIND,
                &credential_capability,
                &credential_pool,
            ),
            base_advertisement(
                j,
                installation::JOIN_BASE_KIND,
                &join_capability,
                &join_pool,
            ),
        ];
        for (base, caps, resources) in [
            (bases[0], &capabilities[0..1], &credential_resources),
            (bases[1], &capabilities[2..3], &join_resources),
        ] {
            validate_base_provider_entry_ref(
                BaseProviderEntryRef {
                    base_id: base.base_id,
                    provider_instance_id: base.provider_instance_id,
                    provider_generation: base.provider_generation,
                    implementation_id: base.implementation_id,
                    mechanism_family: base.mechanism_family,
                    enforcement_class: base.enforcement_class,
                    lifecycle: base.lifecycle,
                    capabilities: CapabilityOfferRefs::Borrowed(caps),
                    resources: ResourceOfferRefs::Borrowed(resources),
                },
                BaseRegistryLimits {
                    maximum_bases: 1,
                    maximum_capabilities_per_base: 1,
                    maximum_resources_per_base: 1,
                    maximum_advertised_capabilities: 3,
                    maximum_advertised_resources: 2,
                },
            )
            .map_err(|_| E::Advertisement)?;
        }
        bases.sort_by(|a, b| a.base_id.cmp(b.base_id));
        let mut resources = [credential_resources[0], join_resources[0]];
        resources.sort_by(|a, b| a.pool_id.cmp(b.pool_id));
        Ok(use_advertisement(
            self,
            HostAdvertisementRef {
                bases: BaseProviderAdvertisementRefs::Borrowed(&bases),
                resources: ResourceOfferRefs::Borrowed(&resources),
                capabilities: CapabilityOfferRefs::Borrowed(&capabilities),
                ..header
            },
        ))
    }
}
fn base_advertisement<'a>(
    i: &'a installation::NetworkNativeIdentity,
    family: &'a str,
    capabilities: &'a [&'a str],
    resources: &'a [&'a str],
) -> conduit_core::BaseProviderAdvertisementRef<'a> {
    use conduit_core::*;
    BaseProviderAdvertisementRef {
        base_id: i.base.as_str(),
        provider_instance_id: i.instance.as_str(),
        provider_generation: i.provider_generation,
        implementation_id: i.base_implementation.as_str(),
        mechanism_family: family,
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capability_ids: CapabilityIdRefs::Borrowed(capabilities),
        resource_pool_ids: ResourcePoolIdRefs::Borrowed(resources),
    }
}

fn resource_advertisement<'a>(
    i: &'a installation::NetworkNativeIdentity,
    class_id: &'a str,
) -> conduit_core::ResourceOfferRef<'a> {
    conduit_core::ResourceOfferRef {
        pool_id: i.resource.as_str(),
        class_id,
        capacity_units: 1,
        compute: None,
        content: None,
    }
}

fn compact_lease(lease: NetworkOperationLease) -> conduit_core::BoundBaseOperation {
    conduit_core::BoundBaseOperation::bind(lease.table, lease.handle, lease.claim)
        .unwrap_or_else(|_| panic!("preflighted actual bearer"))
}
/// Exact credential selection retained while Root issues the second operation.
/// The first table remains affine; no permit or request counter is reset.
pub struct PreparedCredentialSelection<'a, P, S> {
    owners: ReadyNetworkPairView<'a, P, S>,
    operation: conduit_core::BoundBaseOperation,
    node: conduit_kernel::NodeId,
}
impl<'a, P: NetworkJoinProvider, S: CredentialSource> ReadyNetworkPairView<'a, P, S> {
    #[allow(
        clippy::result_large_err,
        reason = "Refusal returns unique owners and issued evidence without another allocation."
    )]
    pub fn check_flat_credentials(
        self,
        lease: NetworkOperationLease,
        tables: &FlatDirectLocalKernelTables<'_, 3, 2, 1, 3, 2, 3>,
        active: &ActivePlayIdentity,
        placement: &str,
    ) -> Result<PreparedCredentialSelection<'a, P, S>, (NetworkOperationLease, NetworkPairRefusal)>
    {
        let check = || {
            let gear = tables
                .preparation_scope()
                .view()
                .placements
                .iter()
                .find(|g| g.placement_id == placement)
                .ok_or(NetworkPairRefusal::Credentials(
                    CredentialCallRefusal::WrongRequest,
                ))?;
            self.pair
                .credentials
                .check_selected_native_ref(
                    &lease.claim,
                    gear,
                    self.pair.credential_contract.kind().limits,
                )
                .map_err(NetworkPairRefusal::Credentials)?;
            let node = NetworkCredentialsHostCall::<S>::validate_flat_direct(
                &lease.table,
                &lease.claim,
                &self.pair.credential_contract,
                SelectedFlatDirectOperationPlan {
                    tables,
                    active,
                    placement_id: placement,
                },
            )
            .map_err(NetworkPairRefusal::Credentials)?;
            conduit_core::BoundBaseOperation::check(&lease.table, &lease.handle, &lease.claim)
                .map_err(|e| {
                    NetworkPairRefusal::Credentials(CredentialCallRefusal::Capability(e))
                })?;
            Ok(node)
        };
        match check() {
            Ok(node) => Ok(PreparedCredentialSelection {
                owners: self,
                operation: compact_lease(lease),
                node,
            }),
            Err(error) => Err((lease, error)),
        }
    }
}
impl<'a, P: NetworkJoinProvider, S: CredentialSource> PreparedCredentialSelection<'a, P, S> {
    pub fn join_identity(&self) -> &'a installation::NetworkNativeIdentity {
        self.owners.join_identity()
    }
    pub fn into_operation(self) -> conduit_core::BoundBaseOperation {
        self.operation
    }
    #[allow(
        clippy::result_large_err,
        reason = "Refusal returns unique owners and issued evidence without another allocation."
    )]
    pub fn check_flat_join(
        self,
        lease: NetworkOperationLease,
        tables: &FlatDirectLocalKernelTables<'_, 3, 2, 1, 3, 2, 3>,
        active: &ActivePlayIdentity,
        placement: &str,
    ) -> Result<PreparedNetworkPairBindings<'a>, (Self, NetworkOperationLease, NetworkPairRefusal)>
    {
        let check = || {
            let view = tables.preparation_scope().view();
            let old = &self.operation.inspection().scope;
            if old.plan_id.as_str() != view.plan_id
                || old.active_play_id != active.active_play_id
                || old.host_id != active.host_id
                || old.boot_id != active.boot_id
            {
                return Err(NetworkPairRefusal::Join(JoinCallRefusal::WrongBinding));
            }
            let gear = view
                .placements
                .iter()
                .find(|g| g.placement_id == placement)
                .ok_or(NetworkPairRefusal::Join(JoinCallRefusal::WrongBinding))?;
            self.owners
                .pair
                .join
                .check_selected_native_ref(
                    &lease.claim,
                    gear,
                    self.owners.pair.join_contract.kind().limits,
                )
                .map_err(NetworkPairRefusal::Join)?;
            let node = NetworkJoinHostCall::<P>::validate_flat_direct(
                &lease.table,
                &lease.claim,
                self.owners.pair.join.attachment_generation(),
                &self.owners.pair.join_contract,
                SelectedFlatDirectOperationPlan {
                    tables,
                    active,
                    placement_id: placement,
                },
            )
            .map_err(NetworkPairRefusal::Join)?;
            conduit_core::BoundBaseOperation::check(&lease.table, &lease.handle, &lease.claim)
                .map_err(|e| NetworkPairRefusal::Join(JoinCallRefusal::Capability(e)))?;
            Ok(node)
        };
        match check() {
            Ok(node) => Ok(PreparedNetworkPairBindings {
                credentials: self.operation,
                join: compact_lease(lease),
                credential_node: self.node,
                join_node: node,
                brand: self.owners.brand,
            }),
            Err(error) => Err((self, lease, error)),
        }
    }
}
