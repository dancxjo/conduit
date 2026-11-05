//! Existing bounded Conduit stream framing over one physical CDC interface.
use conduit_wire::stream_framing::StreamFrameDecoder;
use embassy_rp::{peripherals::USB, usb};
use embassy_time::{with_timeout, Duration};
use embassy_usb::class::cdc_acm::CdcAcmClass;
pub const LIMIT: usize = 4096;
pub type Cdc = CdcAcmClass<'static, usb::Driver<'static, USB>>;
pub struct Link {
    class: Cdc,
    decoder: StreamFrameDecoder<LIMIT>,
}
impl Link {
    pub fn new(class: Cdc) -> Self {
        Self {
            class,
            decoder: StreamFrameDecoder::new(LIMIT).unwrap(),
        }
    }
    pub async fn connected(&mut self) {
        self.class.wait_connection().await;
    }
    pub fn reset(&mut self) {
        self.decoder = StreamFrameDecoder::new(LIMIT).unwrap();
    }
    pub async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let mut packet = [0; 64];
        loop {
            if let Some(frame) = self.decoder.next_frame().map_err(|_| ())? {
                let length = frame.len();
                let target = output.get_mut(..length).ok_or(())?;
                target.copy_from_slice(frame);
                if crate::bootsel::handle(self, &output[..length]).await? {
                    continue;
                }
                return Ok(&output[..length]);
            }
            let n = with_timeout(Duration::from_secs(30), self.class.read_packet(&mut packet))
                .await
                .map_err(|_| ())?
                .map_err(|_| ())?;
            if n != 0 {
                self.decoder.accept_bytes(&packet[..n]).map_err(|_| ())?;
            }
        }
    }
    /// Feed the already selected document receiver directly from the existing
    /// frame decoder. No second frame/document buffer or transport is created.
    /// Other control frames refuse this transaction; its caller owns one deadline.
    pub async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        let mut packet = [0u8; 64];
        loop {
            if let Some(frame) = self.decoder.next_frame().map_err(|_| ())? {
                if document.push(frame).map_err(|_| ())? {
                    return Ok(());
                }
                continue;
            }
            let n = with_timeout(Duration::from_secs(30), self.class.read_packet(&mut packet))
                .await
                .map_err(|_| ())?
                .map_err(|_| ())?;
            if n != 0 {
                self.decoder.accept_bytes(&packet[..n]).map_err(|_| ())?;
            }
        }
    }
    pub async fn send(&mut self, payload: &[u8]) -> Result<(), ()> {
        if payload.is_empty() || payload.len() > LIMIT {
            return Err(());
        }
        let header = (payload.len() as u16).to_be_bytes();
        with_timeout(Duration::from_secs(2), self.class.write_packet(&header))
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
        for packet in payload.chunks(64) {
            with_timeout(Duration::from_secs(2), self.class.write_packet(packet))
                .await
                .map_err(|_| ())?
                .map_err(|_| ())?;
        }
        if payload.len().is_multiple_of(64) {
            self.class.write_packet(&[]).await.map_err(|_| ())?;
        }
        Ok(())
    }
}
