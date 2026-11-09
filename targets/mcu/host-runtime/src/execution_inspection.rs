//! Bounded read-only execution inspection over the existing framed carrier.
//! Unsigned inspection is neither execution authority nor durable acceptance.
//! Only actual observer events may be recorded; losing this carrier must never
//! cancel or restart the independently owned local workset.
use crate::{execution_workset::StartedEvidence, transport::MessageTransport};
use conduit_kernel::{KernelEvent, KernelEventKind};
use core::{
    cell::Cell,
    fmt::{self, Write},
};
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
    AppendStatus,
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
/// Bounded UTF-8 diagnostic text; formatting never allocates or grants authority.
const REFUSAL_BYTES: usize = 96;
#[derive(Clone, Copy)]
struct RefusalDetail {
    phase: &'static str,
    text: [u8; REFUSAL_BYTES],
    length: u8,
    truncated: bool,
}
impl fmt::Write for RefusalDetail {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Ok(());
        }
        let available = REFUSAL_BYTES - usize::from(self.length);
        let mut length = value.len().min(available);
        while !value.is_char_boundary(length) {
            length -= 1;
        }
        let start = usize::from(self.length);
        self.text[start..start + length].copy_from_slice(&value.as_bytes()[..length]);
        self.length += length as u8;
        self.truncated = length < value.len();
        Ok(())
    }
}
impl Serialize for RefusalDetail {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct View<'a> {
            phase: &'static str,
            error: &'a str,
            truncated: bool,
        }
        View {
            phase: self.phase,
            error: core::str::from_utf8(&self.text[..usize::from(self.length)])
                .expect("formatter preserves UTF-8 boundaries"),
            truncated: self.truncated,
        }
        .serialize(serializer)
    }
}
/// Fixed storage for one last observed event per fragment. This never owns a
/// kernel or enables Start. Root initializes it only after successful Start.
pub struct State<const F: usize> {
    append: Cell<AppendInspection>,
    custody: [u8; 32],
    fragments: [Cell<Observed>; F],
    refusals: [Cell<Option<RefusalDetail>>; F],
}
impl<const F: usize> State<F> {
    pub fn new(started: &StartedEvidence) -> Self {
        Self::from_digest(*started.custody_identity())
    }
    fn from_digest(custody: [u8; 32]) -> Self {
        Self {
            append: Cell::new(AppendInspection::default()),
            custody,
            fragments: core::array::from_fn(|_| Cell::new(Observed::default())),
            refusals: core::array::from_fn(|_| Cell::new(None)),
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
    /// Record the actual local refusal before its actor stops. This is unsigned
    /// diagnostic evidence only; it cannot alter any execution or resource owner.
    pub fn observe_refusal(
        &self,
        fragment: usize,
        phase: &'static str,
        error: &impl fmt::Debug,
    ) -> Result<(), Refusal> {
        let slot = self.refusals.get(fragment).ok_or(Refusal::Fragment)?;
        let mut detail = RefusalDetail {
            phase,
            text: [0; REFUSAL_BYTES],
            length: 0,
            truncated: false,
        };
        if write!(&mut detail, "{error:?}").is_err() {
            detail.truncated = true;
        }
        slot.set(Some(detail));
        Ok(())
    }
    /// Serialize only fixed retained observations. Legacy [S,fragment,event]
    /// inspection can retrieve last-slot ordinal0; older slots are unavailable.
    pub fn encode(&self, request: &[u8], output: &mut [u8]) -> Result<usize, Refusal> {
        let fragments = Fragments(self);
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
            append: self.append.get(),
        };
        serde_json_core::to_slice(&report, output).map_err(|_| Refusal::Capacity)
    }
}
#[derive(Serialize)]
struct Report<'a, const F: usize> {
    schema: &'static str,
    assurance: &'static str,
    start: &'static str,
    custody_sha256: &'a str,
    fragments: &'a Fragments<'a, F>,
    selected: Option<Selected>,
    append: AppendInspection,
}
#[derive(Serialize)]
struct Fragment {
    observed_events: u64,
    completed_calls: u64,
    overwritten_slots: u64,
    last: Option<Event>,
    condition: Option<Observation>,
    refusal: Option<RefusalDetail>,
}
impl From<Observed> for Fragment {
    fn from(value: Observed) -> Self {
        Self {
            observed_events: value.events,
            completed_calls: value.completions,
            overwritten_slots: value.overwritten,
            last: value.last.map(Event::from),
            condition: value.condition,
            refusal: None,
        }
    }
}
// Stream one fragment at a time instead of copying all diagnostic arrays onto
// the response stack. Fixed retained state is the only source of these fields.
struct Fragments<'a, const F: usize>(&'a State<F>);
impl<const F: usize> Serialize for Fragments<'_, F> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(Some(F))?;
        for i in 0..F {
            let mut fragment = Fragment::from(self.0.fragments[i].get());
            fragment.refusal = self.0.refusals[i].get();
            sequence.serialize_element(&fragment)?;
        }
        sequence.end()
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

