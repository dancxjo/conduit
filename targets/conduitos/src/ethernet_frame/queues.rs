//! Fixed Ethernet II queues shared by one stack adapter and its admitted owner.
use core::cell::RefCell;
pub const FRAME_BYTES: usize = super::contract::FRAME_DATA_BYTES as usize;
pub const FIFO_ITEMS: usize = 2;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameRefusal {
    Length,
    SourceMac,
    EtherType,
    Pressure,
    Stale,
    Cancelled,
    ProviderLost,
    Sequence,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lifecycle {
    Open,
    Cancelled,
    ProviderLost,
}
#[derive(Clone, Copy)]
struct Slot {
    wire: [u8; FRAME_BYTES],
    length: usize,
}
impl Slot {
    const EMPTY: Self = Self {
        wire: [0; FRAME_BYTES],
        length: 0,
    };
}
struct Fifo {
    slots: [Slot; FIFO_ITEMS],
    head: usize,
    count: usize,
}
impl Fifo {
    const fn new() -> Self {
        Self {
            slots: [Slot::EMPTY; FIFO_ITEMS],
            head: 0,
            count: 0,
        }
    }
    fn push(&mut self, wire: &[u8]) -> Result<(), FrameRefusal> {
        validate_frame(wire)?;
        if self.count == FIFO_ITEMS {
            return Err(FrameRefusal::Pressure);
        }
        let slot = &mut self.slots[(self.head + self.count) % FIFO_ITEMS];
        slot.wire[..wire.len()].copy_from_slice(wire);
        slot.length = wire.len();
        self.count += 1;
        Ok(())
    }
    fn front(&self) -> Result<&Slot, FrameRefusal> {
        if self.count == 0 {
            Err(FrameRefusal::Pressure)
        } else {
            Ok(&self.slots[self.head])
        }
    }
    fn pop(&mut self) {
        self.slots[self.head].length = 0;
        self.head = (self.head + 1) % FIFO_ITEMS;
        self.count -= 1;
    }
}
struct State {
    rx: Fifo,
    tx: Fifo,
    pending: Option<(u64, usize)>,
    next: u64,
    lifecycle: Lifecycle,
}
/// Move-only Root-owned storage. The Device and selected Call owner borrow the
/// same queues; neither a matching generation nor a MAC creates authority.
pub struct EthernetFrameQueues {
    generation: u64,
    mac: [u8; 6],
    state: RefCell<State>,
}
impl EthernetFrameQueues {
    pub fn prepare(generation: u64, mac: [u8; 6]) -> Result<Self, FrameRefusal> {
        if generation == 0 {
            return Err(FrameRefusal::Stale);
        }
        if mac == [0; 6] || mac[0] & 1 != 0 {
            return Err(FrameRefusal::SourceMac);
        }
        Ok(Self {
            generation,
            mac,
            state: RefCell::new(State {
                rx: Fifo::new(),
                tx: Fifo::new(),
                pending: None,
                next: 0,
                lifecycle: Lifecycle::Open,
            }),
        })
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    pub const fn mac(&self) -> [u8; 6] {
        self.mac
    }
    fn check(&self, generation: u64) -> Result<(), FrameRefusal> {
        if generation != self.generation {
            return Err(FrameRefusal::Stale);
        }
        match self.state.borrow().lifecycle {
            Lifecycle::Open => Ok(()),
            Lifecycle::Cancelled => Err(FrameRefusal::Cancelled),
            Lifecycle::ProviderLost => Err(FrameRefusal::ProviderLost),
        }
    }
    pub(crate) fn transmit_ready(&self, generation: u64) -> Result<(), FrameRefusal> {
        self.check(generation)?;
        if self.state.borrow().tx.count == FIFO_ITEMS {
            Err(FrameRefusal::Pressure)
        } else {
            Ok(())
        }
    }
    pub(crate) fn enqueue_transmit(
        &self,
        generation: u64,
        wire: &[u8],
    ) -> Result<(), FrameRefusal> {
        self.check(generation)?;
        self.state.borrow_mut().tx.push(wire)
    }
    pub(crate) fn receive(
        &self,
        generation: u64,
        output: &mut [u8; FRAME_BYTES],
    ) -> Result<usize, FrameRefusal> {
        self.check(generation)?;
        let mut state = self.state.borrow_mut();
        let slot = state.rx.front()?;
        let length = slot.length;
        output[..length].copy_from_slice(&slot.wire[..length]);
        state.rx.pop();
        Ok(length)
    }
    pub(crate) fn deliver_receive(&self, generation: u64, wire: &[u8]) -> Result<(), FrameRefusal> {
        self.check(generation)?;
        self.state.borrow_mut().rx.push(wire)
    }
    pub(crate) fn take_transmit(
        &self,
        generation: u64,
        output: &mut [u8; FRAME_BYTES],
    ) -> Result<(u64, usize), FrameRefusal> {
        self.check(generation)?;
        let mut state = self.state.borrow_mut();
        if state.pending.is_some() {
            return Err(FrameRefusal::Pressure);
        }
        let next = state.next.checked_add(1).ok_or(FrameRefusal::Sequence)?;
        let slot = state.tx.front()?;
        let length = slot.length;
        output[..length].copy_from_slice(&slot.wire[..length]);
        let ordinal = state.next;
        state.next = next;
        state.pending = Some((ordinal, length));
        Ok((ordinal, length))
    }
    pub(crate) fn acknowledge_transmit(
        &self,
        generation: u64,
        ordinal: u64,
        actual: u64,
    ) -> Result<(), FrameRefusal> {
        self.check(generation)?;
        let mut state = self.state.borrow_mut();
        let Some((expected, length)) = state.pending else {
            return Err(FrameRefusal::Stale);
        };
        if ordinal != expected {
            return Err(FrameRefusal::Stale);
        }
        if actual != length as u64 {
            return Err(FrameRefusal::Length);
        }
        state.tx.pop();
        state.pending = None;
        Ok(())
    }
    pub(crate) fn cancel(&self) {
        self.state.borrow_mut().lifecycle = Lifecycle::Cancelled;
    }
    pub(crate) fn provider_lost(&self) {
        let mut state = self.state.borrow_mut();
        if state.lifecycle == Lifecycle::Open {
            state.lifecycle = Lifecycle::ProviderLost;
        }
    }
}
/// This exact first frame profile is Ethernet II (no FCS/VLAN/jumbo). Higher
/// protocol and destination/MAC admission policy belong to the network owner.
pub fn validate_frame(wire: &[u8]) -> Result<(), FrameRefusal> {
    if !(14..=FRAME_BYTES).contains(&wire.len()) {
        return Err(FrameRefusal::Length);
    }
    if wire[6..12] == [0; 6] || wire[6] & 1 != 0 {
        return Err(FrameRefusal::SourceMac);
    }
    let ether_type = u16::from_be_bytes([wire[12], wire[13]]);
    if ether_type < 0x0600 || matches!(ether_type, 0x8100 | 0x88a8) {
        return Err(FrameRefusal::EtherType);
    }
    Ok(())
}
#[cfg(test)]
mod tests;
