//! Finite attended Create 1 Full-mode and hello transaction.

use conduit_create_oi::{
    decode_sensor_packet, encode_mode, encode_pause_stream, encode_query_sensor,
    encode_sensor_stream, encode_start, write_command, CreateOiModeRequest, CreateUartProvider,
    STREAM_HEADER,
};
use embassy_rp::watchdog::Watchdog;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_nb::serial::Read as _;
use portable_atomic::{AtomicBool, Ordering};

use super::create_control::{now_ms, watchdog_delay, Provider};
use crate::{create_link_gate, uart_diagnostic};

const ACQUISITION_ATTEMPTS: usize = 5;
const START_SETTLE_MS: u64 = 20;
const MODE_SETTLE_MS: u64 = 20;
const MODE_RESPONSE_MS: u64 = 1_000;
const REACQUIRE_COOLDOWN_MS: u64 = 50;
const READY_CUE_DEFINE: [u8; 11] = [140, 2, 4, 60, 32, 64, 32, 0, 12, 67, 40];
const READY_CUE_PLAY: [u8; 2] = [141, 2];
const READY_CUE_COMPLETION_MS: u64 = 1_913;
const MODE_STREAM_RESPONSE_MS: u64 = 2_000;

static READY_CUE_PLAYED: AtomicBool = AtomicBool::new(false);

pub fn ready_cue_command_sent() -> bool {
    READY_CUE_PLAYED.load(Ordering::Acquire)
}

fn discard_uart(provider: &mut Provider) {
    for _ in 0..128 {
        match provider.uart.read() {
            Ok(_) => {
                uart_diagnostic::record_rx(now_ms());
                uart_diagnostic::record_discard(1);
            }
            Err(nb::Error::WouldBlock) => break,
            Err(nb::Error::Other(error)) => uart_diagnostic::record_error(error),
        }
    }
}

async fn read_packet(
    provider: &mut Provider,
    watchdog: &mut Watchdog,
    packet_id: u8,
    deadline: Instant,
) -> Result<u8, ()> {
    while Instant::now() < deadline && create_link_gate::authorized() {
        match provider.uart.read() {
            Ok(byte) => {
                uart_diagnostic::record_rx(now_ms());
                watchdog.feed(Duration::from_millis(2_000));
                if decode_sensor_packet(packet_id, &[byte]).is_err() {
                    uart_diagnostic::record_frame(packet_id, &[byte], false);
                    // A TX-correlated edge can produce a bounded invalid byte
                    // through the auto-direction level shifter. Keep scanning
                    // until the deadline, but never promote that byte into a
                    // mode or song observation.
                    continue;
                }
                uart_diagnostic::record_frame(packet_id, &[byte], true);
                return Ok(byte);
            }
            Err(nb::Error::WouldBlock) => {
                Timer::after(Duration::from_millis(1)).await;
                watchdog.feed(Duration::from_millis(2_000));
            }
            Err(nb::Error::Other(error)) => {
                uart_diagnostic::record_error(error);
                Timer::after(Duration::from_millis(1)).await;
                watchdog.feed(Duration::from_millis(2_000));
            }
        }
    }
    uart_diagnostic::record_timeout();
    Err(())
}

async fn read_full_stream(
    provider: &mut Provider,
    watchdog: &mut Watchdog,
    deadline: Instant,
) -> Result<(), ()> {
    let mut frame = [0_u8; 5];
    let mut received = 0_usize;
    while Instant::now() < deadline && create_link_gate::authorized() {
        match provider.uart.read() {
            Ok(byte) => {
                uart_diagnostic::record_rx(now_ms());
                watchdog.feed(Duration::from_millis(2_000));
                let accepted = match received {
                    0 => byte == STREAM_HEADER,
                    1 => byte == 2,
                    2 => byte == 35,
                    _ => true,
                };
                if !accepted {
                    uart_diagnostic::record_discard(uart_diagnostic::discarded_on_mismatch(
                        received,
                        byte,
                        STREAM_HEADER,
                    ));
                    received = usize::from(byte == STREAM_HEADER);
                    if received == 1 {
                        frame[0] = byte;
                    }
                    continue;
                }
                frame[received] = byte;
                received += 1;
                if received == frame.len() {
                    let valid = frame
                        .iter()
                        .fold(0_u8, |sum, value| sum.wrapping_add(*value))
                        == 0
                        && frame[3] == 3;
                    uart_diagnostic::record_frame(35, &frame, valid);
                    if valid {
                        write_command(provider, &encode_pause_stream()).map_err(|_| ())?;
                        return Ok(());
                    }
                    received = 0;
                }
            }
            Err(nb::Error::WouldBlock) => {
                Timer::after(Duration::from_millis(1)).await;
                watchdog.feed(Duration::from_millis(2_000));
            }
            Err(nb::Error::Other(error)) => {
                uart_diagnostic::record_error(error);
                Timer::after(Duration::from_millis(1)).await;
                watchdog.feed(Duration::from_millis(2_000));
            }
        }
    }
    let _ = write_command(provider, &encode_pause_stream());
    uart_diagnostic::record_timeout();
    Err(())
}

