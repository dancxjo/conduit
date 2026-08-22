//! Persistent, bounded Create 1 OI ownership for the Pete carrier.

use conduit_create_oi::{
    decode_sensor_packet, encode_mode, encode_pause_stream, encode_sensor_stream,
    encode_start, write_command, CreateOiModeRequest, CreateUartProvider, UartProfile,
    CREATE_OI_BAUD, STREAM_HEADER,
};
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::{PIN_0, PIN_1, UART0, WATCHDOG};
use embassy_rp::uart::{Blocking, Config, DataBits, Parity, StopBits, Uart};
use embassy_rp::watchdog::Watchdog;
use embassy_rp::Peri;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_nb::serial::Read as _;
use portable_atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

use crate::uart_diagnostic;

const START_SETTLE_MS: u64 = 250;
const MODE_SETTLE_MS: u64 = 100;
const LINK_FRESHNESS_MS: u64 = 1_000;
const REACQUIRE_COOLDOWN_MS: u64 = 500;
const FULL_REFRESH_MS: u64 = 1_000;
const WATCHDOG_TIMEOUT_MS: u64 = 2_000;
const WATCHDOG_FEED_MS: u64 = 250;
const READY_CUE_DEFINE: [u8; 9] = [140, 2, 3, 65, 6, 67, 6, 71, 10];
const READY_CUE_PLAY: [u8; 2] = [141, 2];
const MODE_FRAME_BYTES: usize = 5;
const MAX_STREAM_FRAME_BYTES: usize = uart_diagnostic::MAX_FRAME_BYTES;
const SAFETY_POLL_MS: u64 = 20;
const CHARGING_POLL_MS: u64 = 250;
const MODE_POLL_MS: u64 = 250;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum State {
    Initializing = 0,
    Acquiring = 1,
    Full = 2,
    Passive = 3,
    Safe = 4,
    LinkLost = 5,
    UartFault = 6,
}

impl State {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Acquiring,
            2 => Self::Full,
            3 => Self::Passive,
            4 => Self::Safe,
            5 => Self::LinkLost,
            6 => Self::UartFault,
            _ => Self::Initializing,
        }
    }
}

pub struct Snapshot {
    pub state: State,
    pub packets: u32,
    pub last_packet_ms: u32,
}

static STATE: AtomicU8 = AtomicU8::new(State::Initializing as u8);
static OI_MODE: AtomicU8 = AtomicU8::new(0);
static PACKETS: AtomicU32 = AtomicU32::new(0);
static LAST_PACKET_MS: AtomicU32 = AtomicU32::new(0);
static CHARGING_SOURCES: AtomicU8 = AtomicU8::new(0);
static READY_CUE_PLAYED: AtomicBool = AtomicBool::new(false);

pub fn snapshot() -> Snapshot {
    Snapshot {
        state: State::from_raw(STATE.load(Ordering::Acquire)),
        packets: PACKETS.load(Ordering::Acquire),
        last_packet_ms: LAST_PACKET_MS.load(Ordering::Acquire),
    }
}

pub fn is_fresh(snapshot: &Snapshot, now_ms: u32) -> bool {
    snapshot.packets > 0
        && now_ms.wrapping_sub(snapshot.last_packet_ms) <= LINK_FRESHNESS_MS as u32
}

pub fn ready_cue_command_sent() -> bool {
    READY_CUE_PLAYED.load(Ordering::Acquire)
}

struct Provider {
    uart: Uart<'static, Blocking>,
}

impl CreateUartProvider for Provider {
    type Error = embassy_rp::uart::Error;

    fn is_available(&self) -> bool {
        true
    }

    fn profile(&self) -> UartProfile {
        UartProfile::CREATE_OI
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.uart.blocking_write(bytes)?;
        self.uart.blocking_flush()?;
        uart_diagnostic::record_tx(bytes.len());
        Ok(())
    }

