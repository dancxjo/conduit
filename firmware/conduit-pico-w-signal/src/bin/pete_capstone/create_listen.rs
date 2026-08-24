//! Attended transmit-zero observation of the Create RX electrical boundary.

use core::fmt::Write as _;

use embassy_rp::gpio::Output;
use embassy_rp::watchdog::Watchdog;
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_nb::serial::Read as _;
use heapless::String;
use portable_atomic::{AtomicU32, AtomicU8, Ordering};

use super::create_control::{now_ms, Provider};
use crate::{create_link_gate, send_control_frame, uart_diagnostic, InertCdc, BOOTSEL_FRAME_MAX};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_RX_LISTEN@1:";
const WINDOW_MS: u64 = 1_000;
const IDLE: u8 = 0;
const PENDING: u8 = 1;
const ACTIVE: u8 = 2;
const COMPLETED: u8 = 3;
static STATE: AtomicU8 = AtomicU8::new(IDLE);
static RX_BYTES: AtomicU32 = AtomicU32::new(0);
static OVERRUNS: AtomicU32 = AtomicU32::new(0);
static BREAKS: AtomicU32 = AtomicU32::new(0);
static PARITY_ERRORS: AtomicU32 = AtomicU32::new(0);
static FRAMING_ERRORS: AtomicU32 = AtomicU32::new(0);
static OTHER_ERRORS: AtomicU32 = AtomicU32::new(0);

pub fn request_matches(request: &[u8]) -> bool {
    let mut expected = String::<BOOTSEL_FRAME_MAX>::new();
    write!(
        expected,
        "{REQUEST_PREFIX}{}",
        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID")
    )
    .is_ok()
        && request == expected.as_bytes()
}

pub async fn serve(class: &mut InertCdc) {
    let mut response = String::<BOOTSEL_FRAME_MAX>::new();
    if STATE
        .compare_exchange(IDLE, PENDING, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let deadline = Instant::now() + Duration::from_millis(2_000);
    while Instant::now() < deadline && STATE.load(Ordering::Acquire) != COMPLETED {
        Timer::after(Duration::from_millis(5)).await;
    }
    let completed = STATE.load(Ordering::Acquire) == COMPLETED;
    let _ = write!(
        response,
        "{{\"schema\":\"conduit.pete/create-rx-listen@1\",\"build_id\":\"{}\",\"success\":{},\"window_ms\":1000,\"translator_final_level\":\"low\",\"uart_tx_bytes\":0,\"rx_bytes\":{},\"errors\":{{\"overrun\":{},\"break\":{},\"parity\":{},\"framing\":{},\"other\":{}}},\"motion_authority_granted\":false}}",
        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
        completed,
        RX_BYTES.load(Ordering::Acquire),
        OVERRUNS.load(Ordering::Acquire),
        BREAKS.load(Ordering::Acquire),
        PARITY_ERRORS.load(Ordering::Acquire),
        FRAMING_ERRORS.load(Ordering::Acquire),
        OTHER_ERRORS.load(Ordering::Acquire),
    );
    let _ = send_control_frame(class, response.as_bytes()).await;
    STATE.store(IDLE, Ordering::Release);
}

pub fn claim_pending() -> bool {
    STATE
        .compare_exchange(PENDING, ACTIVE, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

pub async fn execute(
    provider: &mut Provider,
    translator_oe: &mut Output<'static>,
    watchdog: &mut Watchdog,
) {
    let before = uart_diagnostic::snapshot();
    create_link_gate::set_translator(translator_oe, true);
    Timer::after(Duration::from_millis(10)).await;
    let deadline = Instant::now() + Duration::from_millis(WINDOW_MS);
    while Instant::now() < deadline {
        match provider.uart.read() {
            Ok(_) => {
                uart_diagnostic::record_rx(now_ms());
                uart_diagnostic::record_discard(1);
            }
            Err(nb::Error::WouldBlock) => Timer::after(Duration::from_millis(1)).await,
            Err(nb::Error::Other(error)) => uart_diagnostic::record_error(error),
        }
        watchdog.feed(Duration::from_millis(2_000));
    }
    create_link_gate::set_translator(translator_oe, false);
    let after = uart_diagnostic::snapshot();
    RX_BYTES.store(after.rx_bytes.wrapping_sub(before.rx_bytes), Ordering::Release);
    OVERRUNS.store(after.overruns.wrapping_sub(before.overruns), Ordering::Release);
    BREAKS.store(after.breaks.wrapping_sub(before.breaks), Ordering::Release);
    PARITY_ERRORS.store(
        after.parity_errors.wrapping_sub(before.parity_errors),
        Ordering::Release,
    );
    FRAMING_ERRORS.store(
        after.framing_errors.wrapping_sub(before.framing_errors),
        Ordering::Release,
    );
    OTHER_ERRORS.store(
        after.other_errors.wrapping_sub(before.other_errors),
        Ordering::Release,
    );
    STATE.store(COMPLETED, Ordering::Release);
}
