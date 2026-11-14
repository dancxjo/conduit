//! Root's original native grant and one-attempt opaque capability issuance.
//! Facts are borrowed only from retained initialized Bases. Incoming assignments
//! cannot manufacture readiness, original authority, or provider possession.
extern crate alloc;
use alloc::string::String;
use conduit_body as b;
use conduit_core as c;
use sha2::{Digest, Sha256};
#[path = "execution_authority_continuation.rs"]
mod continuation;
#[path = "execution_authority_description.rs"]
mod description;
#[path = "execution_authority_grant.rs"]
mod grant_identity;
pub use description::ContinuationNativeDescription;

pub struct NativeFacts<'a> {
    host: &'a c::HostId,
    boot: &'a c::BootId,
    instance: &'a c::BaseInstanceId,
    generation: u64,
    pool: &'a c::ResourcePoolId,
    resource_generation: &'a c::ResourceGenerationId,
    envelope: &'a c::CapabilityEnvelopeId,
    artifact: &'a c::ArtifactId,
    implementation: &'a str,
    capability: &'a str,
}
macro_rules! facts {
    ($id:expr,$capability:expr) => {{
        let id = $id;
        NativeFacts {
            host: &id.host_id,
            boot: &id.boot_id,
            instance: &id.provider_instance_id,
            generation: id.provider_generation,
            pool: &id.resource_pool_id,
            resource_generation: &id.resource_generation_id,
            envelope: &id.envelope_id,
            artifact: &id.artifact_id,
            implementation: id.implementation_id.as_str(),
            capability: $capability,
        }
    }};
}
#[path = "execution_authority_facts.rs"]
mod facts;

