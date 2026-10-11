//! The existing smoltcp Device seam, backed by the Root-owned fixed frame queues.
//! Protocol sockets/routes remain in the existing Interface; no new stack here.
use super::queues::*;
use core::cell::Cell;
use smoltcp::{
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    time::Instant,
};
pub struct EthernetFrameDevice<'a> {
    queues: &'a EthernetFrameQueues,
    receive: [u8; FRAME_BYTES],
    error: Cell<Option<FrameRefusal>>,
}
impl<'a> EthernetFrameDevice<'a> {
    pub fn prepare(queues: &'a EthernetFrameQueues) -> Self {
        Self {
            queues,
            receive: [0; FRAME_BYTES],
            error: Cell::new(None),
        }
    }
    pub fn take_error(&self) -> Option<FrameRefusal> {
        self.error.take()
    }
}
pub struct FrameRxToken<'a>(&'a [u8]);
impl RxToken for FrameRxToken<'_> {
    fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
        f(self.0)
    }
}
pub struct FrameTxToken<'a> {
    queues: &'a EthernetFrameQueues,
    error: &'a Cell<Option<FrameRefusal>>,
}
impl TxToken for FrameTxToken<'_> {
    fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, length: usize, f: F) -> R {
        let mut frame = [0; FRAME_BYTES];
        let usable = length.min(FRAME_BYTES);
        let result = f(&mut frame[..usable]);
        let submitted = if length > FRAME_BYTES {
            Err(FrameRefusal::Length)
        } else {
            self.queues
                .enqueue_transmit(self.queues.generation(), &frame[..length])
        };
        if let Err(error) = submitted {
            self.error.set(Some(error));
        }
        result
    }
}
impl Device for EthernetFrameDevice<'_> {
    type RxToken<'a>
        = FrameRxToken<'a>
    where
        Self: 'a;
    type TxToken<'a>
        = FrameTxToken<'a>
    where
        Self: 'a;
    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        // smoltcp receives a paired transmit token. Retain RX when the
        // fixed TX queue cannot honor that token's immediate response.
        match self.queues.transmit_ready(self.queues.generation()) {
            Ok(()) => {}
            Err(FrameRefusal::Pressure) => return None,
            Err(error) => {
                self.error.set(Some(error));
                return None;
            }
        }
        match self
            .queues
            .receive(self.queues.generation(), &mut self.receive)
        {
            Ok(length) => Some((
                FrameRxToken(&self.receive[..length]),
                FrameTxToken {
                    queues: self.queues,
                    error: &self.error,
                },
            )),
            Err(FrameRefusal::Pressure) => None,
            Err(error) => {
                self.error.set(Some(error));
                None
            }
        }
    }
    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        match self.queues.transmit_ready(self.queues.generation()) {
            Ok(()) => Some(FrameTxToken {
                queues: self.queues,
                error: &self.error,
            }),
            Err(FrameRefusal::Pressure) => None,
            Err(error) => {
                self.error.set(Some(error));
                None
            }
        }
    }
    fn capabilities(&self) -> DeviceCapabilities {
        let mut capabilities = DeviceCapabilities::default();
        capabilities.medium = Medium::Ethernet;
        capabilities.max_transmission_unit = FRAME_BYTES;
        capabilities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paired_receive_token_preserves_rx_under_transmit_pressure() {
        let queues = EthernetFrameQueues::prepare(7, [2, 1, 2, 3, 4, 5]).unwrap();
        let mut wire = [0; 64];
        wire[..6].fill(0xff);
        wire[6..12].copy_from_slice(&queues.mac());
        wire[12..14].copy_from_slice(&0x0800u16.to_be_bytes());
        let mut device = EthernetFrameDevice::prepare(&queues);
        for _ in 0..FIFO_ITEMS {
            device
                .transmit(Instant::from_millis(0))
                .unwrap()
                .consume(64, |output| output.copy_from_slice(&wire));
        }
        queues.deliver_receive(7, &wire).unwrap();
        assert!(device.receive(Instant::from_millis(0)).is_none());
        let mut out = [0; FRAME_BYTES];
        let (ordinal, length) = queues.take_transmit(7, &mut out).unwrap();
        queues
            .acknowledge_transmit(7, ordinal, length as u64)
            .unwrap();
        let (rx, _) = device.receive(Instant::from_millis(0)).unwrap();
        rx.consume(|bytes| assert_eq!(bytes, wire));
        assert_eq!(device.take_error(), None);
    }
}
