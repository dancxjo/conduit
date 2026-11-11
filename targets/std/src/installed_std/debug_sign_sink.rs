//! Optional bounded observation beside the unchanged mandatory std Sign store.
use super::{MAX_CORDS, MAX_NODES, PORTS};
use conduit_kernel::{
    debug_observation::{
        DebugExecutionIdentity, DebugNodeBinding, DebugObservationBuffer, DebugObservationRecord,
        DebugObservationRefusal, DebugObserverControl, DebugRuntimeEvent, ObservedSignSink,
        MAX_DEBUG_VALUE_PREVIEW_BYTES,
    },
    HostedSignLog, KernelEvent, KernelEventKind, NodeId, PortId, RemoteLifecycleIdentity,
    RequestId, SignError, SignQuery, SignRetentionGap, SignSink,
};

pub(super) const DEBUG_RECORDS: usize = 128;
type Observer = ObservedSignSink<HostedSignLog, MAX_NODES, PORTS, DEBUG_RECORDS>;
pub(super) struct InstalledDebugScope {
    pub execution: DebugExecutionIdentity,
    pub node_bindings: [DebugNodeBinding; MAX_NODES],
    /// Exact planned Cord value kinds, interned by the preparation owner.
    pub cord_types: [Option<u16>; MAX_CORDS],
}
pub(super) enum InstalledSignSink {
    Plain(HostedSignLog),
    // Both allocations are exclusive to the explicitly selected observer.
    Observed(Box<Observer>, Box<[Option<u16>; MAX_CORDS]>),
}
impl InstalledSignSink {
    pub(super) fn observed(
        sign: HostedSignLog,
        scope: InstalledDebugScope,
    ) -> Result<(Self, DebugObservationBuffer<DEBUG_RECORDS>), String> {
        let bytes = u32::try_from(DEBUG_RECORDS * core::mem::size_of::<DebugObservationRecord>())
            .map_err(|_| "installed debugger byte budget overflow")?;
        let history = DebugObservationBuffer::new(
            scope.execution,
            DEBUG_RECORDS as u16,
            bytes,
            MAX_DEBUG_VALUE_PREVIEW_BYTES as u8,
        )
        .map_err(|error| format!("prepare std debugger history: {error:?}"))?;
        // Port numbers are directional in the Plan but the observer's legacy
        // fallback table is not. Exact Cord typing below avoids conflating a
        // model's request port zero with its result port zero.
        let observed = Box::new(Observer::detached(
            sign,
            scope.execution,
            scope.node_bindings,
            [[None; PORTS]; MAX_NODES],
        ));
        Ok((
            Self::Observed(observed, Box::new(scope.cord_types)),
            history,
        ))
    }
    pub(super) fn inner(&self) -> &HostedSignLog {
        match self {
            Self::Plain(sign) => sign,
            Self::Observed(sign, _) => sign.inner(),
        }
    }
    pub(super) fn events(&self) -> impl Iterator<Item = KernelEvent> + '_ {
        self.inner().events()
    }
    /// Mandatory Sign storage only; debugger storage is separately fixed/admitted.
    #[cfg(test)]
    pub(super) fn allocation_capacity(&self) -> usize {
        self.inner().allocation_capacity()
    }
    pub(super) fn take_debug_history(&mut self) -> Option<DebugObservationBuffer<DEBUG_RECORDS>> {
        match self {
            Self::Plain(_) => None,
            Self::Observed(sign, _) => sign.detach().ok(),
        }
    }
}
impl SignSink for InstalledSignSink {
    fn item_capacity(&self) -> u16 {
        self.inner().item_capacity()
    }
    fn byte_capacity(&self) -> u32 {
        self.inner().byte_capacity()
    }
    fn len(&self) -> u16 {
        self.inner().len()
    }
    fn used_bytes(&self) -> u32 {
        self.inner().used_bytes()
    }
    fn ensure_capacity(&self, additional: u16) -> Result<(), SignError> {
        self.inner().ensure_capacity(additional)
    }
    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<KernelEvent, SignError> {
        match self {
            Self::Plain(sign) => sign.record(node, port, request, kind),
            Self::Observed(sign, _) => sign.record(node, port, request, kind),
        }
    }
    fn record_remote(
        &mut self,
        node: NodeId,
        port: PortId,
        kind: KernelEventKind,
        remote: RemoteLifecycleIdentity,
    ) -> Result<KernelEvent, SignError> {
        match self {
            Self::Plain(sign) => sign.record_remote(node, port, kind, remote),
            Self::Observed(sign, _) => sign.record_remote(node, port, kind, remote),
        }
    }
    fn ensure_remote_capacity(&self, additional: u16) -> Result<(), SignError> {
        self.inner().ensure_remote_capacity(additional)
    }
    fn requires_canonical_debug_values(&self) -> bool {
        match self {
            Self::Plain(sign) => sign.requires_canonical_debug_values(),
            Self::Observed(sign, _) => sign.requires_canonical_debug_values(),
        }
    }
    fn observe_debug(&mut self, mut event: DebugRuntimeEvent<'_>) {
        match self {
            Self::Plain(sign) => sign.observe_debug(event),
            Self::Observed(sign, types) => {
                if let Some(cord) = event.cord {
                    event.type_identity = types.get(usize::from(cord.0)).copied().flatten();
                }
                sign.observe_debug(event);
            }
        }
    }
}
impl SignQuery for InstalledSignSink {
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.inner().contains_kind(kind)
    }
    fn remote_identity(&self, event_sequence: u32) -> Option<RemoteLifecycleIdentity> {
        self.inner().remote_identity(event_sequence)
    }
    fn retention_gap(&self) -> Option<SignRetentionGap> {
        self.inner().retention_gap()
    }
}
impl DebugObserverControl for InstalledSignSink {
    type History = DebugObservationBuffer<DEBUG_RECORDS>;
    fn attach_debug_observer(
        &mut self,
        history: Self::History,
    ) -> Result<(), DebugObservationRefusal> {
        match self {
            Self::Plain(_) => Err(DebugObservationRefusal::ObserverDetached),
            Self::Observed(sign, _) => sign.attach(history),
        }
    }
    fn detach_debug_observer(&mut self) -> Result<Self::History, DebugObservationRefusal> {
        self.take_debug_history()
            .ok_or(DebugObservationRefusal::ObserverDetached)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{
        debug_observation::{DebugEventKind, DebugSubject},
        CordId,
    };
    fn sign() -> HostedSignLog {
        HostedSignLog::new(16, (16 * core::mem::size_of::<KernelEvent>()) as u32).unwrap()
    }
    fn scope() -> InstalledDebugScope {
        let mut types = [None; MAX_CORDS];
        types[0] = Some(11);
        types[1] = Some(22);
        InstalledDebugScope {
            execution: DebugExecutionIdentity {
                body: [1; 32],
                plan: [2; 32],
                play: [3; 32],
            },
            node_bindings: [DebugNodeBinding { host: 0, plot: 0 }; MAX_NODES],
            cord_types: types,
        }
    }
    #[test]
    fn exact_cord_types_distinguish_input_and_output_with_the_same_port_number() {
        let (mut sink, history) = InstalledSignSink::observed(sign(), scope()).unwrap();
        sink.attach_debug_observer(history).unwrap();
        for (cord, kind) in [
            (0, DebugEventKind::ValueReceived),
            (1, DebugEventKind::ValueSent),
        ] {
            sink.observe_debug(DebugRuntimeEvent {
                node: NodeId(0),
                port: Some(PortId(0)),
                cord: Some(CordId(cord)),
                kind,
                type_identity: Some(99),
                value: Some(b"value"),
                fault_code: None,
            });
        }
        let history = sink.take_debug_history().unwrap();
        assert_eq!(history.record(0).unwrap().type_identity, Some(11));
        assert_eq!(history.record(1).unwrap().type_identity, Some(22));
        assert_eq!(
            history.record(0).unwrap().subject,
            DebugSubject::Cord(CordId(0))
        );
        assert!(sink.take_debug_history().is_none());
    }
    #[test]
    fn debugger_pressure_changes_only_optional_history_and_preserves_mandatory_signs() {
        let mut plain = InstalledSignSink::Plain(sign());
        let (mut observed, history) = InstalledSignSink::observed(sign(), scope()).unwrap();
        observed.attach_debug_observer(history).unwrap();
        for kind in [
            KernelEventKind::HostCallRequested,
            KernelEventKind::HostCallCompleted,
        ] {
            assert_eq!(
                plain.record(NodeId(0), None, Some(RequestId(0)), kind),
                observed.record(NodeId(0), None, Some(RequestId(0)), kind)
            );
        }
        for _ in 0..DEBUG_RECORDS + 2 {
            observed.observe_debug(DebugRuntimeEvent {
                node: NodeId(0),
                port: Some(PortId(0)),
                cord: Some(CordId(0)),
                kind: DebugEventKind::ValueSent,
                type_identity: None,
                value: Some(b"actual"),
                fault_code: None,
            });
        }
        assert_eq!(
            plain.events().collect::<Vec<_>>(),
            observed.events().collect::<Vec<_>>()
        );
        assert_eq!(plain.allocation_capacity(), observed.allocation_capacity());
        let history = observed.take_debug_history().unwrap();
        assert_eq!(history.len(), DEBUG_RECORDS);
        assert_eq!(history.gap().unwrap().dropped_records, 2);
        assert!(!observed.requires_canonical_debug_values());
        assert!(plain.take_debug_history().is_none());
    }
}
