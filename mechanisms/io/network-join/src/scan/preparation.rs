//! Affine handoff mirroring the ordinary network pair: descriptions retire
//! before the uniquely retained provider moves into its selected operation.
use super::*;
use conduit_plan_lowering::selected_operation::SelectedFlatDirectOperationPlan;
use core::marker::PhantomData;
type Brand<'a> = PhantomData<fn(&'a mut ()) -> &'a mut ()>;
pub type ScanPreparation<P, R, E> =
    Result<(NetworkScanHostCall<P>, R), (ReadyNetworkScanBase<P>, E)>;
pub struct ReadyScanView<'a, P> {
    pub(super) ready: &'a ReadyNetworkScanBase<P>,
    pub(super) brand: Brand<'a>,
}
#[must_use = "retain the actual issued operation on a later preparation refusal"]
pub struct PreparedScanBinding<'a> {
    pub(super) operation: BoundBaseOperation,
    pub(super) node: NodeId,
    pub(super) brand: Brand<'a>,
}
impl PreparedScanBinding<'_> {
    /// Exact actual possession for a caller's terminal-refusal custody. This
    /// strips no scope, revokes nothing and cannot reset an operation budget.
    pub fn into_operation(self) -> BoundBaseOperation {
        self.operation
    }
    pub fn node(&self) -> NodeId {
        self.node
    }
}
impl<P: NetworkScanProvider> ReadyNetworkScanBase<P> {
    /// A callback can return a binding only for this same generative owner
    /// borrow. Failed preparation returns the actual Ready owner, not a copy.
    #[allow(
        clippy::result_large_err,
        reason = "Refusal returns the original affine provider without allocating"
    )]
    pub fn prepare<R, E>(
        self,
        check: impl for<'a> FnOnce(ReadyScanView<'a, P>) -> Result<(PreparedScanBinding<'a>, R), E>,
    ) -> ScanPreparation<P, R, E> {
        let checked = match check(ReadyScanView {
            ready: &self,
            brand: PhantomData,
        }) {
            Ok(value) => value,
            Err(error) => return Err((self, error)),
        };
        let (
            PreparedScanBinding {
                operation, node, ..
            },
            result,
        ) = checked;
        Ok((self.into_checked_call(operation, node), result))
    }
}
impl<'a, P: NetworkScanProvider> ReadyScanView<'a, P> {
    pub fn native_identity(&self) -> &'a NetworkNativeIdentity {
        &self.ready.identity
    }
    #[allow(
        clippy::result_large_err,
        reason = "Refusal returns every original issued bearer without allocating"
    )]
    pub fn check_flat(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selected: SelectedFlatDirectOperationPlan<'_, 2, 1, 1, 2, 1, 2>,
    ) -> Result<PreparedScanBinding<'a>, ScanBindingFailure> {
        let node = match self
            .ready
            .check_flat_binding(&table, &handle, &claim, selected)
        {
            Ok(node) => node,
            Err(error) => return Err((error, table, handle, claim)),
        };
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .unwrap_or_else(|_| unreachable!("same exclusive preflighted scan possession"));
        Ok(PreparedScanBinding {
            operation,
            node,
            brand: self.brand,
        })
    }
    /// Describe the actual initialized provider and a canonical pure evidence
    /// sink without cloning identity strings or constructing owned offers.
    /// Descriptions cannot escape the callback or transfer the provider.
    pub fn with_advertisement<R>(
        self,
        header: HostAdvertisementRef<'_>,
        evidence: &PreparedCanonicalNetworkOffer<'_>,
        use_advertisement: impl for<'description> FnOnce(
            ReadyScanView<'a, P>,
            HostAdvertisementRef<'description>,
        ) -> R,
    ) -> Result<R, ScanCallRefusal> {
        let i = self.native_identity();
        if header.host_id != i.host.as_str()
            || header.boot_id != i.boot.as_str()
            || !header.bases.is_empty()
            || !header.resources.is_empty()
            || !header.capabilities.is_empty()
            || !header.planner_capabilities.is_empty()
        {
            return Err(ScanCallRefusal::WrongBinding);
        }
        let scan = PreparedCanonicalNetworkOffer::scan(
            SCAN_CAPABILITY,
            i.implementation.as_str(),
            i.artifact.as_str(),
        );
        let scan = [scan.view()];
        let evidence = evidence.view();
        if evidence.kind_id != NETWORK_SCAN_SIGN_OPERATION
            || evidence.capability_id == SCAN_CAPABILITY
        {
            return Err(ScanCallRefusal::WrongBinding);
        }
        let resources = [crate::pair::resource_advertisement(
            i,
            WIFI_STATION_RESOURCE_CLASS,
        )];
        let capabilities = [SCAN_CAPABILITY];
        let pools = [i.resource.as_str()];
        let base = crate::pair::base_advertisement(i, SCAN_BASE_KIND, &capabilities, &pools);
        validate_base_provider_entry_ref(
            BaseProviderEntryRef {
                base_id: base.base_id,
                provider_instance_id: base.provider_instance_id,
                provider_generation: base.provider_generation,
                implementation_id: base.implementation_id,
                mechanism_family: base.mechanism_family,
                enforcement_class: base.enforcement_class,
                lifecycle: base.lifecycle,
                capabilities: CapabilityOfferRefs::Borrowed(&scan),
                resources: ResourceOfferRefs::Borrowed(&resources),
            },
            BaseRegistryLimits {
                maximum_bases: 1,
                maximum_capabilities_per_base: 1,
                maximum_resources_per_base: 1,
                maximum_advertised_capabilities: 2,
                maximum_advertised_resources: 1,
            },
        )
        .map_err(|_| ScanCallRefusal::WrongBinding)?;
        let bases = [base];
        let offers = [scan[0], evidence];
        Ok(use_advertisement(
            self,
            HostAdvertisementRef {
                bases: BaseProviderAdvertisementRefs::Borrowed(&bases),
                resources: ResourceOfferRefs::Borrowed(&resources),
                capabilities: CapabilityOfferRefs::Borrowed(&offers),
                ..header
            },
        ))
    }
}
