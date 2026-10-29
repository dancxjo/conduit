//! Exclusive bounded carrier for the unchanged authenticated Root exchanges.
//! TCP reachability conveys no Body membership or operation authority.
#[cfg(not(feature = "owner-wifi"))]
pub type Carrier<'a> = esp_hal::uart::Uart<'a, esp_hal::Async>;
#[cfg(feature = "owner-wifi")]
pub struct Carrier<'a>(embassy_net::tcp::TcpSocket<'a>);
#[cfg(feature = "owner-wifi")]
impl Carrier<'_> {
    pub async fn read_async(&mut self, bytes: &mut [u8]) -> Result<usize, ()> {
        self.0.read(bytes).await.map_err(|_| ())
    }
    pub async fn write_async(&mut self, bytes: &[u8]) -> Result<usize, ()> {
        self.0.write(bytes).await.map_err(|_| ())
    }
}
#[cfg(feature = "owner-wifi")]
impl Carrier<'_> {
    pub fn abort(&mut self) {
        self.0.abort();
    }
}
#[cfg(feature = "owner-wifi")]
pub struct Listener {
    ap: Carrier<'static>,
    station: Carrier<'static>,
}
#[cfg(feature = "owner-wifi")]
impl Listener {
    pub fn new(stacks: (embassy_net::Stack<'static>, embassy_net::Stack<'static>)) -> Self {
        // One socket per interface; fixed 2 KiB payload backing, reused after
        // disconnect. Only one management carrier is served at a time.
        static BUFFERS: static_cell::StaticCell<[[u8; 512]; 4]> = static_cell::StaticCell::new();
        let [ap_rx, ap_tx, station_rx, station_tx] = BUFFERS.init([[0; 512]; 4]);
        Self {
            ap: Carrier(embassy_net::tcp::TcpSocket::new(stacks.0, ap_rx, ap_tx)),
            station: Carrier(embassy_net::tcp::TcpSocket::new(
                stacks.1, station_rx, station_tx,
            )),
        }
    }
    pub async fn accept(
        &mut self,
        root: &mut crate::native_root::NativeRoot,
    ) -> &mut Carrier<'static> {
        use embassy_futures::select::{select, Either};
        loop {
            root.advance_setup().await;
            let accepted = embassy_time::with_timeout(
                embassy_time::Duration::from_millis(100),
                select(self.ap.0.accept(4242), self.station.0.accept(4242)),
            )
            .await;
            match accepted {
                Ok(Either::First(Ok(()))) => return &mut self.ap,
                Ok(Either::Second(Ok(()))) => return &mut self.station,
                _ => (),
            }
        }
    }
}
