//! Immutable exact preparation may outlive temporary Source/offer tables;
//! actual issuance still requires the same generative initialized owner.
use super::*;
use conduit_plan_lowering::selected_operation::{
    IssuedFlatDirectOperation, PreparedFlatClaimBacking, PreparedFlatDirectOperation,
    SelectedOperationContractRef,
};

pub struct PreparedScanSelection<'owner, P> {
    owner: ReadyScanView<'owner, P>,
    operation: PreparedFlatDirectOperation,
}
impl<'owner, P: NetworkScanProvider> ReadyScanView<'owner, P> {
    pub fn prepare_flat_selection(
        self,
        expected: BaseCapabilityScope,
        tables: &ScanFlatDirectTables<'_>,
        placement: &str,
    ) -> Result<PreparedScanSelection<'owner, P>, ScanCallRefusal> {
        let i = self.native_identity();
        if expected.host_id != i.host
            || expected.boot_id != i.boot
            || expected.base_instance_id != i.instance
            || expected.base_provider_generation != i.provider_generation
            || expected.resource_pool_id != i.resource
            || expected.resource_generation_id != i.resource_generation
            || expected.envelope_id != i.envelope
            || expected.implementation_id != i.implementation
            || expected.maximum_operations != 1
            || expected.maximum_in_flight != 1
            || expected.maximum_parameter_bytes != MAXIMUM_SCAN_INPUT_BYTES
            || expected.maximum_result_bytes != MAXIMUM_SCAN_OUTPUT_BYTES
            || expected.authority_contract_id.as_str() != NETWORK_SCAN_AUTHORITY
        {
            return Err(ScanCallRefusal::WrongBinding);
        }
        let canonical = PreparedCanonicalNetworkOffer::scan(
            SCAN_CAPABILITY,
            i.implementation.as_str(),
            i.artifact.as_str(),
        )
        .into_contract();
        let view = tables.preparation_scope().view();
        let gear = view
            .placements
            .iter()
            .find(|g| g.placement_id == placement)
            .ok_or(ScanCallRefusal::WrongBinding)?;
        self.ready.check_native_ref(gear, canonical.kind().limits)?;
        let operation = PreparedFlatDirectOperation::prepare(
            expected,
            SelectedOperationContractRef {
                kind: canonical.kind(),
                semantic_contract: canonical.semantic_contract(),
                call: NETWORK_SCAN_HOST_CALL,
                input_bytes: MAXIMUM_SCAN_INPUT_BYTES,
                output_bytes: MAXIMUM_SCAN_OUTPUT_BYTES,
                resource_bytes: 1,
            },
            NETWORK_SCAN_SUBJECT,
            &[],
            tables,
            placement,
        )
        .map_err(ScanCallRefusal::Binding)?;
        Ok(PreparedScanSelection {
            owner: self,
            operation,
        })
    }
}
impl<'owner, P: NetworkScanProvider> PreparedScanSelection<'owner, P> {
    pub fn prepare_claim_storage(&self) -> Result<PreparedFlatClaimBacking, ScanCallRefusal> {
        PreparedFlatClaimBacking::try_new(&[&self.operation]).map_err(ScanCallRefusal::Binding)
    }
    /// Prepare actual Root backing before admission, without issuing possession.
    pub fn prepare_issuance_storage<R>(
        &self,
        prepare: impl FnOnce(&NetworkNativeIdentity, &PreparedFlatDirectOperation) -> R,
    ) -> R {
        prepare(self.owner.native_identity(), &self.operation)
    }
    /// On any post-issuance refusal return the complete original bearer. The
    /// enclosing Ready::prepare also returns the same initialized owner.
    #[allow(clippy::result_large_err)]
    pub fn issue_reusing(
        self,
        memory: PreparedFlatClaimBacking,
        issue: impl FnOnce(
            &NetworkNativeIdentity,
            PreparedFlatDirectOperation,
            PreparedFlatClaimBacking,
        ) -> Result<IssuedFlatDirectOperation, ()>,
    ) -> Result<PreparedScanBinding<'owner>, (Option<IssuedFlatDirectOperation>, ScanCallRefusal)>
    {
        let issued = issue(self.owner.native_identity(), self.operation, memory)
            .map_err(|_| (None, ScanCallRefusal::WrongBinding))?;
        let (table, claim) = issued.possession();
        let checked = self
            .owner
            .ready
            .check_claim(claim)
            .and_then(|()| ReadyNetworkScanBase::<P>::check_possession(table, claim));
        if let Err(error) = checked {
            return Err((Some(issued), error));
        }
        match issued.bind() {
            Ok((operation, node)) => Ok(PreparedScanBinding {
                operation,
                node,
                brand: self.owner.brand,
            }),
            Err((issued, error)) => Err((Some(issued), ScanCallRefusal::Binding(error))),
        }
    }
}