    fn read_byte(&mut self, _deadline_tick: u64) -> Result<Option<u8>, Self::Error> {
        match self.uart.read() {
            Ok(byte) => Ok(Some(byte)),
            Err(nb::Error::WouldBlock) => Ok(None),
            Err(nb::Error::Other(error)) => Err(error),
        }
    }
}

fn now_ms() -> u32 {
    Instant::now().as_millis() as u32
}

async fn watchdog_delay(watchdog: &mut Watchdog, millis: u64) {
    let mut remaining = millis;
    while remaining > 0 {
        let step = remaining.min(WATCHDOG_FEED_MS);
        Timer::after(Duration::from_millis(step)).await;
        watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
        remaining -= step;
    }
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

fn begin_supervision(provider: &mut Provider) -> Result<(), ()> {
    write_command(provider, &encode_start()).map_err(|_| ())?;
    Ok(())
}

async fn play_ready_cue(provider: &mut Provider, watchdog: &mut Watchdog) -> Result<(), ()> {
    if READY_CUE_PLAYED.load(Ordering::Acquire) {
        return Ok(());
    }
    provider.write_all(&READY_CUE_DEFINE).map_err(|_| ())?;
    watchdog_delay(watchdog, 20).await;
    provider.write_all(&READY_CUE_PLAY).map_err(|_| ())?;
    READY_CUE_PLAYED.store(true, Ordering::Release);
    Ok(())
}

async fn acquire(provider: &mut Provider, watchdog: &mut Watchdog) -> Result<(), ()> {
    STATE.store(State::Acquiring as u8, Ordering::Release);
    OI_MODE.store(0, Ordering::Release);
    discard_uart(provider);
    begin_supervision(provider)?;
    watchdog_delay(watchdog, START_SETTLE_MS).await;
    write_command(
        provider,
        &encode_mode(CreateOiModeRequest::Full).expect("Full has one exact command"),
    )
    .map_err(|_| ())?;
    watchdog_delay(watchdog, MODE_SETTLE_MS).await;
    let stream = encode_sensor_stream(35).map_err(|_| ())?;
    write_command(provider, &stream).map_err(|_| ())
}

async fn confirm_full_mode(
    provider: &mut Provider,
    watchdog: &mut Watchdog,
    deadline: Instant,
) -> Result<(), ()> {
    let mut frame = [0_u8; MODE_FRAME_BYTES];
    let mut received = 0_usize;
    while Instant::now() < deadline {
        match provider.uart.read() {
            Ok(byte) => {
                uart_diagnostic::record_rx(now_ms());
                watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
                let accepted = match received {
                    0 => byte == STREAM_HEADER,
                    1 => usize::from(byte) + 3 == MODE_FRAME_BYTES,
                    2 => byte == 35,
                    _ => true,
                };
                if !accepted {
                    uart_diagnostic::record_discard(
                        uart_diagnostic::discarded_on_mismatch(received, byte, STREAM_HEADER),
                    );
                    received = usize::from(byte == STREAM_HEADER);
                    if received == 1 {
                        frame[0] = byte;
                    }
                    continue;
                }
                frame[received] = byte;
                received += 1;
                if received == MODE_FRAME_BYTES {
                    if frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)) == 0
                        && decode_sensor_packet(35, &frame[3..4])
                            .map(|packet| packet.bytes()[0] == 3)
                            .unwrap_or(false)
                    {
                        uart_diagnostic::record_frame(35, &frame, true);
                        // Each framed Stream probe is one finite transaction;
                        // pause it before another packet request can begin.
                        write_command(provider, &encode_pause_stream()).map_err(|_| ())?;
                        OI_MODE.store(3, Ordering::Release);
                        return Ok(());
                    }
                    uart_diagnostic::record_frame(35, &frame, false);
                    received = 0;
                }
            }
            Err(nb::Error::WouldBlock) => {
                Timer::after(Duration::from_millis(1)).await;
                watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
            }
            Err(nb::Error::Other(error)) => {
                uart_diagnostic::record_error(error);
                uart_diagnostic::record_discard(received);
                received = 0;
                Timer::after(Duration::from_millis(1)).await;
                watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
            }
        }
    }
    uart_diagnostic::record_timeout();
    Err(())
}

