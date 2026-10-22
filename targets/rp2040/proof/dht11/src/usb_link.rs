//! Diagnostic CDC adapter for the existing firmware lifecycle control module.
use conduit_wire::stream_framing::{encode_stream_frame, StreamFrameDecoder};
use embassy_rp::{peripherals::USB, usb::Driver};
use embassy_time::{with_timeout, Duration};
use embassy_usb::class::cdc_acm::CdcAcmClass;

#[derive(Debug)]
pub enum UsbLinkError {
    BufferOverflow,
    Disconnected,
    Framing,
    Timeout,
}
pub struct UsbLinkSession {
    pub class: CdcAcmClass<'static, Driver<'static, USB>>,
    decoder: StreamFrameDecoder<1026>,
}
impl UsbLinkSession {
    pub async fn write_text(&mut self, line: &str) -> Result<(), UsbLinkError> {
        if !self.class.dtr() {
            return Err(UsbLinkError::Disconnected);
        }
        for chunk in line.as_bytes().chunks(63) {
            with_timeout(Duration::from_secs(1), self.class.write_packet(chunk))
                .await
                .map_err(|_| UsbLinkError::Timeout)?
                .map_err(|_| UsbLinkError::Disconnected)?;
        }
        Ok(())
    }
    pub fn new(class: CdcAcmClass<'static, Driver<'static, USB>>) -> Self {
        Self {
            class,
            decoder: StreamFrameDecoder::new(1024).unwrap(),
        }
    }
    pub fn reset_stream_decoder(&mut self) {
        self.decoder = StreamFrameDecoder::new(1024).unwrap();
    }
    pub async fn wait_connection(&mut self) {
        self.class.wait_connection().await;
    }
    pub async fn receive_control_stream_frame<'b>(
        &mut self,
        output: &'b mut [u8],
    ) -> Result<&'b [u8], UsbLinkError> {
        loop {
            match self.decoder.next_frame() {
                Ok(Some(frame)) => {
                    if frame.len() > output.len() {
                        return Err(UsbLinkError::BufferOverflow);
                    }
                    output[..frame.len()].copy_from_slice(frame);
                    return Ok(&output[..frame.len()]);
                }
                Ok(None) => {}
                Err(_) => self.reset_stream_decoder(),
            }
            let mut packet = [0; 64];
            match with_timeout(
                Duration::from_millis(500),
                self.class.read_packet(&mut packet),
            )
            .await
            {
                Ok(Ok(length)) => {
                    if self.decoder.accept_bytes(&packet[..length]).is_err() {
                        self.reset_stream_decoder();
                    }
                }
                Ok(Err(_)) => return Err(UsbLinkError::Disconnected),
                Err(_) => self.reset_stream_decoder(),
            }
        }
    }
    pub async fn send_raw_stream_frame(&mut self, payload: &[u8]) -> Result<(), UsbLinkError> {
        let mut framed = [0; 1026];
        let length =
            encode_stream_frame(payload, 1024, &mut framed).map_err(|_| UsbLinkError::Framing)?;
        for chunk in framed[..length].chunks(64) {
            self.class
                .write_packet(chunk)
                .await
                .map_err(|_| UsbLinkError::Disconnected)?;
        }
        if length % 64 == 0 {
            self.class
                .write_packet(&[])
                .await
                .map_err(|_| UsbLinkError::Disconnected)?;
        }
        Ok(())
    }
}