/// Independently installed Source/equipment policy. These are requested ceilings,
/// not permission to enlarge the provider's envelope: its exact binder and Core
/// capability issuance must both accept them. Never populate from a received Plan.
pub struct Operation<'a> {
    pub kind: &'a str,
    pub authority: &'a str,
    pub call: &'a str,
    pub maximum_parameter_bytes: u32,
    pub maximum_result_bytes: u32,
    pub maximum_work_units: u64,
    pub maximum_operations: u32,
}
#[derive(Debug)]
pub enum Refusal {
    Scope,
    Bounds,
    Allocation,
    Capability(c::BaseCapabilityRefusal),
}
pub type Issued = (
    c::BaseCapabilityTable,
    c::BaseCapabilityHandle,
    c::BaseOperationClaim,
);
pub struct Issuer {
    next: u64,
    original: Option<(c::AuthorityGrant, [u8; 32])>,
}
impl Issuer {
    pub const fn new() -> Self {
        Self {
            next: 1,
            original: None,
        }
    }
    /// Actual retained original grant strings. Inline Issuer state is funded
    /// by its Root owner; table/claim backing is charged by prepared tickets.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let Some((grant, _)) = &self.original else {
            return Some(0);
        };
        [
            grant.grant_id.0.capacity(),
            grant.contract_id.0.capacity(),
            grant.host_call_contract_id.0.capacity(),
            grant.subject_kind.0.capacity(),
            grant.host_id.0.capacity(),
            grant.boot_id.0.capacity(),
            grant.capability_id.0.capacity(),
        ]
        .into_iter()
        .try_fold(0usize, |n, v| n.checked_add(v))
    }
    pub fn grant(&self) -> Option<&c::AuthorityGrant> {
        self.original.as_ref().map(|v| &v.0)
    }
    /// Root explicitly selects its installed Seed-owner preparation and execution
    /// policy before publishing this grant description. No Plan is required.
    pub fn authorize(
        &mut self,
        context: b::NativePreparationContext<'_>,
        policy: super::execution::Policy<'_>,
        native: &NativeFacts<'_>,
        operation: &Operation<'_>,
        grant_id: &str,
        maximum_bytes: usize,
    ) -> Result<(), Refusal> {
        selected_description(context, policy, native)?;
        let fingerprint = fingerprint(context, native, operation);
        self.authorize_checked(fingerprint, native, operation, grant_id, maximum_bytes)
    }
    fn authorize_checked(
        &mut self,
        fingerprint: [u8; 32],
        native: &NativeFacts<'_>,
        operation: &Operation<'_>,
        grant_id: &str,
        maximum_bytes: usize,
    ) -> Result<(), Refusal> {
        if grant_id.is_empty() || grant_id.len() > maximum_bytes {
            return Err(Refusal::Bounds);
        }
        let identity = grant_identity::bind(grant_id, &fingerprint);
        let grant_id = core::str::from_utf8(&identity).map_err(|_| Refusal::Scope)?;
        if let Some((original, old)) = &self.original {
            return if old == &fingerprint && original.grant_id.as_str() == grant_id {
                Ok(())
            } else {
                Err(Refusal::Scope)
            };
        }
        let mut budget = Budget(maximum_bytes);
        let grant = c::AuthorityGrant {
            grant_id: budget.copy(grant_id)?.into(),
            contract_id: budget.copy(operation.authority)?.into(),
            host_call_contract_id: budget.copy(operation.call)?.into(),
            subject_kind: budget.copy(operation.kind)?.into(),
            host_id: budget.copy(native.host.as_str())?.into(),
            boot_id: budget.copy(native.boot.as_str())?.into(),
            capability_id: budget.copy(native.capability)?.into(),
        };
        self.original = Some((grant, fingerprint));
        Ok(())
    }
    /// Only an opaque approval of a child of the original signed workset may
    /// scope a table. Issuance still does not admit resources, bind or Start.
    pub fn issue(
        &mut self,
        context: b::NativePreparationContext<'_>,
        policy: super::execution::Policy<'_>,
        approved: &b::DelegatedBodyFragment<'_>,
        native: &NativeFacts<'_>,
        op: &Operation<'_>,
        maximum_bytes: usize,
    ) -> Result<Issued, Refusal> {
        selected(context, policy, native)?;
        self.issue_checked(
            context.bootstrap.admission.secret(),
            context.actual,
            context.member.body_id.as_str(),
            fingerprint(context, native, op),
            approved,
            native,
            op,
            maximum_bytes,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn issue_checked(
        &mut self,
        secret: &[u8; 32],
        actual: &c::HostAdvertisement,
        body: &str,
        fingerprint: [u8; 32],
        approved: &b::DelegatedBodyFragment<'_>,
        native: &NativeFacts<'_>,
        op: &Operation<'_>,
        maximum_bytes: usize,
    ) -> Result<Issued, Refusal> {
        let (grant, old) = self.original.as_ref().ok_or(Refusal::Scope)?;
        if old != &fingerprint {
            return Err(Refusal::Scope);
        }
        let m = approved.metadata();
        if m.body != body
            || m.host != native.host.as_str()
            || m.boot != native.boot.as_str()
            || m.offer_generation != actual.offer_generation.0
            || approved.receiver_profile()
                != c::AssignedIdentity::from_text(actual.profile.as_str())
        {
            return Err(Refusal::Scope);
        }
        let attempt = self.next;
        self.next = attempt.checked_add(1).ok_or(Refusal::Bounds)?;
        let mut hash = Sha256::new();
        hash.update(b"conduit.mcu/native-capability-issuer@1\0");
        hash.update(secret);
        hash.update(attempt.to_le_bytes());
        hash.update(old);
        let key = hash.finalize().into();
        let core =
            c::BaseCapabilityTable::single_issuance_backing_bytes(1, 1).ok_or(Refusal::Bounds)?;
        let mut budget = Budget(maximum_bytes.checked_sub(core).ok_or(Refusal::Bounds)?);
        macro_rules! copy {
            ($s:expr) => {
                budget.copy($s)?.into()
            };
        }
        let scope = c::BaseCapabilityScope {
            host_id: copy!(native.host.as_str()),
            boot_id: copy!(native.boot.as_str()),
            base_instance_id: copy!(native.instance.as_str()),
            base_provider_generation: native.generation,
            plan_id: copy!(m.body_plan),
            active_play_id: copy!(m.body_play),
            authority_grant_id: copy!(grant.grant_id.as_str()),
            authority_contract_id: copy!(op.authority),
            capability_id: copy!(native.capability),
            implementation_id: copy!(native.implementation),
            operation_contract_id: copy!(op.call),
            subject_kind: copy!(op.kind),
            resource_pool_id: copy!(native.pool.as_str()),
            resource_generation_id: c::ResourceGenerationId(
                budget.copy(&native.resource_generation.0)?,
            ),
            envelope_id: copy!(native.envelope.as_str()),
            maximum_parameter_bytes: op.maximum_parameter_bytes,
            maximum_result_bytes: op.maximum_result_bytes,
            maximum_work_units: op.maximum_work_units,
            maximum_in_flight: 1,
            maximum_operations: op.maximum_operations,
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
            base_instance_id: copy!(native.instance.as_str()),
            base_provider_generation: native.generation,
            resource_pool_id: copy!(native.pool.as_str()),
            resource_generation_id: c::ResourceGenerationId(
                budget.copy(&native.resource_generation.0)?,
            ),
            operation_contract_id: copy!(op.call),
            envelope_id: copy!(native.envelope.as_str()),
            maximum_parameter_bytes: op.maximum_parameter_bytes,
            maximum_result_bytes: op.maximum_result_bytes,
            maximum_work_units: op.maximum_work_units,
            maximum_in_flight: 1,
            maximum_operations: op.maximum_operations,
        };
        let claim = c::BaseOperationClaim {
            host_id: copy!(native.host.as_str()),
            boot_id: copy!(native.boot.as_str()),
            base_instance_id: copy!(native.instance.as_str()),
            base_provider_generation: native.generation,
            plan_id: copy!(m.body_plan),
            active_play_id: copy!(m.body_play),
            implementation_id: copy!(native.implementation),
            operation_contract_id: copy!(op.call),
            subject_kind: copy!(op.kind),
            resource_pool_id: copy!(native.pool.as_str()),
            resource_generation_id: c::ResourceGenerationId(
                budget.copy(&native.resource_generation.0)?,
            ),
            envelope_id: copy!(native.envelope.as_str()),
            parameter_bytes: op.maximum_parameter_bytes,
            work_units: op.maximum_work_units,
        };
        let mut table = c::BaseCapabilityTable::new(
            copy!(native.host.as_str()),
            copy!(native.boot.as_str()),
            copy!(native.instance.as_str()),
            native.generation,
            key,
            1,
        )
        .map_err(Refusal::Capability)?;
        let handle = table
            .issue(c::CapabilityIssueRequest { scope, authority })
            .map_err(Refusal::Capability)?;
        Ok((table, handle, claim))
    }
}
impl Default for Issuer {
    fn default() -> Self {
        Self::new()
    }
}
struct Budget(usize);
impl Budget {
    fn copy(&mut self, text: &str) -> Result<String, Refusal> {
        if text.is_empty() || text.len() > self.0 {
            return Err(Refusal::Bounds);
        }
        let mut value = String::new();
        value
            .try_reserve_exact(text.len())
            .map_err(|_| Refusal::Allocation)?;
        self.0 = self
            .0
            .checked_sub(value.capacity())
            .ok_or(Refusal::Bounds)?;
        value.push_str(text);
        Ok(value)
    }
}
// Historical membership can support original public return descriptors, never
// opaque issuance or Start. It must be the exact retained Seed credential, not
// a caller-rewritten current-Boot credential or an arbitrary historical value.
fn selected_description(
    context: b::NativePreparationContext<'_>,
    policy: super::execution::Policy<'_>,
    native: &NativeFacts<'_>,
) -> Result<(), Refusal> {
    selected_installation(context, policy, native)?;
    if context.member.boot_id != context.actual.boot_id
        && context.bootstrap.admission.credential() != Some(context.member)
    {
        return Err(Refusal::Scope);
    }
    Ok(())
}
fn selected(
    context: b::NativePreparationContext<'_>,
    policy: super::execution::Policy<'_>,
    native: &NativeFacts<'_>,
) -> Result<(), Refusal> {
    selected_installation(context, policy, native)?;
    if context.member.boot_id != context.actual.boot_id {
        return Err(Refusal::Scope);
    }
    Ok(())
}
fn selected_installation(
    context: b::NativePreparationContext<'_>,
    policy: super::execution::Policy<'_>,
    native: &NativeFacts<'_>,
) -> Result<(), Refusal> {
    let (Some(preparation), Some(execution)) = (policy.preparation, policy.execution) else {
        return Err(Refusal::Scope);
    };
    if !matches!(
        preparation.authorizer,
        b::HostPreparationAuthorizer::SeedOwner
    ) || !matches!(execution.authorizer, b::HostExecutionAuthorizer::SeedOwner)
        || !preparation.has_finite_bounds()
        || execution.maximum_active_worksets == 0
        || context.bootstrap.admission.secret() == &[0; 32]
        || !context.bootstrap.matches_native(
            context.actual.host_id.as_str(),
            context.image_id,
            context.configuration_id,
        )
        || context.actual.profile.as_str() != context.configuration_id
        || native.host != &context.actual.host_id
        || native.boot != &context.actual.boot_id
        || native.artifact.as_str() != context.image_id
        || context.member.body_id != context.bootstrap.body_id
        || context.member.host_id != context.actual.host_id
    {
        return Err(Refusal::Scope);
    }
    Ok(())
}
fn fingerprint(
    context: b::NativePreparationContext<'_>,
    native: &NativeFacts<'_>,
    op: &Operation<'_>,
) -> [u8; 32] {
    fingerprint_parts(
        context.image_id,
        context.configuration_id,
        context.member.body_id.as_str(),
        native,
        op,
    )
}
fn fingerprint_parts(
    image: &str,
    configuration: &str,
    body: &str,
    native: &NativeFacts<'_>,
    op: &Operation<'_>,
) -> [u8; 32] {
    let mut h = Sha256::new();
    for text in [
        image,
        configuration,
        body,
        native.host.as_str(),
        native.boot.as_str(),
        native.instance.as_str(),
        native.pool.as_str(),
        &native.resource_generation.0,
        native.envelope.as_str(),
        native.implementation,
        native.capability,
        op.kind,
        op.authority,
        op.call,
    ] {
        h.update((text.len() as u64).to_le_bytes());
        h.update(text.as_bytes());
    }
    h.update(native.generation.to_le_bytes());
    for n in [
        op.maximum_parameter_bytes,
        op.maximum_result_bytes,
        op.maximum_operations,
    ] {
        h.update(n.to_le_bytes())
    }
    h.update(op.maximum_work_units.to_le_bytes());
    h.finalize().into()
}

#[path = "execution_authority_append.rs"]
mod append;

#[cfg(test)]
#[path = "execution_authority_tests.rs"]
mod tests;

#[path = "execution_authority_service.rs"]
mod service;

#[cfg(test)]
#[path = "execution_authority_io_tests.rs"]
mod io_tests;
