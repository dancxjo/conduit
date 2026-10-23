//! Fixed single-request framing; no scheduling or native-session authority.
use super::NativeBackingRefusal;
use std::io::{self, Read, Write};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Pending,
    Ready,
    Retired,
}
pub(super) struct Exchange {
    phase: Phase,
    request: [u8; 9],
    reply: [u8; 8],
    sent: usize,
    received: usize,
}
impl Exchange {
    pub(super) fn new() -> Self {
        Self {
            phase: Phase::Idle,
            request: [0; 9],
            reply: [0; 8],
            sent: 0,
            received: 0,
        }
    }
    pub(super) fn begin(&mut self, sequence: u32, value: f32) -> Result<(), NativeBackingRefusal> {
        if self.phase != Phase::Idle {
            return Err(NativeBackingRefusal::Protocol);
        }
        self.request[0] = 1;
        self.request[1..5].copy_from_slice(&sequence.to_le_bytes());
        self.request[5..].copy_from_slice(&value.to_bits().to_le_bytes());
        self.reply.fill(0);
        self.sent = 0;
        self.received = 0;
        self.phase = Phase::Pending;
        Ok(())
    }
    pub(super) fn wants_write(&self) -> bool {
        self.phase == Phase::Pending && self.sent < 9 || self.phase == Phase::Ready
    }
    pub(super) fn poll(
        &mut self,
        input: &mut impl Write,
        output: &mut impl Read,
    ) -> Result<bool, NativeBackingRefusal> {
        match self.phase {
            Phase::Ready => return Ok(true),
            Phase::Pending => {}
            _ => return Err(NativeBackingRefusal::Protocol),
        }
        if self.sent < self.request.len() {
            match input.write(&self.request[self.sent..]) {
                Ok(0) => return Err(NativeBackingRefusal::Io),
                Ok(n) => self.sent += n,
                Err(error) if pending(&error) => return Ok(false),
                Err(_) => return Err(NativeBackingRefusal::Io),
            }
            if self.sent < self.request.len() {
                return Ok(false);
            }
        }
        match output.read(&mut self.reply[self.received..]) {
            Ok(0) => return Err(NativeBackingRefusal::ProviderLost),
            Ok(n) => self.received += n,
            Err(error) if pending(&error) => return Ok(false),
            Err(_) => return Err(NativeBackingRefusal::Io),
        }
        if self.received < self.reply.len() {
            return Ok(false);
        }
        if self.reply[..4] != self.request[1..5] {
            return Err(NativeBackingRefusal::Protocol);
        }
        if self.reply[4..] != self.request[5..] {
            return Err(NativeBackingRefusal::InvalidOutput);
        }
        self.phase = Phase::Ready;
        Ok(true)
    }
    pub(super) fn take(
        &mut self,
        input: &mut impl Write,
    ) -> Result<Option<f32>, NativeBackingRefusal> {
        if self.phase != Phase::Ready {
            return Ok(None);
        }
        match input.write(b"C") {
            Ok(1) => {}
            Ok(_) => return Err(NativeBackingRefusal::Io),
            Err(error) if pending(&error) => return Ok(None),
            Err(_) => return Err(NativeBackingRefusal::Io),
        }
        let value = f32::from_bits(u32::from_le_bytes(self.reply[4..].try_into().unwrap()));
        self.phase = Phase::Idle;
        self.reply.fill(0);
        Ok(Some(value))
    }
    pub(super) fn revoke(&mut self) {
        self.phase = Phase::Retired;
        self.reply.fill(0);
        self.request.fill(0);
    }
}
fn pending(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}
#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
