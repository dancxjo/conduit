//! Exact native binding over the immutable fully Core-verified flat projection.
use super::*;
use crate::local_tables::FlatDirectLocalKernelTables;
pub struct SelectedFlatDirectOperationPlan<
    'a,
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
> {
    pub tables: &'a FlatDirectLocalKernelTables<'a, N, C, P, R, T, H>,
    pub active: &'a ActivePlayIdentity,
    pub placement_id: &'a str,
}
pub fn bind_selected_flat_direct_operation_for_subject<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
>(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContractRef<'_>,
    subject: &str,
    configuration: &[conduit_core::ConfigurationEntry],
    selection: SelectedFlatDirectOperationPlan<'_, N, C, P, R, T, H>,
) -> Result<NodeId, SelectedCallRefusal> {
    let view = selection.tables.preparation_scope().view();
    check_plan_scope_ref(
        claim,
        view.fragment_plan_id,
        view.header.host_id,
        view.header.boot_id,
        selection.active,
    )?;
    let mut matches = view
        .placements
        .iter()
        .filter(|gear| gear.placement_id == selection.placement_id);
    let gear = matches.next().ok_or(SelectedCallRefusal::WrongBinding)?;
    if matches.next().is_some() {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    let gear = binding::checked_gear_ref(table, claim, &definition, subject, gear, |gear| {
        gear.configuration == configuration
    })?;
    if definition.kind.inputs.len() > 1 || definition.kind.outputs.len() > 1 {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    selection
        .tables
        .node_for_placement(gear.placement_id)
        .ok_or(SelectedCallRefusal::WrongBinding)
}

/// Immutable, checked selected-operation data. This is not a capability and
/// cannot execute: binding still requires actual current Root-issued possession.
/// The owning expected scope permits temporary Source/Plan descriptions and
/// native Kind contracts to retire before execution storage is allocated.
pub struct PreparedFlatDirectOperation {
    expected: conduit_core::BaseCapabilityScope,
    node: NodeId,
}
impl PreparedFlatDirectOperation {
    pub fn prepare<
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    >(
        expected: conduit_core::BaseCapabilityScope,
        definition: SelectedOperationContractRef<'_>,
        subject: &str,
        configuration: &[conduit_core::ConfigurationEntry],
        tables: &FlatDirectLocalKernelTables<'_, N, C, P, R, T, H>,
        placement_id: &str,
    ) -> Result<Self, SelectedCallRefusal> {
        let view = tables.preparation_scope().view();
        if expected.plan_id.as_str() != view.fragment_plan_id
            || expected.host_id.as_str() != view.header.host_id
            || expected.boot_id.as_str() != view.header.boot_id
        {
            return Err(SelectedCallRefusal::WrongBinding);
        }
        let mut matches = view
            .placements
            .iter()
            .filter(|g| g.placement_id == placement_id);
        let gear = matches.next().ok_or(SelectedCallRefusal::WrongBinding)?;
        if matches.next().is_some() {
            return Err(SelectedCallRefusal::WrongBinding);
        }
        let gear =
            binding::checked_gear_scope_ref(&expected, &definition, subject, gear, |gear| {
                gear.configuration == configuration
            })?;
        if definition.kind.inputs.len() > 1 || definition.kind.outputs.len() > 1 {
            return Err(SelectedCallRefusal::WrongBinding);
        }
        let node = tables
            .node_for_placement(gear.placement_id)
            .ok_or(SelectedCallRefusal::WrongBinding)?;
        Ok(Self { expected, node })
    }
    /// Read-only descriptive policy input, never executable possession.
    pub fn expected_scope(&self) -> &conduit_core::BaseCapabilityScope {
        &self.expected
    }

    /// Move the exact checked scope directly into ordinary Core issuance. The
    /// numeric witness and actual issued possession cannot be separated or
    /// substituted while the caller performs its initialized-owner checks.
    pub fn issue(
        self,
        table: BaseCapabilityTable,
        authority: conduit_core::BaseCapabilityAuthorityRef<'_>,
        active: &ActivePlayIdentity,
        parameter_bytes: u32,
        work_units: u64,
    ) -> Result<IssuedFlatDirectOperation, SelectedCallRefusal> {
        let storage = self.prepare_issuance(parameter_bytes, work_units)?;
        self.issue_prepared(table, authority, active, storage)
    }

    pub fn prepare_issuance(
        &self,
        parameter_bytes: u32,
        work_units: u64,
    ) -> Result<PreparedFlatDirectIssuance, SelectedCallRefusal> {
        let scope = &self.expected;
        let claim = BaseOperationClaim {
            host_id: try_copy(scope.host_id.as_str())?.into(),
            boot_id: try_copy(scope.boot_id.as_str())?.into(),
            base_instance_id: try_copy(scope.base_instance_id.as_str())?.into(),
            base_provider_generation: scope.base_provider_generation,
            plan_id: try_copy(scope.plan_id.as_str())?.into(),
            active_play_id: try_copy(scope.active_play_id.as_str())?.into(),
            implementation_id: try_copy(scope.implementation_id.as_str())?.into(),
            operation_contract_id: try_copy(scope.operation_contract_id.as_str())?.into(),
            subject_kind: try_copy(scope.subject_kind.as_str())?.into(),
            resource_pool_id: try_copy(scope.resource_pool_id.as_str())?.into(),
            resource_generation_id: conduit_core::ResourceGenerationId(try_copy(
                &scope.resource_generation_id.0,
            )?),
            envelope_id: try_copy(scope.envelope_id.as_str())?.into(),
            parameter_bytes,
            work_units,
        };
        let backing = conduit_core::BaseCapabilityIssuanceBacking::try_new(scope.maximum_in_flight)
            .map_err(|_| SelectedCallRefusal::Possession)?;
        Ok(PreparedFlatDirectIssuance {
            claim,
            backing,
            node: self.node,
        })
    }

    pub fn issue_prepared(
        self,
        mut table: BaseCapabilityTable,
        authority: conduit_core::BaseCapabilityAuthorityRef<'_>,
        active: &ActivePlayIdentity,
        storage: PreparedFlatDirectIssuance,
    ) -> Result<IssuedFlatDirectOperation, SelectedCallRefusal> {
        let scope = self.expected;
        let PreparedFlatDirectIssuance {
            claim,
            backing,
            node,
        } = storage;
        if node != self.node
            || claim.host_id != scope.host_id
            || claim.boot_id != scope.boot_id
            || claim.base_instance_id != scope.base_instance_id
            || claim.base_provider_generation != scope.base_provider_generation
            || claim.plan_id != scope.plan_id
            || claim.active_play_id != scope.active_play_id
            || claim.implementation_id != scope.implementation_id
            || claim.operation_contract_id != scope.operation_contract_id
            || claim.subject_kind != scope.subject_kind
            || claim.resource_pool_id != scope.resource_pool_id
            || claim.resource_generation_id != scope.resource_generation_id
            || claim.envelope_id != scope.envelope_id
        {
            return Err(SelectedCallRefusal::WrongBinding);
        }
        check_plan_scope_ref(
            &claim,
            scope.plan_id.as_str(),
            scope.host_id.as_str(),
            scope.boot_id.as_str(),
            active,
        )?;
        let handle = table
            .issue_borrowed_with_backing(scope, authority, backing)
            .map_err(|_| SelectedCallRefusal::Possession)?;
        Ok(IssuedFlatDirectOperation {
            table,
            handle,
            claim,
            node: self.node,
        })
    }
    /// Refuse substituted Root, owner, generation, envelope, grant or Play
    /// scopes. A matching description alone cannot satisfy current possession.
    pub fn bind(
        &self,
        table: &BaseCapabilityTable,
        claim: &BaseOperationClaim,
        active: &ActivePlayIdentity,
    ) -> Result<NodeId, SelectedCallRefusal> {
        check_plan_scope_ref(
            claim,
            self.expected.plan_id.as_str(),
            self.expected.host_id.as_str(),
            self.expected.boot_id.as_str(),
            active,
        )?;
        if matching_scope(table, claim)? != &self.expected {
            return Err(SelectedCallRefusal::WrongBinding);
        }
        Ok(self.node)
    }
}

/// Actual possession inseparably coupled to the exact prepared numeric binding.
/// No public constructor, mutable scope access, Clone, or wire representation.
pub struct IssuedFlatDirectOperation {
    table: BaseCapabilityTable,
    handle: conduit_core::BaseCapabilityHandle,
    claim: BaseOperationClaim,
    node: NodeId,
}
impl IssuedFlatDirectOperation {
    pub fn possession(&self) -> (&BaseCapabilityTable, &BaseOperationClaim) {
        (&self.table, &self.claim)
    }
    /// Return every bearer for caller-owned terminal refusal custody.
    pub fn into_possession(
        self,
    ) -> (
        BaseCapabilityTable,
        conduit_core::BaseCapabilityHandle,
        BaseOperationClaim,
    ) {
        (self.table, self.handle, self.claim)
    }
    #[allow(
        clippy::result_large_err,
        reason = "Refusal must return the same affine actual possession without allocating."
    )]
    pub fn bind(
        self,
    ) -> Result<(conduit_core::BoundBaseOperation, NodeId), (Self, SelectedCallRefusal)> {
        let check = || {
            // Complete canonical Active correspondence was checked before
            // Core issued this immutable, inseparable scope/node possession.
            matching_scope(&self.table, &self.claim)?;
            conduit_core::BoundBaseOperation::check(&self.table, &self.handle, &self.claim)
                .map_err(|_| SelectedCallRefusal::Possession)
        };
        if let Err(error) = check() {
            return Err((self, error));
        }
        let operation = conduit_core::BoundBaseOperation::bind(self.table, self.handle, self.claim)
            .unwrap_or_else(|_| unreachable!("exact preflighted actual possession"));
        Ok((operation, self.node))
    }
}

/// Fallibly admitted claim and issuance memory; never capability possession.
pub struct PreparedFlatDirectIssuance {
    claim: BaseOperationClaim,
    backing: conduit_core::BaseCapabilityIssuanceBacking,
    node: NodeId,
}
fn try_copy(value: &str) -> Result<alloc::string::String, SelectedCallRefusal> {
    let mut result = alloc::string::String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| SelectedCallRefusal::Possession)?;
    result.push_str(value);
    Ok(result)
}

#[cfg(test)]
#[path = "flat_tests.rs"]
mod tests;