async fn transact_sensor_packet(
    provider: &mut Provider,
    watchdog: &mut Watchdog,
    motion: &mut crate::create_motion::Runtime,
    packet_id: u8,
    deadline: Instant,
) -> Result<(), ()> {
    let data_len = match packet_id {
        0 => 26,
        34 | 35 => 1,
        _ => return Err(()),
    };
    let frame_len = data_len + 4;
    let stream = encode_sensor_stream(packet_id).map_err(|_| ())?;
    write_command(provider, &stream).map_err(|_| ())?;
    let mut frame = [0_u8; MAX_STREAM_FRAME_BYTES];
    let mut received = 0_usize;
    while Instant::now() < deadline {
        match provider.uart.read() {
            Ok(byte) => {
                uart_diagnostic::record_rx(now_ms());
                watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
                let accepted = match received {
                    0 => byte == STREAM_HEADER,
                    1 => usize::from(byte) == data_len + 1,
                    2 => byte == packet_id,
                    _ => true,
                };
                if !accepted {
                    uart_diagnostic::record_discard(
                        uart_diagnostic::discarded_on_mismatch(received, byte, STREAM_HEADER),
                    );
                    received = usize::from(byte == STREAM_HEADER);
                    if received == 1 {
                        frame[0] = byte;
                    }
                    continue;
                }
                frame[received] = byte;
                received += 1;
                if received == frame_len {
                    if frame[..frame_len]
                        .iter()
                        .fold(0_u8, |sum, value| sum.wrapping_add(*value))
                        != 0
                    {
                        uart_diagnostic::record_frame(packet_id, &frame[..frame_len], false);
                        received = 0;
                        continue;
                    }
                    uart_diagnostic::record_frame(packet_id, &frame[..frame_len], true);
                    write_command(provider, &encode_pause_stream()).map_err(|_| ())?;
                    let data = &frame[3..3 + data_len];
                    let decoded = decode_sensor_packet(packet_id, data).map_err(|_| ())?;
                    match packet_id {
                        0 => {
                            let bytes = decoded.bytes();
                            motion.observe(
                                provider,
                                now_ms(),
                                bytes,
                                CHARGING_SOURCES.load(Ordering::Acquire),
                            );
                        }
                        34 => CHARGING_SOURCES.store(data[0], Ordering::Release),
                        35 => {
                            let mode = data[0];
                            OI_MODE.store(mode, Ordering::Release);
                            STATE.store(
                                match mode {
                                    3 => State::Full,
                                    2 => State::Safe,
                                    _ => State::Passive,
                                } as u8,
                                Ordering::Release,
                            );
                        }
                        _ => return Err(()),
                    }
                    PACKETS.fetch_add(1, Ordering::Relaxed);
                    LAST_PACKET_MS.store(now_ms(), Ordering::Release);
                    return Ok(());
                }
            }
            Err(nb::Error::WouldBlock) => {
                motion.tick(provider, now_ms());
                Timer::after(Duration::from_millis(1)).await;
                watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
            }
            Err(nb::Error::Other(error)) => {
                uart_diagnostic::record_error(error);
                uart_diagnostic::record_discard(received);
                received = 0;
                Timer::after(Duration::from_millis(1)).await;
                watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
            }
        }
    }
    uart_diagnostic::record_timeout();
    Err(())
}

