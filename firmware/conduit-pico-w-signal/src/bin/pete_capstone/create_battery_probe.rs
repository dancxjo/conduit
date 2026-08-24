//! Attended one-query Create 1 OI battery RX probe.

use core::fmt::Write as _;

use conduit_create_oi::{
    decode_sensor_packet, presentation_bytes_are_motion_free, require_provider,
    CreateUartProvider, PRESENTATION_SAFE, PRESENTATION_START,
};
use embassy_rp::watchdog::Watchdog;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_nb::serial::Read as _;
use heapless::String;
use portable_atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering};

use super::create_control::{now_ms, watchdog_delay, Provider};
use crate::{
    create_link_gate, create_play, send_control_frame, uart_diagnostic, InertCdc,
    BOOTSEL_FRAME_MAX,
};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_BATTERY_RX@1:";
pub const AUTHORITY_GRANT: &str = "grant/pete-create-battery-rx-no-motion-hil";
const GROUP_ZERO_PACKET_ID: u8 = 0;
const GROUP_ZERO_BYTES: usize = 26;
const START_SETTLE_MS: u64 = 20;
const RESPONSE_DEADLINE_MS: u64 = 1_000;
const SENSOR_QUERY: [u8; 2] = [142, GROUP_ZERO_PACKET_ID];

static START_SENT: AtomicBool = AtomicBool::new(false);
static QUERY_SENT: AtomicBool = AtomicBool::new(false);
static RX_VALID: AtomicBool = AtomicBool::new(false);
static SAFE_SENT: AtomicBool = AtomicBool::new(false);
static PREQUERY_DISCARDED: AtomicU8 = AtomicU8::new(0);
static RX_BYTES: AtomicU8 = AtomicU8::new(0);
static RX_OUTCOME: AtomicU8 = AtomicU8::new(RxOutcome::NotAttempted as u8);
static UART_TX_BYTES: AtomicU8 = AtomicU8::new(0);
static CHARGING_STATE: AtomicU8 = AtomicU8::new(0);
static MILLIVOLTS: AtomicU16 = AtomicU16::new(0);
static MILLIAMPS_BITS: AtomicU16 = AtomicU16::new(0);
static TEMPERATURE_BITS: AtomicU8 = AtomicU8::new(0);
static CHARGE_MAH: AtomicU16 = AtomicU16::new(0);
static CAPACITY_MAH: AtomicU16 = AtomicU16::new(0);
static RAW_RX: [AtomicU8; GROUP_ZERO_BYTES] =
    [const { AtomicU8::new(0) }; GROUP_ZERO_BYTES];

const _: () = {
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_START));
    assert!(presentation_bytes_are_motion_free(&SENSOR_QUERY));
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_SAFE));
};

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
enum RxOutcome {
    NotAttempted = 0,
    Absent = 1,
    Truncated = 2,
    Malformed = 3,
    Inconsistent = 4,
    ReadError = 5,
    AuthorityLost = 6,
    Valid = 7,
}

impl RxOutcome {
    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Absent,
            2 => Self::Truncated,
            3 => Self::Malformed,
            4 => Self::Inconsistent,
            5 => Self::ReadError,
            6 => Self::AuthorityLost,
            7 => Self::Valid,
            _ => Self::NotAttempted,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::Absent => "absent",
            Self::Truncated => "truncated",
            Self::Malformed => "malformed",
            Self::Inconsistent => "inconsistent",
            Self::ReadError => "read_error",
            Self::AuthorityLost => "authority_lost",
            Self::Valid => "valid",
        }
    }
}

#[derive(Clone, Copy)]
struct BatterySnapshot {
    raw: [u8; GROUP_ZERO_BYTES],
    start_sent: bool,
    query_sent: bool,
    rx_valid: bool,
    safe_sent: bool,
    prequery_discarded: u8,
    rx_bytes: u8,
    rx_outcome: RxOutcome,
    uart_tx_bytes: u8,
    charging_state: u8,
    millivolts: u16,
    milliamps: i16,
    temperature_celsius: i8,
    charge_mah: u16,
    capacity_mah: u16,
}

