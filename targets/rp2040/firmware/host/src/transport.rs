//! Shared bounded Conduit framing over the RP2040-owned CDC interface.
pub use conduit_mcu_host_runtime::transport::LIMIT;
use conduit_mcu_host_runtime::transport::{Carrier, Link as FramedLink};
use embassy_rp::{peripherals::USB, usb};
use embassy_time::{with_timeout, Duration};
use embassy_usb::class::cdc_acm::CdcAcmClass;
pub type Cdc = CdcAcmClass<'static, usb::Driver<'static, USB>>;
struct UsbCarrier(Cdc);
impl Carrier for UsbCarrier {
    type Error = ();
    async fn connected(&mut self) {
        self.0.wait_connection().await;
    }
    async fn read(&mut self, output: &mut [u8], timeout_millis: u32) -> Result<usize, ()> {
        with_timeout(
            Duration::from_millis(u64::from(timeout_millis)),
            self.0.read_packet(output),
        )
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
    }
    async fn write(&mut self, bytes: &[u8], timeout_millis: u32) -> Result<(), ()> {
        with_timeout(
            Duration::from_millis(u64::from(timeout_millis)),
            self.0.write_packet(bytes),
        )
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
    }
}
pub struct Link(FramedLink<UsbCarrier>);
impl Link {
    pub fn new(class: Cdc) -> Self {
        Self(FramedLink::new(UsbCarrier(class)))
    }
    pub async fn connected(&mut self) {
        self.0.connected().await;
    }
    pub fn reset(&mut self) {
        self.0.reset();
    }
    pub async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        loop {
            let length = self.0.receive(output).await.map_err(|_| ())?.len();
            if !crate::bootsel::handle(self, &output[..length]).await? {
                return Ok(&output[..length]);
            }
        }
    }
    pub async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        self.0.receive_document(document).await.map_err(|_| ())
    }
    pub async fn send(&mut self, payload: &[u8]) -> Result<(), ()> {
        self.0.send(payload).await.map_err(|_| ())
    }
}

impl conduit_mcu_host_runtime::transport::MessageTransport for Link {
    type Error = ();
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        Link::receive(self, output).await
    }
    async fn send(&mut self, payload: &[u8]) -> Result<(), ()> {
        Link::send(self, payload).await
    }
}

impl conduit_mcu_host_runtime::transport::DocumentTransport for Link {
    async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), Self::Error> {
        Link::receive_document(self, document).await
    }
}
