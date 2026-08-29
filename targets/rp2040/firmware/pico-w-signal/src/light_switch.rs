//! Absolute Boolean CYW43 manifestation for the physical light-switch demo.
//!
//! This Base adapter accepts only `0\n` or `1\n`. It does not retain or
//! derive toggle truth; every manifestation is acknowledged independently.

use cyw43::Control;
use crate::{receipts::UsbCdc, usb::PicoUsbCdcLine};

const READY: &str = "CONDUIT_LIGHT_SWITCH_READY host=pico-w led=cyw43-gpio0";
const LED_OFF: &str = "CONDUIT_LIGHT_SWITCH_LED host=pico-w level=false";
const LED_ON: &str = "CONDUIT_LIGHT_SWITCH_LED host=pico-w level=true";

pub async fn run(
    line: PicoUsbCdcLine,
    signs: &mut UsbCdc,
    control: &mut Control<'_>,
) -> ! {
    let (mut sender, mut receiver) = line.class.split();
    receiver.wait_connection().await;
    if signs.write_marker(READY).await.is_err() {
        core::future::pending::<()>().await;
    }
    control.gpio_set(0, false).await;
    if signs.write_marker(LED_OFF).await.is_err() {
        core::future::pending::<()>().await;
    }

    let mut pending = None;
    let mut packet = [0_u8; 64];
    loop {
        let length = match receiver.read_packet(&mut packet).await {
            Ok(length) => length,
            Err(_) => core::future::pending::<usize>().await,
        };
        for byte in &packet[..length] {
            match (*byte, pending) {
                (b'0' | b'1', None) => pending = Some(*byte == b'1'),
                (b'\n', Some(level)) => {
                    control.gpio_set(0, level).await;
                    if signs
                        .write_marker(if level { LED_ON } else { LED_OFF })
                        .await
                        .is_err()
                    {
                        core::future::pending::<()>().await;
                    }
                    // A one-byte ACK makes the command path independently
                    // observable even when the sign consumer is absent.
                    if sender.write_packet(if level { b"1\n" } else { b"0\n" }).await.is_err() {
                        core::future::pending::<()>().await;
                    }
                    pending = None;
                }
                _ => pending = None,
            }
        }
    }
}