impl BatterySnapshot {
    fn charge_permille(self) -> i32 {
        if self.capacity_mah == 0 {
            -1
        } else {
            i32::from(self.charge_mah) * 1_000 / i32::from(self.capacity_mah)
        }
    }
}

fn snapshot() -> BatterySnapshot {
    let mut raw = [0_u8; GROUP_ZERO_BYTES];
    for (target, source) in raw.iter_mut().zip(&RAW_RX) {
        *target = source.load(Ordering::Acquire);
    }
    BatterySnapshot {
        raw,
        start_sent: START_SENT.load(Ordering::Acquire),
        query_sent: QUERY_SENT.load(Ordering::Acquire),
        rx_valid: RX_VALID.load(Ordering::Acquire),
        safe_sent: SAFE_SENT.load(Ordering::Acquire),
        prequery_discarded: PREQUERY_DISCARDED.load(Ordering::Acquire),
        rx_bytes: RX_BYTES.load(Ordering::Acquire),
        rx_outcome: RxOutcome::from_u8(RX_OUTCOME.load(Ordering::Acquire)),
        uart_tx_bytes: UART_TX_BYTES.load(Ordering::Acquire),
        charging_state: CHARGING_STATE.load(Ordering::Acquire),
        millivolts: MILLIVOLTS.load(Ordering::Acquire),
        milliamps: MILLIAMPS_BITS.load(Ordering::Acquire) as i16,
        temperature_celsius: TEMPERATURE_BITS.load(Ordering::Acquire) as i8,
        charge_mah: CHARGE_MAH.load(Ordering::Acquire),
        capacity_mah: CAPACITY_MAH.load(Ordering::Acquire),
    }
}

fn reset_report() {
    START_SENT.store(false, Ordering::Release);
    QUERY_SENT.store(false, Ordering::Release);
    RX_VALID.store(false, Ordering::Release);
    SAFE_SENT.store(false, Ordering::Release);
    PREQUERY_DISCARDED.store(0, Ordering::Release);
    RX_BYTES.store(0, Ordering::Release);
    RX_OUTCOME.store(RxOutcome::NotAttempted as u8, Ordering::Release);
    UART_TX_BYTES.store(0, Ordering::Release);
    CHARGING_STATE.store(0, Ordering::Release);
    MILLIVOLTS.store(0, Ordering::Release);
    MILLIAMPS_BITS.store(0, Ordering::Release);
    TEMPERATURE_BITS.store(0, Ordering::Release);
    CHARGE_MAH.store(0, Ordering::Release);
    CAPACITY_MAH.store(0, Ordering::Release);
    for byte in &RAW_RX {
        byte.store(0, Ordering::Release);
    }
}

fn write_exact(provider: &mut Provider, bytes: &[u8]) -> bool {
    let admitted = bytes == PRESENTATION_START || bytes == SENSOR_QUERY || bytes == PRESENTATION_SAFE;
    if !admitted
        || !presentation_bytes_are_motion_free(bytes)
        || require_provider(provider).is_err()
    {
        return false;
    }
    if provider.write_all(bytes).is_err() {
        return false;
    }
    UART_TX_BYTES.fetch_add(bytes.len() as u8, Ordering::AcqRel);
    true
}

fn discard_prequery(provider: &mut Provider) -> bool {
    let mut discarded = 0_u8;
    for _ in 0..32 {
        match provider.uart.read() {
            Ok(_) => {
                discarded = discarded.saturating_add(1);
                uart_diagnostic::record_rx(now_ms());
            }
            Err(nb::Error::WouldBlock) => break,
            Err(nb::Error::Other(error)) => {
                RX_OUTCOME.store(RxOutcome::ReadError as u8, Ordering::Release);
                uart_diagnostic::record_error(error);
                PREQUERY_DISCARDED.store(discarded, Ordering::Release);
                return false;
            }
        }
    }
    if discarded != 0 {
        uart_diagnostic::record_discard(usize::from(discarded));
    }
    PREQUERY_DISCARDED.store(discarded, Ordering::Release);
    true
}

