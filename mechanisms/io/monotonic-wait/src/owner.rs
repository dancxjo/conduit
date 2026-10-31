use conduit_core as c;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::selected_operation::{
    bind_selected_configured_operation, SelectedOperationContract, SelectedOperationPlan,
};

pub const PARAMETER_BYTES: u32 = 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitProviderFailure {
    Pressure,
    ProviderUnavailable,
}
/// The native Root must retain an initialized monotonic timer slot. Waiting
/// performs no allocation, terminates within its admitted duration/work bound,
/// and drops/quiesces its pending operation when the future is cancelled.
pub trait MonotonicWaitProvider {
    fn wait(
        &mut self,
        duration_ms: u64,
    ) -> impl core::future::Future<Output = Result<(), WaitProviderFailure>>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitAttachment {
    pub generation: u64,
    pub resource_bytes: u64,
    pub maximum_duration_ms: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub enum WaitCallRefusal {
    Possession,
    WrongBinding,
    StaleRequest,
    SequenceExhausted,
    InvalidDuration,
    Capability(c::BaseCapabilityRefusal),
    Provider(WaitProviderFailure),
}
/// Retirement refuses without consuming the retained native owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitRetirementRefusal {
    Unavailable,
    InFlight,
    NotRevoked,
}

pub struct MonotonicWaitHostCall<P> {
    operation: c::BoundBaseOperation,
    native_identity: Option<crate::installation::NativeReturnSeal>,
    attachment: WaitAttachment,
    provider: P,
    node: NodeId,
    next_request: u32,
}
impl<P: MonotonicWaitProvider> MonotonicWaitHostCall<P> {
    /// Bind actual native possession to the exact planned Tick and Play.
    /// # Safety
    /// Root must own this initialized timer slot and generation, independently
    /// authorize the exact Plan/Play, and admit provider memory, duration and work.
    /// Configuration must come from Root's independently checked Source, not
    /// the received Plan. The cancellation law belongs to native installation.
    pub unsafe fn bind_selected(
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        attachment: WaitAttachment,
        provider: P,
        configuration: &[c::ConfigurationEntry],
        selection: SelectedOperationPlan<'_>,
    ) -> Result<Self, WaitCallRefusal> {
        let node = checked_binding(&table, &claim, attachment, configuration, selection)?;
        c::BoundBaseOperation::check(&table, &handle, &claim)
            .map_err(WaitCallRefusal::Capability)?;
        Ok(Self::from_bound(
            table, handle, claim, attachment, provider, node,
        ))
    }
    pub(crate) fn from_bound(
        table: c::BaseCapabilityTable,
        handle: c::BaseCapabilityHandle,
        claim: c::BaseOperationClaim,
        attachment: WaitAttachment,
        provider: P,
        node: NodeId,
    ) -> Self {
        Self {
            operation: bound_operation(table, handle, claim),
            native_identity: None,
            attachment,
            provider,
            node,
            next_request: 0,
        }
    }
    /// Invalid input, a foreign node/call or stale request cannot start a wait.
    /// Zero duration is valid. No result value is emitted by this Host Call.
    pub async fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<(), WaitCallRefusal> {
        use WaitCallRefusal as E;
        self.check_binding(node, call)?;
        if request != RequestId(self.next_request) {
            return Err(E::StaleRequest);
        }
        let next = self
            .next_request
            .checked_add(1)
            .ok_or(E::SequenceExhausted)?;
        let duration = c::decode_monotonic_duration(input).map_err(|_| E::InvalidDuration)?;
        if duration > self.attachment.maximum_duration_ms {
            return Err(E::InvalidDuration);
        }
        let lease = self.operation.authorize().map_err(E::Capability)?;
        self.next_request = next;
        // Created before the provider future: cancellation drops provider work
        // first, then revokes the unfinished capability. No implicit retry.
        let pending = Pending {
            operation: &mut self.operation,
            lease: Some(lease),
        };
        let result = self.provider.wait(duration).await.map_err(E::Provider);
        pending.complete().map_err(E::Capability)?;
        result
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), WaitCallRefusal> {
        self.check_binding(node, call)?;
        self.operation.revoke().map_err(WaitCallRefusal::Capability)
    }
    /// Consume only revoked, idle possession and return the exact provider and
    /// attachment. Refusal leaves `owner` intact. No capability, claim or Play
    /// authority survives this transfer; a later binding needs fresh admission.
    /// A pending invocation borrows the owner, so its provider future must be
    /// dropped/quiesced before this exclusive retirement entrance is available.
    pub fn retire_retained(
        owner: &mut Option<Self>,
    ) -> Result<(P, WaitAttachment), WaitRetirementRefusal> {
        Self::retire_base_retained(owner).map(|retired| retired.into_parts())
    }
    /// Preserve opaque Ready-origin continuation while retiring actual revoked,
    /// idle custody. Raw unsafe bindings produce an ineligible continuation.
    pub fn retire_base_retained(
        owner: &mut Option<Self>,
    ) -> Result<crate::installation::RetiredWaitBase<P>, WaitRetirementRefusal> {
        let current = owner.as_ref().ok_or(WaitRetirementRefusal::Unavailable)?;
        let inspection = current.inspection();
        if inspection.in_flight_operations != 0 {
            return Err(WaitRetirementRefusal::InFlight);
        }
        if inspection.lifecycle != c::CapabilityLifecycle::Revoked {
            return Err(WaitRetirementRefusal::NotRevoked);
        }
        let generation = current.inspection().scope.base_provider_generation;
        let current = owner.take().expect("preflighted retained owner");
        Ok(crate::installation::RetiredWaitBase::new(
            current.provider,
            current.attachment,
            current.native_identity,
            generation,
        ))
    }
    pub(crate) fn with_native_return(
        mut self,
        seal: crate::installation::NativeReturnSeal,
    ) -> Self {
        self.native_identity = Some(seal);
        self
    }

