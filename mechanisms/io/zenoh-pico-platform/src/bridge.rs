use super::ring::Ring;

/// Local socket incarnation only; this is not a Host/Boot or authority token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SocketGeneration(u32);
impl SocketGeneration {
    pub const fn get(self) -> u32 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub ipv4: [u8; 4],
    pub port: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    TimedOut,
    Network,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Busy,
    Endpoint,
    Exhausted,
    Stale,
    State,
    Extent,
    Fault(Fault),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Bytes(usize),
    Pending,
    Eof,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Connecting,
    Open,
    PeerClosed,
    Failed(Fault),
    Closing,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkCommand {
    Connect {
        generation: SocketGeneration,
        endpoint: Endpoint,
        deadline_ms: u64,
    },
    Close {
        generation: SocketGeneration,
    },
}
/// One asynchronous write observation; duplicate/late completion cannot consume
/// a later publication in the same socket generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendTicket {
    generation: SocketGeneration,
    sequence: u32,
    maximum: usize,
}
impl SendTicket {
    pub const fn len(self) -> usize {
        self.maximum
    }
    pub const fn is_empty(self) -> bool {
        self.maximum == 0
    }
}
#[derive(Clone, Copy)]
enum State {
    Idle,
    Connecting {
        generation: SocketGeneration,
        endpoint: Endpoint,
        deadline_ms: u64,
        taken: bool,
    },
    Open(SocketGeneration),
    PeerClosed(SocketGeneration),
    Failed(SocketGeneration, Fault),
    Closing {
        generation: SocketGeneration,
        taken: bool,
    },
}
impl State {
    fn generation(self) -> Option<SocketGeneration> {
        match self {
            Self::Idle => None,
            Self::Connecting { generation, .. }
            | Self::Closing { generation, .. }
            | Self::Open(generation)
            | Self::PeerClosed(generation)
            | Self::Failed(generation, _) => Some(generation),
        }
    }
}

