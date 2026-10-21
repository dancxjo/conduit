//! Guest-owned finite Ethernet staging; no hardware access or endpoint authority.
use crate::{FRAME_BYTES, Refusal, buffer};
use alloc::vec::Vec;
use core::cell::RefCell;
use smoltcp::{
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    time::Instant,
};
const QUEUE: usize = 4;
struct Frame {
    bytes: Vec<u8>,
    length: usize,
    id: u32,
    taken: bool,
}
struct Queue {
    frames: [Frame; QUEUE],
    count: usize,
    next: u32,
}
impl Queue {
    fn new() -> Result<Self, Refusal> {
        Ok(Self {
            frames: [frame()?, frame()?, frame()?, frame()?],
            count: 0,
            next: 1,
        })
    }
    fn available(&self) -> bool {
        self.count < QUEUE && self.next < u32::MAX
    }
}
fn frame() -> Result<Frame, Refusal> {
    Ok(Frame {
        bytes: buffer(FRAME_BYTES)?,
        length: 0,
        id: 0,
        taken: false,
    })
}
pub(crate) struct Adapter {
    ingress: Vec<u8>,
    length: usize,
    consumed: bool,
    queue: RefCell<Queue>,
}
impl Adapter {
    pub fn new() -> Result<Self, Refusal> {
        Ok(Self {
            ingress: buffer(FRAME_BYTES)?,
            length: 0,
            consumed: true,
            queue: RefCell::new(Queue::new()?),
        })
    }
    pub fn supply(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        if !self.empty() || bytes.len() > FRAME_BYTES {
            return Err(Refusal::Order);
        }
        self.ingress[..bytes.len()].copy_from_slice(bytes);
        self.length = bytes.len();
        self.consumed = bytes.is_empty();
        Ok(())
    }
    pub fn empty(&self) -> bool {
        self.queue.borrow().count == 0
    }
    pub fn pending(&self) -> (u32, u32) {
        let q = self.queue.borrow();
        (
            q.count as u32,
            if q.count == 0 { 0 } else { q.frames[0].id },
        )
    }
    pub fn take(&self, id: u32, out: &mut [u8]) -> Result<usize, Refusal> {
        let mut q = self.queue.borrow_mut();
        if q.count == 0 || q.frames[0].id != id || q.frames[0].taken {
            return Err(Refusal::Order);
        }
        let f = &mut q.frames[0];
        let n = f.length;
        out.get_mut(..n)
            .ok_or(Refusal::Bounds)?
            .copy_from_slice(&f.bytes[..n]);
        f.taken = true;
        Ok(n)
    }
    pub fn ack(&mut self, id: u32) -> Result<(), Refusal> {
        let mut q = self.queue.borrow_mut();
        if q.count == 0 || q.frames[0].id != id || !q.frames[0].taken {
            return Err(Refusal::Order);
        }
        q.frames[0].bytes.fill(0);
        q.frames[0].length = 0;
        q.frames[0].taken = false;
        let count = q.count;
        q.frames[..count].rotate_left(1);
        q.count -= 1;
        Ok(())
    }
}
pub(crate) struct Receive<'a>(&'a [u8]);
pub(crate) struct Transmit<'a>(&'a RefCell<Queue>);
impl RxToken for Receive<'_> {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(self.0)
    }
}
impl TxToken for Transmit<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut q = self.0.borrow_mut();
        // Token creation reserved finite capacity. Only canonical smoltcp chooses length.
        assert!(q.available() && len <= FRAME_BYTES);
        let at = q.count;
        let id = q.next;
        let frame = &mut q.frames[at];
        let result = f(&mut frame.bytes[..len]);
        frame.length = len;
        frame.id = id;
        frame.taken = false;
        q.next += 1;
        q.count += 1;
        result
    }
}
impl Device for Adapter {
    type RxToken<'a> = Receive<'a>;
    type TxToken<'a> = Transmit<'a>;
    fn receive(&mut self, _: Instant) -> Option<(Receive<'_>, Transmit<'_>)> {
        if self.consumed || !self.queue.borrow().available() {
            return None;
        }
        self.consumed = true;
        Some((Receive(&self.ingress[..self.length]), Transmit(&self.queue)))
    }
    fn transmit(&mut self, _: Instant) -> Option<Transmit<'_>> {
        self.queue
            .borrow()
            .available()
            .then_some(Transmit(&self.queue))
    }
    fn capabilities(&self) -> DeviceCapabilities {
        let mut c = DeviceCapabilities::default();
        c.medium = Medium::Ethernet;
        c.max_transmission_unit = FRAME_BYTES;
        c
    }
}
