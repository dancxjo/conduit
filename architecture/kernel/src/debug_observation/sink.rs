use crate::{
    KernelEvent, KernelEventKind, NodeId, PortId, RemoteLifecycleIdentity, RequestId, SignError,
    SignSink,
};

use super::{
    DebugEventKind, DebugExecutionIdentity, DebugNodeBinding, DebugObservationBuffer,
    DebugObservationInput, DebugObservationRefusal, DebugRuntimeEvent, DebugSubject,
};

/// Narrow lifecycle control for an optional debugger projection.
///
/// Implementations must not expose or mutate the mandatory Sign path.
pub trait DebugObserverControl {
    type History;

    fn attach_debug_observer(
        &mut self,
        history: Self::History,
    ) -> Result<(), DebugObservationRefusal>;

    fn detach_debug_observer(&mut self) -> Result<Self::History, DebugObservationRefusal>;
}

pub struct ObservedSignSink<E, const NODES: usize, const PORTS: usize, const RECORDS: usize>
where
    E: SignSink,
{
    inner: E,
    execution: DebugExecutionIdentity,
    node_bindings: [DebugNodeBinding; NODES],
    port_type_identities: [[Option<u16>; PORTS]; NODES],
    observations: Option<DebugObservationBuffer<RECORDS>>,
    started: [bool; NODES],
    next_host_sequences: [Option<(u16, u64)>; NODES],
}

impl<E, const NODES: usize, const PORTS: usize, const RECORDS: usize>
    ObservedSignSink<E, NODES, PORTS, RECORDS>
where
    E: SignSink,
{
    pub const fn detached(
        inner: E,
        execution: DebugExecutionIdentity,
        node_bindings: [DebugNodeBinding; NODES],
        port_type_identities: [[Option<u16>; PORTS]; NODES],
    ) -> Self {
        Self {
            inner,
            execution,
            node_bindings,
            port_type_identities,
            observations: None,
            started: [false; NODES],
            next_host_sequences: [None; NODES],
        }
    }

    pub fn attach(
        &mut self,
        observations: DebugObservationBuffer<RECORDS>,
    ) -> Result<(), DebugObservationRefusal> {
        if self.observations.is_some() {
            return Err(DebugObservationRefusal::ObserverAlreadyAttached);
        }
        if observations.execution() != self.execution {
            return Err(DebugObservationRefusal::StaleExecution);
        }
        self.observations = Some(observations);
        Ok(())
    }

    pub fn detach(&mut self) -> Result<DebugObservationBuffer<RECORDS>, DebugObservationRefusal> {
        self.observations
            .take()
            .ok_or(DebugObservationRefusal::ObserverDetached)
    }

    pub fn observations(&self) -> Option<&DebugObservationBuffer<RECORDS>> {
        self.observations.as_ref()
    }

    pub const fn inner(&self) -> &E {
        &self.inner
    }

    fn project(&mut self, event: DebugRuntimeEvent<'_>) {
        let node = usize::from(event.node.0);
        let Some(binding) = self.node_bindings.get(node).copied() else {
            return;
        };
        if event.kind == DebugEventKind::GearStarted {
            if self.started[node] {
                return;
            }
            self.started[node] = true;
        }
        let host_sequence = if let Some((_, next)) = self
            .next_host_sequences
            .iter_mut()
            .flatten()
            .find(|(host, _)| *host == binding.host)
        {
            let sequence = *next;
            *next = next.saturating_add(1);
            sequence
        } else {
            let Some(slot) = self
                .next_host_sequences
                .iter_mut()
                .find(|slot| slot.is_none())
            else {
                return;
            };
            *slot = Some((binding.host, 1));
            0
        };
        let subject = event.cord.map_or_else(
            || {
                event
                    .port
                    .map_or(DebugSubject::Gear(event.node), |port| DebugSubject::Port {
                        gear: event.node,
                        port,
                    })
            },
            DebugSubject::Cord,
        );
        let related_subject = event.port.map(|port| DebugSubject::Port {
            gear: event.node,
            port,
        });
        let type_identity = event.type_identity.or_else(|| {
            event.port.and_then(|port| {
                self.port_type_identities
                    .get(node)
                    .and_then(|types| types.get(usize::from(port.0)))
                    .copied()
                    .flatten()
            })
        });
        if let Some(observations) = &mut self.observations {
            let _ = observations.admit(DebugObservationInput {
                execution: self.execution,
                host_sequence,
                host: binding.host,
                form: binding.form,
                subject,
                related_subject,
                kind: event.kind,
                type_identity,
                value: event.value,
                fault_code: event.fault_code,
            });
        }
    }
}

impl<E, const NODES: usize, const PORTS: usize, const RECORDS: usize> SignSink
    for ObservedSignSink<E, NODES, PORTS, RECORDS>
where
    E: SignSink,
{
    fn item_capacity(&self) -> u16 {
        self.inner.item_capacity()
    }

    fn byte_capacity(&self) -> u32 {
        self.inner.byte_capacity()
    }

    fn len(&self) -> u16 {
        self.inner.len()
    }

    fn used_bytes(&self) -> u32 {
        self.inner.used_bytes()
    }

    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<KernelEvent, SignError> {
        self.inner.record(node, port, request, kind)
    }

    fn record_remote(
        &mut self,
        node: NodeId,
        port: PortId,
        kind: KernelEventKind,
        remote: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError> {
        self.inner.record_remote(node, port, kind, remote)
    }

    fn ensure_remote_capacity(&self, additional: u16) -> Result<(), SignError> {
        self.inner.ensure_remote_capacity(additional)
    }

    fn observe_debug(&mut self, event: DebugRuntimeEvent<'_>) {
        self.project(event)
    }
}

impl<E, const NODES: usize, const PORTS: usize, const RECORDS: usize> DebugObserverControl
    for ObservedSignSink<E, NODES, PORTS, RECORDS>
where
    E: SignSink,
{
    type History = DebugObservationBuffer<RECORDS>;

    fn attach_debug_observer(
        &mut self,
        history: Self::History,
    ) -> Result<(), DebugObservationRefusal> {
        self.attach(history)
    }

    fn detach_debug_observer(&mut self) -> Result<Self::History, DebugObservationRefusal> {
        self.detach()
    }
}