/// Serve read-only inspection using caller-owned response storage and an
/// optional synchronous encoder. Its borrow ends before carrier transmission;
/// the callback grants no execution, publication or recording authority.
pub async fn serve_with<T: MessageTransport, const F: usize>(
    state: &State<F>,
    carrier: &mut T,
    output: &mut [u8],
    mut encode: impl FnMut(&[u8], &mut [u8]) -> Result<Option<usize>, ()>,
) -> Result<(), Refusal<T::Error>> {
    let mut control = [0u8; CONTROL_BYTES];
    loop {
        let request = carrier
            .receive(&mut control)
            .await
            .map_err(Refusal::Carrier)?;
        let count = match encode(request, output).map_err(|_| Refusal::Capacity)? {
            Some(count) if count <= output.len() => count,
            Some(_) => return Err(Refusal::Capacity),
            None => state
                .encode(request, output)
                .map_err(|_| Refusal::Capacity)?,
        };
        carrier
            .send(&output[..count])
            .await
            .map_err(Refusal::Carrier)?;
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
    fn refusal_details_are_bounded_utf8_and_preserve_original_observations() {
        struct Long;
        impl fmt::Debug for Long {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for _ in 0..40 {
                    f.write_str("界")?;
                }
                Ok(())
            }
        }
        let state = State::<3>::from_digest([6; 32]);
        state.observe(1, Observation::Refused).unwrap();
        state.observe_refusal(1, "step", &Long).unwrap();
        assert_eq!(
            state.observe_refusal(3, "frame", &Long),
            Err(Refusal::Fragment)
        );
        let mut output = [0; RESPONSE_BYTES];
        let n = state.encode(b"?", &mut output).unwrap();
        let report: serde_json::Value = serde_json::from_slice(&output[..n]).unwrap();
        let fragment = &report["fragments"][1];
        assert_eq!(fragment["condition"], "refused");
        assert_eq!(fragment["refusal"]["phase"], "step");
        assert_eq!(fragment["refusal"]["truncated"], true);
        assert_eq!(
            fragment["refusal"]["error"].as_str().unwrap().len(),
            REFUSAL_BYTES
        );
        assert_eq!(fragment["observed_events"], 0);
        assert!(report["fragments"][0]["refusal"].is_null());
        state.observe_refusal(1, "begin", &"Face").unwrap();
        let n = state.encode(b"?", &mut output).unwrap();
        let report: serde_json::Value = serde_json::from_slice(&output[..n]).unwrap();
        assert_eq!(report["fragments"][1]["refusal"]["phase"], "begin");
        assert_eq!(report["fragments"][1]["refusal"]["error"], "\"Face\"");
        assert_eq!(report["fragments"][1]["refusal"]["truncated"], false);
        // Three retained slots remain below 400 extra inline bytes even on the
        // 64-bit test host; MCU slots are smaller. Encoding streams one slot.
        assert!(3 * core::mem::size_of::<Option<RefusalDetail>>() <= 400);
    }
    #[test]
    fn extended_inspection_encodes_synchronously_before_send_and_refuses_overflow() {
        struct Carrier;
        impl MessageTransport for Carrier {
            type Error = ();
            async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
                output[0] = b'F';
                Ok(&output[..1])
            }
            async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
                assert_eq!(bytes, b"retained");
                Err(())
            }
        }
        let state = State::<1>::from_digest([5; 32]);
        let calls = Cell::new(0);
        let mut output = [0; 8];
        let mut carrier = Carrier;
        let mut future = core::pin::pin!(serve_with(
            &state,
            &mut carrier,
            &mut output,
            |request, bytes| {
                assert_eq!(request, b"F");
                calls.set(calls.get() + 1);
                bytes.copy_from_slice(b"retained");
                Ok(Some(8))
            }
        ));
        let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
        use core::future::Future;
        assert_eq!(
            future.as_mut().poll(&mut cx),
            core::task::Poll::Ready(Err(Refusal::Carrier(())))
        );
        assert_eq!(calls.get(), 1);
        let mut carrier = Carrier;
        let mut output = [0; 8];
        let mut future = core::pin::pin!(serve_with(&state, &mut carrier, &mut output, |_, _| Ok(
            Some(9)
        )));
        assert_eq!(
            future.as_mut().poll(&mut cx),
            core::task::Poll::Ready(Err(Refusal::Capacity))
        );
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

