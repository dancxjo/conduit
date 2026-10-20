//! Optional logical binary-message and explicit API copy counters.
//! Excludes WebSocket/TCP framing, allocator internals and kernel/OS copies.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct NativeWebSocketTraffic {
    pub sent_messages: u64,
    pub sent_binary_bytes: u64,
    pub received_messages: u64,
    pub received_binary_bytes: u64,
    pub transmit_copy_bytes: u64,
    pub receive_copy_bytes: u64,
    pub saturated: bool,
}
impl NativeWebSocketTraffic {
    fn add(counter: &mut u64, amount: u64, saturated: &mut bool) {
        if let Some(sum) = counter.checked_add(amount) {
            *counter = sum;
        } else {
            *counter = u64::MAX;
            *saturated = true;
        }
    }
    pub(super) fn transmit_copy(&mut self, length: usize) {
        Self::add(
            &mut self.transmit_copy_bytes,
            length as u64,
            &mut self.saturated,
        );
    }
    pub(super) fn sent(&mut self, length: usize) {
        Self::add(&mut self.sent_messages, 1, &mut self.saturated);
        Self::add(
            &mut self.sent_binary_bytes,
            length as u64,
            &mut self.saturated,
        );
    }
    pub(super) fn received(&mut self, length: usize) {
        Self::add(&mut self.received_messages, 1, &mut self.saturated);
        Self::add(
            &mut self.received_binary_bytes,
            length as u64,
            &mut self.saturated,
        );
    }
    pub(super) fn receive_copy(&mut self, length: usize) {
        Self::add(
            &mut self.receive_copy_bytes,
            length as u64,
            &mut self.saturated,
        );
    }
}
