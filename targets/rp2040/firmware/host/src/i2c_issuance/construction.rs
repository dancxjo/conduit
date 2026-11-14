//! Fallible owned identities and Core's existing opaque capability table.
use super::*;
use alloc::string::String;
use conduit_body::BodyFragmentApprovalMetadata as Metadata;
use conduit_i2c_transaction::installation::I2cNativeIdentity;
use conduit_kernel::scheduler::KernelPreparationError;

pub struct Budget {
    remaining: usize,
}
impl Budget {
    pub fn prepare(
        id: &I2cNativeIdentity,
        m: &Metadata<'_>,
        suffix: &str,
        available: usize,
    ) -> Result<Self, E> {
        let grant_bytes = id
            .boot_id
            .as_str()
            .len()
            .checked_add(suffix.len())
            .ok_or(E::Bounds)?;
        let fields = [
            (id.host_id.as_str().len(), 4),
            (id.boot_id.as_str().len(), 4),
            (id.provider_instance_id.as_str().len(), 4),
            (m.body_plan.len(), 2),
            (m.body_play.len(), 2),
            (grant_bytes, 2),
            (I2C_AUTHORITY.len(), 2),
            (I2C_IMPLEMENTATION.len(), 4),
            (I2C_CALL.len(), 4),
            (I2C_KIND.len(), 3),
            (id.resource_pool_id.as_str().len(), 3),
            (id.resource_generation_id.0.len(), 3),
            (id.envelope_id.as_str().len(), 3),
        ];
        let core = c::BaseCapabilityTable::single_issuance_backing_bytes(1, 1).ok_or(E::Bounds)?;
        let required = fields
            .into_iter()
            .try_fold(core, |n, (len, count)| {
                n.checked_add(len.checked_mul(count)?)
            })
            .ok_or(E::Bounds)?;
        if required > available {
            return Err(E::Bounds);
        }
        Ok(Self {
            remaining: available - core,
        })
    }
    fn copy(&mut self, parts: &[&str]) -> Result<String, E> {
        let length = parts
            .iter()
            .try_fold(0usize, |n, s| n.checked_add(s.len()))
            .ok_or(E::Bounds)?;
        if length > self.remaining {
            return Err(E::Bounds);
        }
        let mut text = String::new();
        text.try_reserve_exact(length)
            .map_err(|_| E::Storage(KernelPreparationError::Allocation))?;
        self.remaining = self
            .remaining
            .checked_sub(text.capacity())
            .ok_or(E::Bounds)?;
        for part in parts {
            text.push_str(part);
        }
        Ok(text)
    }
}
fn capability(error: c::BaseCapabilityRefusal) -> E {
    E::Native(I2cCallRefusal::Capability(error))
}
pub fn issue(
    mut b: Budget,
    id: &I2cNativeIdentity,
    m: &Metadata<'_>,
    grant: &c::AuthorityGrant,
    key: [u8; 32],
) -> Result<Issued, E> {
    macro_rules! copy {
        ($s:expr) => {
            b.copy(&[$s])?.into()
        };
    }
    let scope = c::BaseCapabilityScope {
        host_id: copy!(id.host_id.as_str()),
        boot_id: copy!(id.boot_id.as_str()),
        base_instance_id: copy!(id.provider_instance_id.as_str()),
        base_provider_generation: id.provider_generation,
        plan_id: copy!(m.body_plan),
        active_play_id: copy!(m.body_play),
        authority_grant_id: copy!(grant.grant_id.as_str()),
        authority_contract_id: copy!(I2C_AUTHORITY),
        capability_id: copy!(I2C_IMPLEMENTATION),
        implementation_id: copy!(I2C_IMPLEMENTATION),
        operation_contract_id: copy!(I2C_CALL),
        subject_kind: copy!(I2C_KIND),
        resource_pool_id: copy!(id.resource_pool_id.as_str()),
        resource_generation_id: c::ResourceGenerationId(b.copy(&[&id.resource_generation_id.0])?),
        envelope_id: copy!(id.envelope_id.as_str()),
        maximum_parameter_bytes: I2C_MAXIMUM_BYTES,
        maximum_result_bytes: I2C_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: config::I2C_CALL_MAXIMUM_OPERATIONS,
    };
    let authority = c::BaseCapabilityAuthority {
        grant: c::AuthorityGrant {
            grant_id: copy!(grant.grant_id.as_str()),
            contract_id: copy!(grant.contract_id.as_str()),
            host_call_contract_id: copy!(grant.host_call_contract_id.as_str()),
            subject_kind: copy!(grant.subject_kind.as_str()),
            host_id: copy!(grant.host_id.as_str()),
            boot_id: copy!(grant.boot_id.as_str()),
            capability_id: copy!(grant.capability_id.as_str()),
        },
        base_instance_id: copy!(id.provider_instance_id.as_str()),
        base_provider_generation: id.provider_generation,
        resource_pool_id: copy!(id.resource_pool_id.as_str()),
        resource_generation_id: c::ResourceGenerationId(b.copy(&[&id.resource_generation_id.0])?),
        operation_contract_id: copy!(I2C_CALL),
        envelope_id: copy!(id.envelope_id.as_str()),
        maximum_parameter_bytes: I2C_MAXIMUM_BYTES,
        maximum_result_bytes: I2C_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: config::I2C_CALL_MAXIMUM_OPERATIONS,
    };
    let claim = c::BaseOperationClaim {
        host_id: copy!(id.host_id.as_str()),
        boot_id: copy!(id.boot_id.as_str()),
        base_instance_id: copy!(id.provider_instance_id.as_str()),
        base_provider_generation: id.provider_generation,
        plan_id: copy!(m.body_plan),
        active_play_id: copy!(m.body_play),
        implementation_id: copy!(I2C_IMPLEMENTATION),
        operation_contract_id: copy!(I2C_CALL),
        subject_kind: copy!(I2C_KIND),
        resource_pool_id: copy!(id.resource_pool_id.as_str()),
        resource_generation_id: c::ResourceGenerationId(b.copy(&[&id.resource_generation_id.0])?),
        envelope_id: copy!(id.envelope_id.as_str()),
        parameter_bytes: I2C_MAXIMUM_BYTES,
        work_units: 1,
    };
    let mut table = c::BaseCapabilityTable::new(
        copy!(id.host_id.as_str()),
        copy!(id.boot_id.as_str()),
        copy!(id.provider_instance_id.as_str()),
        id.provider_generation,
        key,
        1,
    )
    .map_err(capability)?;
    let handle = table
        .issue(c::CapabilityIssueRequest { scope, authority })
        .map_err(capability)?;
    Ok((table, handle, claim))
}