fn accept_payload(payload: &[u8; GROUP_ZERO_BYTES]) -> bool {
    if decode_sensor_packet(GROUP_ZERO_PACKET_ID, payload).is_err() {
        RX_OUTCOME.store(RxOutcome::Malformed as u8, Ordering::Release);
        uart_diagnostic::record_frame(GROUP_ZERO_PACKET_ID, payload, false);
        return false;
    }
    let charge_mah = u16::from_be_bytes([payload[22], payload[23]]);
    let capacity_mah = u16::from_be_bytes([payload[24], payload[25]]);
    let millivolts = u16::from_be_bytes([payload[17], payload[18]]);
    if millivolts > 60_000
        || capacity_mah == 0
        || charge_mah > capacity_mah
    {
        RX_OUTCOME.store(RxOutcome::Inconsistent as u8, Ordering::Release);
        uart_diagnostic::record_frame(GROUP_ZERO_PACKET_ID, payload, false);
        return false;
    }
    CHARGING_STATE.store(payload[16], Ordering::Release);
    MILLIVOLTS.store(millivolts, Ordering::Release);
    MILLIAMPS_BITS.store(
        i16::from_be_bytes([payload[19], payload[20]]) as u16,
        Ordering::Release,
    );
    TEMPERATURE_BITS.store(payload[21], Ordering::Release);
    CHARGE_MAH.store(charge_mah, Ordering::Release);
    CAPACITY_MAH.store(capacity_mah, Ordering::Release);
    RX_OUTCOME.store(RxOutcome::Valid as u8, Ordering::Release);
    uart_diagnostic::record_frame(GROUP_ZERO_PACKET_ID, payload, true);
    true
}

async fn read_group_zero(provider: &mut Provider, watchdog: &mut Watchdog) -> bool {
    let deadline = Instant::now() + Duration::from_millis(RESPONSE_DEADLINE_MS);
    let mut payload = [0_u8; GROUP_ZERO_BYTES];
    let mut received = 0_usize;
    while received < GROUP_ZERO_BYTES
        && Instant::now() < deadline
        && create_link_gate::authorized()
    {
        match provider.uart.read() {
            Ok(byte) => {
                payload[received] = byte;
                RAW_RX[received].store(byte, Ordering::Release);
                received += 1;
                RX_BYTES.store(received as u8, Ordering::Release);
                uart_diagnostic::record_rx(now_ms());
                watchdog.feed(Duration::from_millis(2_000));
            }
            Err(nb::Error::WouldBlock) => {
                Timer::after(Duration::from_millis(1)).await;
                watchdog.feed(Duration::from_millis(2_000));
            }
            Err(nb::Error::Other(error)) => {
                RX_OUTCOME.store(RxOutcome::ReadError as u8, Ordering::Release);
                uart_diagnostic::record_error(error);
                if received != 0 {
                    uart_diagnostic::record_frame(
                        GROUP_ZERO_PACKET_ID,
                        &payload[..received],
                        false,
                    );
                }
                return false;
            }
        }
    }
    if received != GROUP_ZERO_BYTES {
        RX_OUTCOME.store(
            if !create_link_gate::authorized() {
                RxOutcome::AuthorityLost
            } else if received == 0 {
                RxOutcome::Absent
            } else {
                RxOutcome::Truncated
            } as u8,
            Ordering::Release,
        );
        uart_diagnostic::record_timeout();
        if received != 0 {
            uart_diagnostic::record_frame(
                GROUP_ZERO_PACKET_ID,
                &payload[..received],
                false,
            );
        }
        return false;
    }
    accept_payload(&payload)
}

pub fn request_matches(request: &[u8]) -> bool {
    let mut expected = String::<BOOTSEL_FRAME_MAX>::new();
    write!(
        expected,
        "{REQUEST_PREFIX}{}:{AUTHORITY_GRANT}",
        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID")
    )
    .is_ok()
        && request == expected.as_bytes()
}

