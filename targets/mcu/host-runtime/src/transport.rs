//! The existing length-prefixed Conduit carrier, independent of USB or UART.
use conduit_wire::stream_framing::StreamFrameDecoder;
pub const LIMIT: usize = 4096;
pub const PACKET_BYTES: usize = 64;
pub const READ_TIMEOUT_MILLIS: u32 = 30_000;
pub const WRITE_TIMEOUT_MILLIS: u32 = 2_000;

/// Platform-owned IO. Every read/write must enforce the passed local deadline.
/// Cancellation must stop using the borrowed buffer before its future returns
/// or is dropped. Empty writes terminate USB packets; UART may treat them as no-op.
#[allow(async_fn_in_trait)]
pub trait Carrier {
    type Error;
    async fn connected(&mut self);
    async fn read(&mut self, output: &mut [u8], timeout_millis: u32) -> Result<usize, Self::Error>;
    async fn write(&mut self, bytes: &[u8], timeout_millis: u32) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Carrier,
    Framing,
    OutputExtent,
    Document,
    PayloadExtent,
}

pub struct Link<C> {
    pub(crate) carrier: C,
    // Fixed payload ceiling, its wire header, and one carrier read packet.
    decoder: StreamFrameDecoder<{ LIMIT + 2 + PACKET_BYTES }>,
}
impl<C: Carrier> Link<C> {
    pub fn new(carrier: C) -> Self {
        Self {
            carrier,
            decoder: StreamFrameDecoder::new(LIMIT).expect("fixed nonzero extent"),
        }
    }
    pub async fn connected(&mut self) {
        self.carrier.connected().await;
    }
    pub fn reset(&mut self) {
        self.decoder = StreamFrameDecoder::new(LIMIT).expect("fixed extent");
    }
    pub async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], Refusal> {
        let mut packet = [0; PACKET_BYTES];
        loop {
            if let Some(frame) = self.decoder.next_frame().map_err(|_| Refusal::Framing)? {
                let target = output.get_mut(..frame.len()).ok_or(Refusal::OutputExtent)?;
                target.copy_from_slice(frame);
                return Ok(target);
            }
            let n = self
                .carrier
                .read(&mut packet, READ_TIMEOUT_MILLIS)
                .await
                .map_err(|_| Refusal::Carrier)?;
            let bytes = packet.get(..n).ok_or(Refusal::OutputExtent)?;
            self.decoder
                .accept_bytes(bytes)
                .map_err(|_| Refusal::Framing)?;
        }
    }
    /// No second complete frame or document buffer. The caller retains the
    /// document assembly and its whole-transaction deadline, as on the USB path.
    pub async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), Refusal> {
        let mut packet = [0; PACKET_BYTES];
        loop {
            if let Some(frame) = self.decoder.next_frame().map_err(|_| Refusal::Framing)? {
                if document.push(frame).map_err(|_| Refusal::Document)? {
                    return Ok(());
                }
                continue;
            }
            let n = self
                .carrier
                .read(&mut packet, READ_TIMEOUT_MILLIS)
                .await
                .map_err(|_| Refusal::Carrier)?;
            let bytes = packet.get(..n).ok_or(Refusal::OutputExtent)?;
            self.decoder
                .accept_bytes(bytes)
                .map_err(|_| Refusal::Framing)?;
        }
    }
    pub async fn send(&mut self, payload: &[u8]) -> Result<(), Refusal> {
        if payload.is_empty() || payload.len() > LIMIT {
            return Err(Refusal::PayloadExtent);
        }
        self.carrier
            .write(&(payload.len() as u16).to_be_bytes(), WRITE_TIMEOUT_MILLIS)
            .await
            .map_err(|_| Refusal::Carrier)?;
        for packet in payload.chunks(PACKET_BYTES) {
            self.carrier
                .write(packet, WRITE_TIMEOUT_MILLIS)
                .await
                .map_err(|_| Refusal::Carrier)?;
        }
        if payload.len().is_multiple_of(PACKET_BYTES) {
            self.carrier
                .write(&[], WRITE_TIMEOUT_MILLIS)
                .await
                .map_err(|_| Refusal::Carrier)?;
        }
        Ok(())
    }
}

/// Already framed protocol messages. Platform control frames may be intercepted
/// before delivery (for example RP2040 BOOTSEL) without changing Body semantics.
#[allow(async_fn_in_trait)]
pub trait MessageTransport {
    type Error;
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], Self::Error>;
    async fn send(&mut self, payload: &[u8]) -> Result<(), Self::Error>;
}
impl<C: Carrier> MessageTransport for Link<C> {
    type Error = Refusal;
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], Self::Error> {
        Link::receive(self, output).await
    }
    async fn send(&mut self, payload: &[u8]) -> Result<(), Self::Error> {
        Link::send(self, payload).await
    }
}

/// Existing bounded document chunks over the same framed channel, without a
/// second full document buffer. The caller owns its whole-transaction deadline.
#[allow(async_fn_in_trait)]
pub trait DocumentTransport: MessageTransport {
    async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), Self::Error>;
}
impl<C: Carrier> DocumentTransport for Link<C> {
    async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), Self::Error> {
        Link::receive_document(self, document).await
    }
}
