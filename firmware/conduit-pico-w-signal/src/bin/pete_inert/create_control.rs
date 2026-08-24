//! Persistent, bounded Create 1 OI ownership for the Pete carrier.

use conduit_create_oi::{
    decode_sensor_packet, encode_mode, encode_pause_stream, encode_sensor_stream, write_command,
    CreateOiModeRequest, CreateUartProvider, UartProfile, CREATE_OI_BAUD, STREAM_HEADER,
};
use embassy_rp::gpio::Output;
use embassy_rp::peripherals::{PIN_0, PIN_1, UART0, WATCHDOG};
use embassy_rp::uart::{Blocking, Config, DataBits, Parity, StopBits, Uart};
use embassy_rp::watchdog::Watchdog;
use embassy_rp::Peri;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_nb::serial::Read as _;
use portable_atomic::{AtomicU32, AtomicU8, Ordering};

use crate::{create_acquisition, create_link_gate, create_play, uart_diagnostic};

const LINK_FRESHNESS_MS: u64 = 1_000;
const REACQUIRE_COOLDOWN_MS: u64 = 50;
const FULL_REFRESH_MS: u64 = 1_000;
const WATCHDOG_TIMEOUT_MS: u64 = 2_000;
const WATCHDOG_FEED_MS: u64 = 250;
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
    pub translator_enabled: bool,
}

static STATE: AtomicU8 = AtomicU8::new(State::Initializing as u8);
static OI_MODE: AtomicU8 = AtomicU8::new(0);
static PACKETS: AtomicU32 = AtomicU32::new(0);
static LAST_PACKET_MS: AtomicU32 = AtomicU32::new(0);
static CHARGING_SOURCES: AtomicU8 = AtomicU8::new(0);

pub fn snapshot() -> Snapshot {
    Snapshot {
        state: State::from_raw(STATE.load(Ordering::Acquire)),
        packets: PACKETS.load(Ordering::Acquire),
        last_packet_ms: LAST_PACKET_MS.load(Ordering::Acquire),
        translator_enabled: create_link_gate::translator_enabled(),
    }
}

pub fn is_fresh(snapshot: &Snapshot, now_ms: u32) -> bool {
    snapshot.packets > 0
        && now_ms.wrapping_sub(snapshot.last_packet_ms) <= LINK_FRESHNESS_MS as u32
}

pub fn ready_cue_command_sent() -> bool {
    create_acquisition::ready_cue_command_sent()
}

pub(super) struct Provider {
    pub(super) uart: Uart<'static, Blocking>,
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

pub(super) fn now_ms() -> u32 {
    Instant::now().as_millis() as u32
}

pub(super) async fn watchdog_delay(watchdog: &mut Watchdog, millis: u64) {
    let mut remaining = millis;
    while remaining > 0 {
        let step = remaining.min(WATCHDOG_FEED_MS);
        Timer::after(Duration::from_millis(step)).await;
        watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
        remaining -= step;
    }
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
    while Instant::now() < deadline && create_link_gate::authorized() {
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
    mut power_toggle: Output<'static>,
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
    create_link_gate::set_translator(&mut translator_oe, false);
    let mut motion = crate::create_motion::Runtime::new();

    loop {
        while !create_link_gate::authorized() {
            create_link_gate::set_translator(&mut translator_oe, false);
            STATE.store(State::Initializing as u8, Ordering::Release);
            OI_MODE.store(0, Ordering::Release);
            if crate::create_power::claim_pending() {
                crate::create_power::execute(
                    &mut power_toggle,
                    &mut translator_oe,
                    &mut watchdog,
                )
                .await;
                continue;
            }
            watchdog_delay(&mut watchdog, 20).await;
        }
        create_link_gate::set_translator(&mut translator_oe, true);
        watchdog_delay(&mut watchdog, 10).await;
        STATE.store(State::Acquiring as u8, Ordering::Release);
        OI_MODE.store(0, Ordering::Release);
        if create_acquisition::establish_full(&mut provider, &mut watchdog)
            .await
            .is_err()
        {
            let kind = create_play::request_kind();
            if kind != create_play::RequestKind::None && create_play::claim_pending(kind) {
                create_play::set_result(7);
                create_play::set_state(create_play::RequestState::Refused);
            }
            STATE.store(State::LinkLost as u8, Ordering::Release);
            create_link_gate::set_translator(&mut translator_oe, false);
            continue;
        }
        OI_MODE.store(3, Ordering::Release);
        STATE.store(State::Full as u8, Ordering::Release);
        if create_acquisition::play_ready_cue(&mut provider, &mut watchdog)
            .await
            .is_err()
        {
            let kind = create_play::request_kind();
            if kind != create_play::RequestKind::None && create_play::claim_pending(kind) {
                create_play::set_result(7);
                create_play::set_state(create_play::RequestState::Refused);
            }
            STATE.store(State::UartFault as u8, Ordering::Release);
            create_link_gate::set_translator(&mut translator_oe, false);
            continue;
        }
        if create_play::request_kind() == create_play::RequestKind::Hello {
            if create_play::claim_pending(create_play::RequestKind::Hello) {
                if create_acquisition::restore_safe(&mut provider, &mut watchdog)
                    .await
                    .is_ok()
                {
                    OI_MODE.store(2, Ordering::Release);
                    STATE.store(State::Safe as u8, Ordering::Release);
                    create_play::set_result(0);
                    create_play::set_state(create_play::RequestState::Completed);
                } else {
                    OI_MODE.store(0, Ordering::Release);
                    STATE.store(State::LinkLost as u8, Ordering::Release);
                    create_play::set_result(7);
                    create_play::set_state(create_play::RequestState::Refused);
                }
            }
            create_link_gate::set_translator(&mut translator_oe, false);
            continue;
        }
        let mut next_full_refresh = Instant::now() + Duration::from_millis(FULL_REFRESH_MS);
        let mut next_safety_poll = Instant::now();
        let mut next_charging_poll = Instant::now();
        let mut next_mode_poll = Instant::now();
        loop {
            if !create_link_gate::authorized() {
                motion.link_lost(&mut provider);
                STATE.store(State::Initializing as u8, Ordering::Release);
                OI_MODE.store(0, Ordering::Release);
                create_link_gate::set_translator(&mut translator_oe, false);
                break;
            }
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
        if !create_link_gate::authorized() {
            create_link_gate::set_translator(&mut translator_oe, false);
        }
        watchdog_delay(&mut watchdog, REACQUIRE_COOLDOWN_MS).await;
    }
}
