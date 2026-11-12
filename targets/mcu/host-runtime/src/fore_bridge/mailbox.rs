//! One item per direction; network IO must occur outside mailbox/kernel loans.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    IngressAccepted(u64),
    IngressDelivered(u64),
    IngressPressure(u64),
    EgressOffered { sequence: u64, length: usize },
}
#[derive(Clone, Copy)]
pub(super) enum Input {
    Empty,
    Offered { sequence: u64, length: usize },
    Accepted(u64),
    Delivered(u64),
    Pressure(u64),
}
#[derive(Clone, Copy)]
pub(super) enum OutputPhase {
    Ready,
    AwaitPeer,
    PeerAccepted,
    Accepted,
    PeerDelivered,
}
#[derive(Clone, Copy)]
pub(super) enum Output {
    Empty,
    Offered {
        sequence: u64,
        length: usize,
        phase: OutputPhase,
    },
}
pub(super) struct State<'b, 'p> {
    pub active: bool,
    pub ingress: PrimitiveIngressSpec<'p>,
    pub egress: EgressSpec<'p>,
    pub ingress_bytes: &'b mut [u8],
    pub egress_bytes: &'b mut [u8],
    pub input: Input,
    pub output: Output,
    pub next_input: u64,
}
impl<'b, 'p> State<'b, 'p> {
    pub fn new(
        ingress: PrimitiveIngressSpec<'p>,
        egress: EgressSpec<'p>,
        ingress_bytes: &'b mut [u8],
        egress_bytes: &'b mut [u8],
    ) -> Self {
        Self {
            active: false,
            ingress,
            egress,
            ingress_bytes,
            egress_bytes,
            input: Input::Empty,
            output: Output::Empty,
            next_input: 0,
        }
    }
    fn check(&self, retired: bool) -> Result<(), Refusal> {
        if !self.active || retired {
            Err(Refusal::Inactive)
        } else {
            Ok(())
        }
    }
}
/// Obtain only from the Root-activated bound bridge. Each method is synchronous;
/// caller checks existing Wire SessionMachine BEFORE submitting peer events.
/// No public message, hash or mailbox constructor can mint this port.
pub struct CarrierPort<'a, 'b, 'p> {
    pub(super) shared: &'a RefCell<State<'b, 'p>>,
    pub(super) retired: &'a Cell<bool>,
}
impl CarrierPort<'_, '_, '_> {
    pub fn stage_ingress(&self, sequence: u64, bytes: &[u8]) -> Result<(), Refusal> {
        let mut state = self.shared.try_borrow_mut().map_err(|_| Refusal::Busy)?;
        state.check(self.retired.get())?;
        if !matches!(state.input, Input::Empty) {
            return Err(Refusal::Busy);
        }
        if sequence != state.next_input || sequence == u64::MAX {
            return Err(Refusal::Sequence);
        }
        state.ingress.validate(bytes)?;
        state.ingress_bytes[..bytes.len()].copy_from_slice(bytes);
        state.input = Input::Offered {
            sequence,
            length: bytes.len(),
        };
        Ok(())
    }
    pub fn next_event(&self) -> Result<Option<Event>, Refusal> {
        let state = self.shared.try_borrow().map_err(|_| Refusal::Busy)?;
        state.check(self.retired.get())?;
        Ok(match state.input {
            Input::Accepted(s) => Some(Event::IngressAccepted(s)),
            Input::Delivered(s) => Some(Event::IngressDelivered(s)),
            Input::Pressure(s) => Some(Event::IngressPressure(s)),
            _ => match state.output {
                Output::Offered {
                    sequence,
                    length,
                    phase: OutputPhase::Ready,
                } => Some(Event::EgressOffered { sequence, length }),
                _ => None,
            },
        })
    }
    /// Encode/copy synchronously into the original IO frame buffer, THEN release
    /// this loan before any network await. Only one bounded mailbox copy exists;
    /// the original kernel Value remains queued until genuine peer delivery.
    pub fn with_egress<R>(
        &self,
        sequence: u64,
        use_bytes: impl FnOnce(&[u8]) -> R,
    ) -> Result<R, Refusal> {
        let state = self.shared.try_borrow().map_err(|_| Refusal::Busy)?;
        state.check(self.retired.get())?;
        match state.output {
            Output::Offered {
                sequence: s,
                length,
                phase: OutputPhase::Ready,
            } if s == sequence => Ok(use_bytes(&state.egress_bytes[..length])),
            _ => Err(Refusal::Sequence),
        }
    }
    /// Call only after the exact frame was successfully sent on the actual
    /// carrier. Local serialization or a failed write is not a sent event.
    pub fn sent(&self, event: Event) -> Result<(), Refusal> {
        let mut state = self.shared.try_borrow_mut().map_err(|_| Refusal::Busy)?;
        state.check(self.retired.get())?;
        match (event, state.input, state.output) {
            (Event::IngressAccepted(s), Input::Accepted(t), _) if s == t => {
                state.input = Input::Delivered(s)
            }
            (Event::IngressDelivered(s), Input::Delivered(t), _) if s == t => {
                state.next_input = s + 1;
                state.input = Input::Empty;
            }
            (Event::IngressPressure(s), Input::Pressure(t), _) if s == t => {
                state.input = Input::Empty
            }
            (
                Event::EgressOffered {
                    sequence: s,
                    length: n,
                },
                _,
                Output::Offered {
                    sequence: t,
                    length,
                    phase: OutputPhase::Ready,
                },
            ) if s == t && n == length => {
                state.output = Output::Offered {
                    sequence: s,
                    length,
                    phase: OutputPhase::AwaitPeer,
                }
            }
            _ => return Err(Refusal::Sequence),
        }
        Ok(())
    }
    pub fn peer_accepted(&self, sequence: u64) -> Result<(), Refusal> {
        self.peer(sequence, 0)
    }
    pub fn peer_delivered(&self, sequence: u64) -> Result<(), Refusal> {
        self.peer(sequence, 1)
    }
    pub fn peer_pressure(&self, sequence: u64) -> Result<(), Refusal> {
        self.peer(sequence, 2)
    }
    fn peer(&self, sequence: u64, event: u8) -> Result<(), Refusal> {
        let mut state = self.shared.try_borrow_mut().map_err(|_| Refusal::Busy)?;
        state.check(self.retired.get())?;
        let Output::Offered {
            sequence: s,
            length,
            phase,
        } = state.output
        else {
            return Err(Refusal::Sequence);
        };
        if s != sequence {
            return Err(Refusal::Sequence);
        }
        let phase = match (event, phase) {
            (0, OutputPhase::AwaitPeer) => OutputPhase::PeerAccepted,
            (1, OutputPhase::PeerAccepted | OutputPhase::Accepted) => OutputPhase::PeerDelivered,
            (2, OutputPhase::AwaitPeer) => OutputPhase::Ready,
            _ => return Err(Refusal::Sequence),
        };
        state.output = Output::Offered {
            sequence,
            length,
            phase,
        };
        Ok(())
    }
    /// Carrier loss/cancellation retires ONLY this bridge. It neither releases
    /// undelivered kernel Values nor cancels independent sampling/display work.
    pub fn retire(&self) -> Result<(), Refusal> {
        self.retired.set(true);
        Ok(())
    }
}
impl Drop for CarrierPort<'_, '_, '_> {
    fn drop(&mut self) {
        self.retired.set(true);
    }
}