pub async fn establish_full(provider: &mut Provider, watchdog: &mut Watchdog) -> Result<(), ()> {
    for _ in 0..ACQUISITION_ATTEMPTS {
        if !create_link_gate::authorized() {
            return Err(());
        }
        discard_uart(provider);
        if write_command(provider, &encode_start()).is_err() {
            break;
        }
        watchdog_delay(watchdog, START_SETTLE_MS).await;
        discard_uart(provider);
        if write_command(
            provider,
            &encode_mode(CreateOiModeRequest::Full).expect("Full has one exact command"),
        )
        .is_err()
        {
            break;
        }
        watchdog_delay(watchdog, MODE_SETTLE_MS).await;
        discard_uart(provider);
        if write_command(
            provider,
            &encode_query_sensor(35).expect("mode packet is allow-listed"),
        )
        .is_err()
        {
            break;
        }
        if read_packet(
            provider,
            watchdog,
            35,
            Instant::now() + Duration::from_millis(MODE_RESPONSE_MS),
        )
        .await
        .is_ok_and(|mode| mode == 3)
        {
            return Ok(());
        }
        watchdog_delay(watchdog, REACQUIRE_COOLDOWN_MS).await;
    }
    discard_uart(provider);
    if write_command(
        provider,
        &encode_sensor_stream(35).expect("mode stream packet is allow-listed"),
    )
    .is_ok()
        && read_full_stream(
            provider,
            watchdog,
            Instant::now() + Duration::from_millis(MODE_STREAM_RESPONSE_MS),
        )
        .await
        .is_ok()
    {
        return Ok(());
    }
    let _ = request_safe_unverified(provider);
    Err(())
}

pub fn request_safe_unverified(provider: &mut Provider) -> Result<(), ()> {
    write_command(provider, &encode_start()).map_err(|_| ())?;
    write_command(
        provider,
        &encode_mode(CreateOiModeRequest::Safe).expect("Safe has one exact command"),
    )
    .map_err(|_| ())
}

pub async fn play_ready_cue(provider: &mut Provider, watchdog: &mut Watchdog) -> Result<(), ()> {
    if !create_link_gate::authorized() {
        return Err(());
    }
    provider.write_all(&READY_CUE_DEFINE).map_err(|_| ())?;
    watchdog_delay(watchdog, 20).await;
    provider.write_all(&READY_CUE_PLAY).map_err(|_| ())?;
    READY_CUE_PLAYED.store(true, Ordering::Release);
    watchdog_delay(watchdog, READY_CUE_COMPLETION_MS).await;
    write_command(
        provider,
        &encode_query_sensor(37).expect("song-playing packet is allow-listed"),
    )
    .map_err(|_| ())?;
    match read_packet(
        provider,
        watchdog,
        37,
        Instant::now() + Duration::from_millis(MODE_RESPONSE_MS),
    )
    .await
    {
        Ok(0) => Ok(()),
        _ => Err(()),
    }
}

pub async fn restore_safe(provider: &mut Provider, watchdog: &mut Watchdog) -> Result<(), ()> {
    write_command(provider, &encode_start()).map_err(|_| ())?;
    watchdog_delay(watchdog, START_SETTLE_MS).await;
    discard_uart(provider);
    write_command(
        provider,
        &encode_mode(CreateOiModeRequest::Safe).expect("Safe has one exact command"),
    )
    .map_err(|_| ())?;
    watchdog_delay(watchdog, MODE_SETTLE_MS).await;
    discard_uart(provider);
    write_command(
        provider,
        &encode_query_sensor(35).expect("mode packet is allow-listed"),
    )
    .map_err(|_| ())?;
    match read_packet(
        provider,
        watchdog,
        35,
        Instant::now() + Duration::from_millis(MODE_RESPONSE_MS),
    )
    .await
    {
        Ok(2) => Ok(()),
        _ => Err(()),
    }
}
