//! Finite attended Create 1 Full-mode and hello transaction.

use conduit_create_oi::{
    decode_sensor_packet, encode_mode, encode_query_sensor, encode_start, write_command,
    CreateOiModeRequest, CreateUartProvider,
};
use embassy_rp::watchdog::Watchdog;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_nb::serial::Read as _;
use portable_atomic::{AtomicBool, Ordering};

use super::create_control::{now_ms, watchdog_delay, Provider};
use crate::{create_link_gate, uart_diagnostic};

const ACQUISITION_ATTEMPTS: usize = 10;
const START_SETTLE_MS: u64 = 20;
const MODE_SETTLE_MS: u64 = 20;
const MODE_RESPONSE_MS: u64 = 500;
const REACQUIRE_COOLDOWN_MS: u64 = 50;
const READY_CUE_DEFINE: [u8; 11] = [140, 2, 4, 60, 32, 64, 32, 0, 12, 67, 40];
const READY_CUE_PLAY: [u8; 2] = [141, 2];
const READY_CUE_COMPLETION_MS: u64 = 1_913;

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
                    return Err(());
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

pub async fn establish_full(
    provider: &mut Provider,
    watchdog: &mut Watchdog,
) -> Result<(), ()> {
    for _ in 0..ACQUISITION_ATTEMPTS {
        if !create_link_gate::authorized() {
            return Err(());
        }
        discard_uart(provider);
        write_command(provider, &encode_start()).map_err(|_| ())?;
        watchdog_delay(watchdog, START_SETTLE_MS).await;
        write_command(
            provider,
            &encode_mode(CreateOiModeRequest::Full).expect("Full has one exact command"),
        )
        .map_err(|_| ())?;
        watchdog_delay(watchdog, MODE_SETTLE_MS).await;
        write_command(
            provider,
            &encode_query_sensor(35).expect("mode packet is allow-listed"),
        )
        .map_err(|_| ())?;
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
    Err(())
}

pub async fn play_ready_cue(
    provider: &mut Provider,
    watchdog: &mut Watchdog,
) -> Result<(), ()> {
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

pub async fn restore_safe(
    provider: &mut Provider,
    watchdog: &mut Watchdog,
) -> Result<(), ()> {
    write_command(provider, &encode_start()).map_err(|_| ())?;
    write_command(
        provider,
        &encode_mode(CreateOiModeRequest::Safe).expect("Safe has one exact command"),
    )
    .map_err(|_| ())?;
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