/// A single fixed local Fore owner, funded before Start. This is not a Line or
/// a remote recording acknowledgment; it consumes only the exact selected ABI.
pub struct PreparedAppendStatus {
    boundary: Option<conduit_core::AssignedForeRequirement>,
    bytes: [u8; conduit_resource_snapshot::append_result::APPEND_RESULT_BYTES as usize],
    length: usize,
    pending: Option<(
        u64,
        conduit_resource_snapshot::append_result::DecodedAppendResult,
    )>,
}
impl PreparedAppendStatus {
    pub fn prepare(
        actual: &[conduit_core::AssignedForeRequirement],
        expected: Option<conduit_core::AssignedForeRequirement>,
    ) -> Result<Self, Refusal> {
        match (actual, expected) {
            ([], None) => (),
            ([actual], Some(expected))
                if actual.payload() == expected.payload()
                    && actual.direction() == conduit_core::AssignedForeDirection::Output =>
            {
                ()
            }
            _ => return Err(Refusal::AppendStatus),
        }
        Ok(Self {
            boundary: expected,
            bytes: [0; conduit_resource_snapshot::append_result::APPEND_RESULT_BYTES as usize],
            length: 0,
            pending: None,
        })
    }
    pub fn boundary(&self) -> Option<conduit_core::AssignedForeRequirement> {
        self.boundary
    }
    pub fn stage(&mut self, sequence: u64, bytes: &[u8]) -> Result<(), Refusal> {
        if let Some((prior, _)) = self.pending {
            return if prior == sequence && bytes == &self.bytes[..self.length] {
                Ok(())
            } else {
                Err(Refusal::AppendStatus)
            };
        }
        if bytes.len() > self.bytes.len() {
            return Err(Refusal::AppendStatus);
        }
        self.bytes[..bytes.len()].copy_from_slice(bytes);
        let outcome = conduit_resource_snapshot::append_result::decode(&self.bytes[..bytes.len()])
            .map_err(|_| Refusal::AppendStatus)?;
        self.length = bytes.len();
        self.pending = Some((sequence, outcome));
        Ok(())
    }
    pub fn delivered<const F: usize>(
        &mut self,
        state: &State<F>,
        sequence: u64,
    ) -> Result<(), Refusal> {
        let (prior, outcome) = self.pending.ok_or(Refusal::AppendStatus)?;
        if prior != sequence {
            return Err(Refusal::AppendStatus);
        }
        let mut next = state.append.get();
        use conduit_resource_snapshot::append_result::DecodedAppendResult::*;
        let (counter, label) = match outcome {
            Recorded(index) => {
                next.last_recorded_index = Some(index);
                (&mut next.recorded, "recorded")
            }
            Capacity => (&mut next.capacity, "capacity"),
            Uncertain => (&mut next.uncertain, "uncertain"),
            RecoveryRequired => (&mut next.recovery_required, "recovery-required"),
        };
        *counter = counter.checked_add(1).ok_or(Refusal::Capacity)?;
        next.last = Some(label);
        state.append.set(next);
        self.pending = None;
        Ok(())
    }
}
#[derive(Clone, Copy, Default, Serialize)]
struct AppendInspection {
    recorded: u64,
    capacity: u64,
    uncertain: u64,
    recovery_required: u64,
    last_recorded_index: Option<u32>,
    last: Option<&'static str>,
}

#[cfg(test)]
mod append_status_tests {
    use super::*;
    use conduit_core::{
        AssignedForeDirection as D, AssignedForeRequirement as F, AssignedIdentity as I,
    };
    #[test]
    fn exact_selected_output_abi_is_required_before_start() {
        let output = F::new(1, 2, D::Output, I::from_text("append_status"), [9; 32]);
        assert!(PreparedAppendStatus::prepare(&[], None).is_ok());
        assert!(PreparedAppendStatus::prepare(&[output], Some(output)).is_ok());
        assert!(PreparedAppendStatus::prepare(&[output], None).is_err());
        assert!(PreparedAppendStatus::prepare(&[], Some(output)).is_err());
        assert!(PreparedAppendStatus::prepare(&[output, output], Some(output)).is_err());
        let foreign = F::new(1, 2, D::Output, I::from_text("append_status"), [8; 32]);
        assert!(PreparedAppendStatus::prepare(&[foreign], Some(output)).is_err());
        let mut ready = PreparedAppendStatus::prepare(&[output], Some(output)).unwrap();
        assert!(ready.stage(0, b"invalid").is_err());
        assert!(ready.pending.is_none());
    }
}
