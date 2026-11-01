//! Finite copied canonical Fore custody. Transport delivery is not recording.
use super::*;
use conduit_std_host::body_execution::{BodyBorrowedForeOutputAdapter, BodyForeDeliveryRef};
use std::sync::mpsc::{self, Receiver, SyncSender};

pub(in super::super::super) struct LiveFore {
    pub scope: Arc<CompletedScope>,
    pub sequence: u64,
    pub length: usize,
    pub bytes: [u8; MAXIMUM_BYTES],
}
pub(in super::super::super) struct LiveCapture {
    port: conduit_core::PortId,
    kind: conduit_core::KindId,
    contract: ObservationContract,
    next: u64,
    sender: SyncSender<LiveFore>,
    pub receiver: Option<Receiver<LiveFore>>,
    pub scope: Arc<Mutex<Option<Arc<CompletedScope>>>>,
}
impl LiveCapture {
    pub fn prepare(port: conduit_core::PortId, kind: conduit_core::KindId) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        Ok(Self {
            port,
            kind,
            contract: ObservationContract::prepare().map_err(super::super::super::debug)?,
            next: 0,
            sender,
            receiver: Some(receiver),
            scope: Arc::new(Mutex::new(None)),
        })
    }
}
impl BodyBorrowedForeOutputAdapter for LiveCapture {
    fn deliver(&mut self, value: BodyForeDeliveryRef<'_>) -> Result<(), String> {
        if value.front_port_id != &self.port
            || value.value_kind != &self.kind
            || value.track != conduit_core::ConnectionTrack::Payload
            || value.sequence != self.next
            || value.bytes.len() > MAXIMUM_BYTES
            || self.contract.decode(value.bytes).is_err()
        {
            return Err("live Source Fore contract or order differs".into());
        }
        let scope = self
            .scope
            .lock()
            .map_err(|_| "native Source scope poisoned")?
            .as_ref()
            .ok_or("live Fore lacks exact native completion")?
            .clone();
        let mut message = LiveFore {
            scope,
            sequence: value.sequence,
            length: value.bytes.len(),
            bytes: [0; MAXIMUM_BYTES],
        };
        message.bytes[..message.length].copy_from_slice(value.bytes);
        let next = self
            .next
            .checked_add(1)
            .ok_or("live Source Fore sequence exhausted")?;
        self.sender
            .try_send(message)
            .map_err(|_| "live Source consumer unavailable or finite slot occupied")?;
        self.next = next;
        Ok(())
    }
}