    pub fn inspection(&self) -> &c::CapabilityInspection {
        self.operation.inspection()
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), WaitCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(WaitCallRefusal::WrongBinding);
        }
        Ok(())
    }
}
struct Pending<'a> {
    operation: &'a mut c::BoundBaseOperation,
    lease: Option<c::BaseOperationLease>,
}
impl Pending<'_> {
    fn complete(mut self) -> Result<(), c::BaseCapabilityRefusal> {
        self.operation
            .complete(self.lease.as_ref().expect("pending").clone(), 0)?;
        self.lease = None;
        Ok(())
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.lease.is_some() {
            let _ = self.operation.revoke();
        }
    }
}

pub(crate) fn checked_binding(
    table: &c::BaseCapabilityTable,
    claim: &c::BaseOperationClaim,
    attachment: WaitAttachment,
    configuration: &[c::ConfigurationEntry],
    selection: SelectedOperationPlan<'_>,
) -> Result<NodeId, WaitCallRefusal> {
    use WaitCallRefusal as E;
    let kind = checked_contract(table, claim, attachment, configuration)?;
    let node = bind_selected_configured_operation(
        table,
        claim,
        SelectedOperationContract {
            kind: &kind,
            call: c::WAIT_HOST_CALL_CONTRACT,
            input_bytes: PARAMETER_BYTES,
            output_bytes: 0,
            resource_bytes: attachment.resource_bytes,
        },
        configuration,
        selection,
    )
    .map_err(|error| match error {
        conduit_plan_lowering::selected_operation::SelectedCallRefusal::WrongBinding => {
            E::WrongBinding
        }
        conduit_plan_lowering::selected_operation::SelectedCallRefusal::Possession => E::Possession,
    })?;
    Ok(node)
}

/// Common finite Source and possession checks before either projection binds.
pub(crate) fn checked_contract(
    table: &c::BaseCapabilityTable,
    claim: &c::BaseOperationClaim,
    attachment: WaitAttachment,
    configuration: &[c::ConfigurationEntry],
) -> Result<c::Kind, WaitCallRefusal> {
    use WaitCallRefusal as E;
    let mut entries = table.inspections();
    let scope = &entries.next().ok_or(E::Possession)?.scope;
    if entries.next().is_some()
        || scope.maximum_in_flight != 1
        || scope.maximum_parameter_bytes < PARAMETER_BYTES
        || claim.parameter_bytes != PARAMETER_BYTES
        || claim.work_units != 1
        || attachment.generation == 0
        || attachment.resource_bytes == 0
    {
        return Err(E::Possession);
    }
    let kind = conduit_time::tick_semantic_contract();
    for (index, entry) in configuration.iter().enumerate() {
        if configuration[..index]
            .iter()
            .any(|prior| prior.key == entry.key)
            || !kind
                .configuration
                .iter()
                .any(|field| field.key == entry.key)
        {
            return Err(E::WrongBinding);
        }
    }
    let configured = |name: &str| -> Result<u64, E> {
        let field = kind
            .configuration
            .iter()
            .find(|field| field.key == name)
            .ok_or(E::WrongBinding)?;
        let value = configuration
            .iter()
            .find(|entry| entry.key == name)
            .map_or(&field.default_value, |entry| &entry.value);
        let c::ConfigurationValue::U64(value) = value else {
            return Err(E::WrongBinding);
        };
        let c::KindConfigurationRule::U64Range { minimum, maximum } = field.rule else {
            return Err(E::WrongBinding);
        };
        if *value < minimum || *value > maximum {
            return Err(E::WrongBinding);
        }
        Ok(*value)
    };
    if configured("period-ms")? > attachment.maximum_duration_ms {
        return Err(E::InvalidDuration);
    }
    if configured("count")? > u64::from(scope.maximum_operations) {
        return Err(E::Possession);
    }
    drop(entries);
    Ok(kind)
}

fn bound_operation(
    table: c::BaseCapabilityTable,
    handle: c::BaseCapabilityHandle,
    claim: c::BaseOperationClaim,
) -> c::BoundBaseOperation {
    match c::BoundBaseOperation::bind(table, handle, claim) {
        Ok(operation) => operation,
        Err(_) => unreachable!("exclusive full-claim preflight before provider transfer"),
    }
}
