//! Preserve the initialized listener's actual TLS binding for startup authentication.
use super::*;
pub(super) struct BoundLine {
    line: SecureWebSocketLine,
    binding: [u8; 32],
}
impl BoundLine {
    pub(super) fn new(line: SecureWebSocketLine, binding: [u8; 32]) -> Self {
        Self { line, binding }
    }
}
impl RendezvousLine for BoundLine {
    fn receive(&mut self) -> Result<Vec<u8>, String> {
        RendezvousLine::receive(&mut self.line)
    }
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        RendezvousLine::send(&mut self.line, bytes)
    }
    fn close(&mut self) -> Result<(), String> {
        RendezvousLine::close(&mut self.line)
    }
    fn enter_retained_idle(&mut self) -> Result<(), String> {
        RendezvousLine::enter_retained_idle(&mut self.line)
    }
    fn poll_receive(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        RendezvousLine::poll_receive(&mut self.line, timeout)
    }
    fn supports_interruptible_exchange(&self) -> bool {
        true
    }
    fn startup_authentication_binding(&self) -> Option<[u8; 32]> {
        Some(self.binding)
    }
}
