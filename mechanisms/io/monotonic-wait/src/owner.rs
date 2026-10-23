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
pub struct MonotonicWaitHostCall<P> {
    table: c::BaseCapabilityTable,
    handle: c::BaseCapabilityHandle,
    claim: c::BaseOperationClaim,
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
            table,
            handle,
            claim,
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
        let lease = self
            .table
            .authorize(&self.handle, &self.claim)
            .map_err(E::Capability)?;
        self.next_request = next;
        // Created before the provider future: cancellation drops provider work
        // first, then revokes the unfinished capability. No implicit retry.
        let pending = Pending {
            table: &mut self.table,
            handle: &mut self.handle,
            lease: Some(lease),
        };
        let result = self.provider.wait(duration).await.map_err(E::Provider);
        pending.complete().map_err(E::Capability)?;
        result
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), WaitCallRefusal> {
        self.check_binding(node, call)?;
        self.table
            .revoke(&self.handle)
            .map_err(WaitCallRefusal::Capability)
    }
    pub fn inspection(&self) -> &c::CapabilityInspection {
        self.table
            .inspections()
            .next()
            .expect("single prepared capability")
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<(), WaitCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(WaitCallRefusal::WrongBinding);
        }
        Ok(())
    }
}
struct Pending<'a> {
    table: &'a mut c::BaseCapabilityTable,
    handle: &'a mut c::BaseCapabilityHandle,
    lease: Option<c::BaseOperationLease>,
}
impl Pending<'_> {
    fn complete(mut self) -> Result<(), c::BaseCapabilityRefusal> {
        self.table.complete(
            self.handle,
            self.lease.as_ref().expect("pending").clone(),
            0,
        )?;
        self.lease = None;
        Ok(())
    }
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if self.lease.is_some() {
            let _ = self.table.revoke(self.handle);
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