pub async fn serve(class: &mut InertCdc) {
    let mut response = String::<BOOTSEL_FRAME_MAX>::new();
    match create_play::submit(create_play::RequestKind::BatteryRx) {
        Ok(generation) => {
            let deadline = Instant::now() + Duration::from_millis(3_000);
            loop {
                let request = create_play::snapshot();
                if request.generation == generation && request.state.terminal() {
                    let battery = snapshot();
                    let mut raw_hex = String::<{ GROUP_ZERO_BYTES * 2 }>::new();
                    for byte in battery
                        .raw
                        .iter()
                        .take(usize::from(battery.rx_bytes))
                    {
                        let _ = write!(raw_hex, "{byte:02x}");
                    }
                    let success = request.state == create_play::RequestState::Completed;
                    let _ = write!(
                        response,
                        concat!(
                            "{{\"schema\":\"conduit.pete/create-battery-rx@1\",",
                            "\"build_id\":\"{}\",\"success\":{},\"state\":\"{}\",",
                            "\"result_code\":{},\"start_sent\":{},",
                            "\"query\":[142,0],\"query_sent\":{},",
                            "\"prequery_discarded\":{},\"rx_bytes\":{},",
                            "\"rx_outcome\":\"{}\",\"rx_valid\":{},\"rx_hex\":\"{}\",",
                            "\"charging_state\":{},\"millivolts\":{},",
                            "\"milliamps\":{},\"temperature_celsius\":{},",
                            "\"charge_mah\":{},\"capacity_mah\":{},",
                            "\"charge_permille\":{},\"safe_sent\":{},",
                            "\"oe_final\":\"low\",\"uart_tx_bytes\":{},",
                            "\"motion_authority\":false,\"grant\":\"{}\"}}"
                        ),
                        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
                        success,
                        request.state.name(),
                        request.result_code,
                        battery.start_sent,
                        battery.query_sent,
                        battery.prequery_discarded,
                        battery.rx_bytes,
                        battery.rx_outcome.name(),
                        battery.rx_valid,
                        raw_hex,
                        battery.charging_state,
                        battery.millivolts,
                        battery.milliamps,
                        battery.temperature_celsius,
                        battery.charge_mah,
                        battery.capacity_mah,
                        battery.charge_permille(),
                        battery.safe_sent,
                        battery.uart_tx_bytes,
                        AUTHORITY_GRANT,
                    );
                    let _ = send_control_frame(class, response.as_bytes()).await;
                    create_play::release(generation);
                    break;
                }
                if Instant::now() >= deadline {
                    create_play::timeout(generation);
                }
                Timer::after(Duration::from_millis(5)).await;
            }
        }
        Err(()) => {
            let _ = write!(
                response,
                "{{\"schema\":\"conduit.pete/create-battery-rx@1\",\"build_id\":\"{}\",\"success\":false,\"state\":\"busy\",\"result_code\":8,\"oe_final\":\"low\",\"motion_authority\":false}}",
                env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
            );
            let _ = send_control_frame(class, response.as_bytes()).await;
        }
    }
}

/// Execute after the sole UART owner raises and settles OE. The caller lowers
/// OE before publishing the terminal request state.
pub async fn execute(provider: &mut Provider, watchdog: &mut Watchdog) -> bool {
    reset_report();
    create_play::set_state(create_play::RequestState::Active);

    let start = create_link_gate::authorized() && write_exact(provider, &PRESENTATION_START);
    START_SENT.store(start, Ordering::Release);
    let prequery_readable = if start {
        watchdog_delay(watchdog, START_SETTLE_MS).await;
        discard_prequery(provider)
    } else {
        false
    };

    let query = prequery_readable
        && create_link_gate::authorized()
        && write_exact(provider, &SENSOR_QUERY);
    QUERY_SENT.store(query, Ordering::Release);
    let rx_valid = query && read_group_zero(provider, watchdog).await;
    RX_VALID.store(rx_valid, Ordering::Release);

    let safe = create_link_gate::authorized() && write_exact(provider, &PRESENTATION_SAFE);
    SAFE_SENT.store(safe, Ordering::Release);
    watchdog_delay(watchdog, START_SETTLE_MS).await;

    start
        && query
        && PREQUERY_DISCARDED.load(Ordering::Acquire) == 0
        && rx_valid
        && safe
        && UART_TX_BYTES.load(Ordering::Acquire) == 5
}