/// One finite socket lease. A caller-owned mutex/critical section protects each
/// short operation; it must never remain held across a socket await or C call.
pub struct SocketBridge<const RX: usize, const TX: usize> {
    state: State,
    last: u32,
    last_send: u32,
    pending_send: Option<SendTicket>,
    rx: Ring<RX>,
    tx: Ring<TX>,
}
impl<const RX: usize, const TX: usize> Default for SocketBridge<RX, TX> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const RX: usize, const TX: usize> SocketBridge<RX, TX> {
    pub const fn new() -> Self {
        Self {
            state: State::Idle,
            last: 0,
            last_send: 0,
            pending_send: None,
            rx: Ring::new(),
            tx: Ring::new(),
        }
    }
    pub fn generation(&self) -> Option<SocketGeneration> {
        self.state.generation()
    }
    pub fn readable(&self, generation: SocketGeneration) -> Result<bool, Refusal> {
        self.current(generation)?;
        match self.state {
            State::Open(_) => Ok(self.rx.len() != 0),
            State::PeerClosed(_) | State::Failed(_, _) => Ok(true),
            _ => Err(Refusal::State),
        }
    }
    pub fn phase(&self) -> Phase {
        match self.state {
            State::Idle => Phase::Idle,
            State::Connecting { .. } => Phase::Connecting,
            State::Open(_) => Phase::Open,
            State::PeerClosed(_) => Phase::PeerClosed,
            State::Failed(_, fault) => Phase::Failed(fault),
            State::Closing { .. } => Phase::Closing,
        }
    }
    pub fn request_connect(
        &mut self,
        endpoint: Endpoint,
        deadline_ms: u64,
    ) -> Result<SocketGeneration, Refusal> {
        if self.phase() != Phase::Idle {
            return Err(Refusal::Busy);
        }
        if endpoint.port == 0 {
            return Err(Refusal::Endpoint);
        }
        let next = self.last.checked_add(1).ok_or(Refusal::Exhausted)?;
        let generation = SocketGeneration(next);
        self.last = next;
        self.state = State::Connecting {
            generation,
            endpoint,
            deadline_ms,
            taken: false,
        };
        Ok(generation)
    }
    /// The socket owner takes each command once. A deadline is monotonic local
    /// preparation time; it is never an environmental observation timestamp.
    pub fn take_command(&mut self, now_ms: u64) -> Option<NetworkCommand> {
        match self.state {
            State::Connecting {
                generation,
                endpoint,
                deadline_ms,
                taken: false,
            } => {
                if now_ms >= deadline_ms {
                    self.state = State::Failed(generation, Fault::TimedOut);
                    None
                } else {
                    self.state = State::Connecting {
                        generation,
                        endpoint,
                        deadline_ms,
                        taken: true,
                    };
                    Some(NetworkCommand::Connect {
                        generation,
                        endpoint,
                        deadline_ms,
                    })
                }
            }
            State::Closing {
                generation,
                taken: false,
            } => {
                self.state = State::Closing {
                    generation,
                    taken: true,
                };
                Some(NetworkCommand::Close { generation })
            }
            _ => None,
        }
    }
    fn current(&self, generation: SocketGeneration) -> Result<(), Refusal> {
        if self.state.generation() != Some(generation) {
            return Err(Refusal::Stale);
        }
        Ok(())
    }
    pub fn complete_connect(
        &mut self,
        generation: SocketGeneration,
        now_ms: u64,
        result: Result<(), Fault>,
    ) -> Result<Phase, Refusal> {
        self.current(generation)?;
        let State::Connecting {
            deadline_ms,
            taken: true,
            ..
        } = self.state
        else {
            return Err(Refusal::State);
        };
        match result {
            _ if now_ms >= deadline_ms => self.state = State::Failed(generation, Fault::TimedOut),
            Ok(()) => self.state = State::Open(generation),
            Err(fault) => self.state = State::Failed(generation, fault),
        }
        Ok(self.phase())
    }
    pub fn request_close(&mut self, generation: SocketGeneration) -> Result<(), Refusal> {
        self.current(generation)?;
        if !matches!(self.state, State::Closing { .. }) {
            self.rx.clear();
            self.tx.clear();
            self.pending_send = None;
            self.state = State::Closing {
                generation,
                taken: false,
            };
        }
        Ok(())
    }
    /// Only after the actual socket is aborted/retired may another connect
    /// reuse its backing. A late connect result cannot resurrect this lease.
    pub fn complete_close(&mut self, generation: SocketGeneration) -> Result<(), Refusal> {
        self.current(generation)?;
        if !matches!(self.state, State::Closing { taken: true, .. }) {
            return Err(Refusal::State);
        }
        self.state = State::Idle;
        Ok(())
    }
    pub fn fail(&mut self, generation: SocketGeneration, fault: Fault) -> Result<(), Refusal> {
        self.current(generation)?;
        if !matches!(self.state, State::Connecting { .. } | State::Open(_)) {
            return Err(Refusal::State);
        }
        self.state = State::Failed(generation, fault);
        Ok(())
    }
    pub fn peer_closed(&mut self, generation: SocketGeneration) -> Result<(), Refusal> {
        self.current(generation)?;
        if self.phase() != Phase::Open {
            return Err(Refusal::State);
        }
        self.state = State::PeerClosed(generation);
        Ok(())
    }
    pub fn rx_room(&self, generation: SocketGeneration) -> Result<usize, Refusal> {
        self.current(generation)?;
        if self.phase() != Phase::Open {
            return Err(Refusal::State);
        }
        Ok(self.rx.room())
    }
    pub fn network_receive(
        &mut self,
        generation: SocketGeneration,
        bytes: &[u8],
    ) -> Result<usize, Refusal> {
        self.rx_room(generation)?;
        Ok(self.rx.push(bytes))
    }
    pub fn read(
        &mut self,
        generation: SocketGeneration,
        bytes: &mut [u8],
    ) -> Result<Progress, Refusal> {
        self.current(generation)?;
        if bytes.is_empty() {
            return Ok(Progress::Bytes(0));
        }
        if matches!(self.state, State::Closing { .. } | State::Connecting { .. }) {
            return Err(Refusal::State);
        }
        let count = self.rx.copy(bytes);
        if count != 0 {
            self.rx.consume(count);
            return Ok(Progress::Bytes(count));
        }
        match self.state {
            State::Open(_) => Ok(Progress::Pending),
            State::PeerClosed(_) => Ok(Progress::Eof),
            State::Failed(_, fault) => Err(Refusal::Fault(fault)),
            _ => Err(Refusal::State),
        }
    }
    pub fn write(
        &mut self,
        generation: SocketGeneration,
        bytes: &[u8],
    ) -> Result<Progress, Refusal> {
        self.current(generation)?;
        match self.state {
            State::Failed(_, fault) => return Err(Refusal::Fault(fault)),
            State::Open(_) => (),
            _ => return Err(Refusal::State),
        }
        if bytes.is_empty() {
            return Ok(Progress::Bytes(0));
        }
        let count = self.tx.push(bytes);
        Ok(if count == 0 {
            Progress::Pending
        } else {
            Progress::Bytes(count)
        })
    }
    /// Copy without consuming: an asynchronous partial socket write must retain
    /// the unsent suffix in this same fixed buffer.
    pub fn network_send(
        &mut self,
        generation: SocketGeneration,
        bytes: &mut [u8],
    ) -> Result<Option<SendTicket>, Refusal> {
        self.current(generation)?;
        if self.phase() != Phase::Open {
            return Err(Refusal::State);
        }
        if self.pending_send.is_some() {
            return Err(Refusal::Busy);
        }
        if bytes.is_empty() || self.tx.len() == 0 {
            return Ok(None);
        }
        let next = self.last_send.checked_add(1).ok_or(Refusal::Exhausted)?;
        let count = self.tx.copy(bytes);
        let ticket = SendTicket {
            generation,
            sequence: next,
            maximum: count,
        };
        self.last_send = next;
        self.pending_send = Some(ticket);
        Ok(Some(ticket))
    }
    pub fn network_sent(&mut self, ticket: SendTicket, count: usize) -> Result<(), Refusal> {
        self.current(ticket.generation)?;
        if self.pending_send != Some(ticket) {
            return Err(Refusal::Stale);
        }
        if self.phase() != Phase::Open {
            return Err(Refusal::State);
        }
        if count > ticket.maximum || count > self.tx.len() {
            return Err(Refusal::Extent);
        }
        self.tx.consume(count);
        self.pending_send = None;
        Ok(())
    }
    /// The caller first cancels its actual socket operation and establishes
    /// that zero bytes were accepted. No bytes are consumed here. If partial
    /// acceptance is unknown, fault and close this generation instead of
    /// replaying bytes in the same stream.
    pub fn cancel_send(&mut self, ticket: SendTicket) -> Result<(), Refusal> {
        self.current(ticket.generation)?;
        if self.pending_send != Some(ticket) {
            return Err(Refusal::Stale);
        }
        self.pending_send = None;
        Ok(())
    }
}