#[embassy_executor::task]
pub async fn task(
    uart0: Peri<'static, UART0>,
    tx: Peri<'static, PIN_0>,
    rx: Peri<'static, PIN_1>,
    mut translator_oe: Output<'static>,
    watchdog: Peri<'static, WATCHDOG>,
) {
    let mut config = Config::default();
    config.baudrate = CREATE_OI_BAUD;
    config.data_bits = DataBits::DataBits8;
    config.stop_bits = StopBits::STOP1;
    config.parity = Parity::ParityNone;
    let mut provider = Provider {
        uart: Uart::new_blocking(uart0, tx, rx, config),
    };
    // Match Pete's RX pull-up so an unpowered Create cannot look like framing noise.
    rp_pac::PADS_BANK0.gpio(1).modify(|value| {
        value.set_pue(true);
        value.set_pde(false);
    });
    let mut watchdog = Watchdog::new(watchdog);
    watchdog.pause_on_debug(true);
    watchdog.start(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
    uart_diagnostic::start(now_ms());
    translator_oe.set_high();
    watchdog_delay(&mut watchdog, 10).await;
    let mut motion = crate::create_motion::Runtime::new();

    loop {
        if acquire(&mut provider, &mut watchdog).await.is_err() {
            STATE.store(State::UartFault as u8, Ordering::Release);
            watchdog_delay(&mut watchdog, REACQUIRE_COOLDOWN_MS).await;
            continue;
        }
        if confirm_full_mode(
            &mut provider,
            &mut watchdog,
            Instant::now() + Duration::from_millis(LINK_FRESHNESS_MS),
        )
        .await
        .is_err()
        {
            STATE.store(State::LinkLost as u8, Ordering::Release);
            OI_MODE.store(0, Ordering::Release);
            watchdog_delay(&mut watchdog, REACQUIRE_COOLDOWN_MS).await;
            continue;
        }
        if play_ready_cue(&mut provider, &mut watchdog).await.is_err() {
            STATE.store(State::UartFault as u8, Ordering::Release);
            watchdog_delay(&mut watchdog, REACQUIRE_COOLDOWN_MS).await;
            continue;
        }
        let mut next_full_refresh = Instant::now() + Duration::from_millis(FULL_REFRESH_MS);
        let mut next_safety_poll = Instant::now();
        let mut next_charging_poll = Instant::now();
        let mut next_mode_poll = Instant::now();
        loop {
            let now = Instant::now();
            let packet_id = if now >= next_safety_poll {
                next_safety_poll = now + Duration::from_millis(SAFETY_POLL_MS);
                0
            } else if now >= next_charging_poll {
                next_charging_poll = now + Duration::from_millis(CHARGING_POLL_MS);
                34
            } else if now >= next_mode_poll {
                next_mode_poll = now + Duration::from_millis(MODE_POLL_MS);
                35
            } else {
                let next_due = next_safety_poll.min(next_charging_poll).min(next_mode_poll);
                motion.tick(&mut provider, now_ms());
                Timer::at(next_due).await;
                continue;
            };
            if transact_sensor_packet(
                &mut provider,
                &mut watchdog,
                &mut motion,
                packet_id,
                Instant::now() + Duration::from_millis(LINK_FRESHNESS_MS),
            )
            .await
            .is_err()
            {
                motion.link_lost(&mut provider);
                STATE.store(State::LinkLost as u8, Ordering::Release);
                OI_MODE.store(0, Ordering::Release);
                break;
            }
            if OI_MODE.load(Ordering::Acquire) != 3 {
                motion.link_lost(&mut provider);
                break;
            }
            if Instant::now() >= next_full_refresh && !motion.is_active() {
                if write_command(
                    &mut provider,
                    &encode_mode(CreateOiModeRequest::Full)
                        .expect("Full has one exact command"),
                )
                .is_err()
                {
                    STATE.store(State::UartFault as u8, Ordering::Release);
                    break;
                }
                next_full_refresh = Instant::now() + Duration::from_millis(FULL_REFRESH_MS);
            }
        }
        watchdog_delay(&mut watchdog, REACQUIRE_COOLDOWN_MS).await;
    }
}
