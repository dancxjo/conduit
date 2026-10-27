//! An affine operation over one actual table/rotating bearer. Full immutable
//! claim facts are checked once; they remain in the exclusively owned table.
use super::*;

/// Prevalidated possession, not a new grant or a reset of finite operations.
/// It has no public constructor from IDs, Clone, serialization, or mutable
/// table/handle escape. The actual 256-bit bearer still selects its exact scope
/// on every authorization. A caller's operation/node binding remains separate.
pub struct BoundBaseOperation {
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    parameter_bytes: u32,
    work_units: u64,
}
impl BoundBaseOperation {
    /// Read-only full claim/bearer preflight; starts no operation and reserves
    /// no work. Useful before an independent physical owner is moved.
    pub fn check(
        table: &BaseCapabilityTable,
        handle: &BaseCapabilityHandle,
        claim: &BaseOperationClaim,
    ) -> Result<(), BaseCapabilityRefusal> {
        let entry = table
            .entries
            .iter()
            .find(|e| bearer_matches(&e.bearer, &handle.bearer))
            .ok_or(BaseCapabilityRefusal::UnknownCapability)?;
        match entry.inspection.lifecycle {
            CapabilityLifecycle::Revoked => return Err(BaseCapabilityRefusal::Revoked),
            CapabilityLifecycle::Exhausted => return Err(BaseCapabilityRefusal::Exhausted),
            CapabilityLifecycle::Issued => {}
        }
        validate_claim(&entry.inspection.scope, claim)?;
        if claim.parameter_bytes > entry.inspection.scope.maximum_parameter_bytes {
            return Err(BaseCapabilityRefusal::ParameterEnvelope);
        }
        if claim.work_units > entry.inspection.scope.maximum_work_units {
            return Err(BaseCapabilityRefusal::WorkEnvelope);
        }
        Ok(())
    }
    /// Consumes the actual table and bearer; every refusal returns all inputs
    /// unchanged. Dropping duplicated claim strings confers no new authority.
    #[allow(clippy::result_large_err)] // Affine refusal must return all owners without allocating.
    pub fn bind(
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
    ) -> Result<
        Self,
        (
            BaseCapabilityTable,
            BaseCapabilityHandle,
            BaseOperationClaim,
            BaseCapabilityRefusal,
        ),
    > {
        if let Err(error) = Self::check(&table, &handle, &claim) {
            return Err((table, handle, claim, error));
        }
        Ok(Self {
            table,
            handle,
            parameter_bytes: claim.parameter_bytes,
            work_units: claim.work_units,
        })
    }
    pub fn authorize(&mut self) -> Result<BaseOperationLease, BaseCapabilityRefusal> {
        let entry = self.table.entry_mut(&self.handle)?;
        match entry.inspection.lifecycle {
            CapabilityLifecycle::Revoked => return Err(BaseCapabilityRefusal::Revoked),
            CapabilityLifecycle::Exhausted => return Err(BaseCapabilityRefusal::Exhausted),
            CapabilityLifecycle::Issued => {}
        }
        // Scope fields are immutable and the full claim was checked by bind.
        // Bounds are retained exactly, including narrower-than-scope claims.
        if self.parameter_bytes > entry.inspection.scope.maximum_parameter_bytes {
            return Err(BaseCapabilityRefusal::ParameterEnvelope);
        }
        if self.work_units > entry.inspection.scope.maximum_work_units {
            return Err(BaseCapabilityRefusal::WorkEnvelope);
        }
        begin_entry(entry)
    }
    pub fn complete(
        &mut self,
        lease: BaseOperationLease,
        result_bytes: u32,
    ) -> Result<(), BaseCapabilityRefusal> {
        self.table.complete(&mut self.handle, lease, result_bytes)
    }
    pub fn revoke(&mut self) -> Result<(), BaseCapabilityRefusal> {
        self.table.revoke(&self.handle)
    }
    pub fn inspection(&self) -> &CapabilityInspection {
        &self
            .table
            .entries
            .iter()
            .find(|e| bearer_matches(&e.bearer, &self.handle.bearer))
            .expect("private bound bearer stays with its table")
            .inspection
    }
}
