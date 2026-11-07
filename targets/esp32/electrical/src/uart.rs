//! Exclusive UART0 carrier; no logger or unsolicited text shares these bytes.
use conduit_mcu_host_runtime::transport::Carrier;
use embassy_time::{with_timeout, Duration};
use esp_hal::{
    peripherals::{GPIO1, GPIO3, UART0},
    uart::{Config, ConfigError, RxError, TxError, Uart},
    Async,
};

#[derive(Debug)]
pub enum CarrierError {
    InvalidDeadline,
    Timeout,
    Receive(RxError),
    Transmit(TxError),
    NoProgress,
}

/// Owns Blue's physical USB-to-UART channel. The composition must leave the
/// ROM/programming channel unshared with esp-println or other native clients.
/// UART has no USB connection event: reachability is established by protocol
/// exchange, never by `connected`, and admission remains the Body's decision.
pub struct Uart0Carrier<'d> {
    uart: Uart<'d, Async>,
}
impl<'d> Uart0Carrier<'d> {
    pub fn new(
        peripheral: UART0<'d>,
        tx: GPIO1<'d>,
        rx: GPIO3<'d>,
        config: Config,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            uart: Uart::new(peripheral, config)?
                .with_tx(tx)
                .with_rx(rx)
                .into_async(),
        })
    }
}
impl Carrier for Uart0Carrier<'_> {
    type Error = CarrierError;
    async fn connected(&mut self) {}
    async fn read(&mut self, output: &mut [u8], timeout_millis: u32) -> Result<usize, Self::Error> {
        if timeout_millis == 0 || output.is_empty() {
            return Err(CarrierError::InvalidDeadline);
        }
        with_timeout(
            Duration::from_millis(u64::from(timeout_millis)),
            self.uart.read_async(output),
        )
        .await
        .map_err(|_| CarrierError::Timeout)?
        .map_err(CarrierError::Receive)
    }
    async fn write(&mut self, bytes: &[u8], timeout_millis: u32) -> Result<(), Self::Error> {
        if timeout_millis == 0 {
            return Err(CarrierError::InvalidDeadline);
        }
        // One deadline covers every partial write and flush. The UART driver
        // futures are cancellation-safe; already transmitted bytes cannot be
        // retracted, and protocol framing handles interrupted transactions.
        with_timeout(Duration::from_millis(u64::from(timeout_millis)), async {
            let mut remaining = bytes;
            while !remaining.is_empty() {
                let written = self
                    .uart
                    .write_async(remaining)
                    .await
                    .map_err(CarrierError::Transmit)?;
                if written == 0 {
                    return Err(CarrierError::NoProgress);
                }
                remaining = &remaining[written..];
            }
            self.uart
                .flush_async()
                .await
                .map_err(CarrierError::Transmit)
        })
        .await
        .map_err(|_| CarrierError::Timeout)?
    }
}
