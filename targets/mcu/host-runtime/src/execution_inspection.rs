//! Bounded read-only execution inspection over the existing framed carrier.
//! Unsigned inspection is neither execution authority nor durable acceptance.
//! Only actual observer events may be recorded; losing this carrier must never
//! cancel or restart the independently owned local workset.
use crate::{execution_workset::StartedEvidence, transport::MessageTransport};
use conduit_kernel::{KernelEvent, KernelEventKind};
use core::cell::Cell;
use serde::Serialize;

// Preserve the existing bounded build-bound platform recovery exchange, which
// the carrier adapter intercepts before exposing ordinary inspection bytes.
pub const CONTROL_BYTES: usize = 128;
pub const RESPONSE_BYTES: usize = 2048;
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal<E = ()> {
    Fragment,
    Capacity,
    Event,
    Carrier(E),
}
/// A bounded observation of an actual actor result or pressure condition.
/// Reporting it cannot cancel work, release resources or authorize a restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Observation {
    Drained,
    Refused,
    Cancelled,
    EvidenceCapacity,
    FrameCapacity,
}
#[derive(Clone, Copy, Default)]
struct Observed {
    events: u64,
    completions: u64,
    overwritten: u64,
    last: Option<KernelEvent>,
    condition: Option<Observation>,
}
/// Fixed storage for one last observed event per fragment. This never owns a
/// kernel or enables Start. Root initializes it only after successful Start.
pub struct State<const F: usize> {
    custody: [u8; 32],
    fragments: [Cell<Observed>; F],
}
impl<const F: usize> State<F> {
    pub fn new(started: &StartedEvidence) -> Self {
        Self::from_digest(*started.custody_identity())
    }
    fn from_digest(custody: [u8; 32]) -> Self {
        Self {
            custody,
            fragments: core::array::from_fn(|_| Cell::new(Observed::default())),
        }
    }
    /// Call synchronously from the original kernel observer. Re-reading the
    /// same last event is harmless; this API does not invent or advance Signs.
    pub fn record(&self, fragment: usize, event: KernelEvent) -> Result<(), Refusal> {
        let slot = self.fragments.get(fragment).ok_or(Refusal::Fragment)?;
        let old = slot.get();
        if old.last == Some(event) {
            return Ok(());
        }
        let completed = event.kind == KernelEventKind::HostCallCompleted;
        if completed && event.request.is_none() {
            return Err(Refusal::Event);
        }
        let next = Observed {
            events: old.events.checked_add(1).ok_or(Refusal::Capacity)?,
            completions: old
                .completions
                .checked_add(u64::from(completed))
                .ok_or(Refusal::Capacity)?,
            overwritten: old
                .overwritten
                .checked_add(u64::from(old.last.is_some()))
                .ok_or(Refusal::Capacity)?,
            last: Some(event),
            condition: old.condition,
        };
        slot.set(next);
        Ok(())
    }
    /// Record only a real local actor result. There is deliberately no
    /// independent "Started" setter; original Start evidence initializes State.
    pub fn observe(&self, fragment: usize, condition: Observation) -> Result<(), Refusal> {
        let slot = self.fragments.get(fragment).ok_or(Refusal::Fragment)?;
        let mut observed = slot.get();
        observed.condition = Some(condition);
        slot.set(observed);
        Ok(())
    }
    /// Serialize only fixed retained observations. Legacy [S,fragment,event]
    /// inspection can retrieve last-slot ordinal0; older slots are unavailable.
    pub fn encode(&self, request: &[u8], output: &mut [u8]) -> Result<usize, Refusal> {
        let fragments: [Fragment; F] =
            core::array::from_fn(|i| Fragment::from(self.fragments[i].get()));
        let selected = if let [b'S', fragment, event] = request {
            let last = self
                .fragments
                .get(usize::from(*fragment))
                .and_then(|f| f.get().last);
            Some(Selected {
                fragment: *fragment,
                event: *event,
                available: *event == 0 && last.is_some(),
                last: if *event == 0 {
                    last.map(Event::from)
                } else {
                    None
                },
            })
        } else {
            None
        };
        let mut hex = [0u8; 64];
        for (i, byte) in self.custody.iter().enumerate() {
            hex[2 * i] = b"0123456789abcdef"[usize::from(byte >> 4)];
            hex[2 * i + 1] = b"0123456789abcdef"[usize::from(byte & 15)];
        }
        let report = Report {
            schema: "conduit.host/execution-inspection@1",
            assurance: "unsigned-inspection",
            start: "authenticated-start-observed",
            custody_sha256: core::str::from_utf8(&hex).map_err(|_| Refusal::Event)?,
            fragments: &fragments,
            selected,
        };
        serde_json_core::to_slice(&report, output).map_err(|_| Refusal::Capacity)
    }
}
#[derive(Serialize)]
struct Report<'a> {
    schema: &'static str,
    assurance: &'static str,
    start: &'static str,
    custody_sha256: &'a str,
    fragments: &'a [Fragment],
    selected: Option<Selected>,
}
#[derive(Serialize)]
struct Fragment {
    observed_events: u64,
    completed_calls: u64,
    overwritten_slots: u64,
    last: Option<Event>,
    condition: Option<Observation>,
}
impl From<Observed> for Fragment {
    fn from(value: Observed) -> Self {
        Self {
            observed_events: value.events,
            completed_calls: value.completions,
            overwritten_slots: value.overwritten,
            last: value.last.map(Event::from),
            condition: value.condition,
        }
    }
}
#[derive(Serialize)]
struct Selected {
    fragment: u8,
    event: u8,
    available: bool,
    last: Option<Event>,
}
#[derive(Serialize)]
struct Event {
    sequence: u32,
    node: u16,
    port: Option<u16>,
    request: Option<u32>,
    kind: &'static str,
}
impl From<KernelEvent> for Event {
    fn from(event: KernelEvent) -> Self {
        use KernelEventKind::*;
        let kind = match event.kind {
            StepFuelGranted => "StepFuelGranted",
            StepYielded => "StepYielded",
            StepFuelExceeded => "StepFuelExceeded",
            ValueStored => "ValueStored",
            ValueRouted => "ValueRouted",
            ValueConsumed => "ValueConsumed",
            RemoteValueOffered => "RemoteValueOffered",
            RemoteValueAccepted => "RemoteValueAccepted",
            RemoteValueDelivered => "RemoteValueDelivered",
            RemoteOutputClosed => "RemoteOutputClosed",
            RemoteInputAdmitted => "RemoteInputAdmitted",
            RemoteInputClosed => "RemoteInputClosed",
            InputClosed => "InputClosed",
            InputAbnormal => "InputAbnormal",
            HostCallRequested => "HostCallRequested",
            HostCallCancellationRequested => "HostCallCancellationRequested",
            HostCallCompleted => "HostCallCompleted",
            BackCompleted => "BackCompleted",
            BackFailed => "BackFailed",
            SemanticAbnormal => "SemanticAbnormal",
            SemanticAbnormalRecovered => "SemanticAbnormalRecovered",
            CancellationRequested => "CancellationRequested",
            RunCancelled => "RunCancelled",
        };
        Self {
            sequence: event.sequence,
            node: event.node.0,
            port: event.port.map(|p| p.0),
            request: event.request.map(|r| r.0),
            kind,
        }
    }
}
/// The caller owns connection/reset/retry policy. Retained command input is
/// untouched; these separate fixed buffers own only this control transaction.
pub async fn serve_one<T: MessageTransport, const F: usize>(
    state: &State<F>,
    carrier: &mut T,
) -> Result<(), Refusal<T::Error>> {
    let mut control = [0u8; CONTROL_BYTES];
    let request = carrier
        .receive(&mut control)
        .await
        .map_err(Refusal::Carrier)?;
    let mut output = [0u8; RESPONSE_BYTES];
    let count = state
        .encode(request, &mut output)
        .map_err(|_| Refusal::Capacity)?;
    carrier
        .send(&output[..count])
        .await
        .map_err(Refusal::Carrier)
}
pub async fn serve<T: MessageTransport, const F: usize>(
    state: &State<F>,
    carrier: &mut T,
) -> Result<(), Refusal<T::Error>> {
    loop {
        serve_one(state, carrier).await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::{NodeId, RequestId};
    #[test]
    fn inspection_counts_only_observed_completions_and_discloses_overwrite() {
        let state = State::<1>::from_digest([7; 32]);
        let mut event = KernelEvent {
            sequence: 4,
            node: NodeId(2),
            port: None,
            request: Some(RequestId(9)),
            kind: KernelEventKind::HostCallRequested,
        };
        state.record(0, event).unwrap();
        state.record(0, event).unwrap();
        event.sequence = 5;
        event.kind = KernelEventKind::HostCallCompleted;
        state.record(0, event).unwrap();
        state.observe(0, Observation::EvidenceCapacity).unwrap();
        let mut output = [0; RESPONSE_BYTES];
        let n = state.encode(&[b'S', 0, 1], &mut output).unwrap();
        let report: serde_json::Value = serde_json::from_slice(&output[..n]).unwrap();
        assert_eq!(report["fragments"][0]["observed_events"], 2);
        assert_eq!(report["fragments"][0]["completed_calls"], 1);
        assert_eq!(report["fragments"][0]["overwritten_slots"], 1);
        assert_eq!(report["selected"]["available"], false);
        assert_eq!(report["fragments"][0]["condition"], "evidence-capacity");
        assert_eq!(state.record(1, event), Err(Refusal::Fragment));
        event.request = None;
        assert_eq!(state.record(0, event), Err(Refusal::Event));
        assert_eq!(state.fragments[0].get().completions, 1);
        assert_eq!(state.encode(b"?", &mut [0; 8]), Err(Refusal::Capacity));
    }
    #[test]
    fn carrier_failure_preserves_observed_execution() {
        struct Disconnected;
        impl MessageTransport for Disconnected {
            type Error = ();
            async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
                output[0] = b'?';
                Ok(&output[..1])
            }
            async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
                Err(())
            }
        }
        let state = State::<1>::from_digest([9; 32]);
        let event = KernelEvent {
            sequence: 1,
            node: NodeId(0),
            port: None,
            request: Some(RequestId(1)),
            kind: KernelEventKind::HostCallCompleted,
        };
        state.record(0, event).unwrap();
        let mut carrier = Disconnected;
        let mut future = core::pin::pin!(serve_one(&state, &mut carrier));
        let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
        use core::future::Future;
        assert_eq!(
            future.as_mut().poll(&mut cx),
            core::task::Poll::Ready(Err(Refusal::Carrier(())))
        );
        assert_eq!(state.fragments[0].get().last, Some(event));
        assert_eq!(state.fragments[0].get().completions, 1);
    }
}
