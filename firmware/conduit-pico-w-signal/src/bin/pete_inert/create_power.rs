//! Build-bound, attended Create power-toggle pulse.

use core::fmt::Write as _;

use conduit_create_oi::{CreatePowerPulseProfile, CreatePowerPulseProgress, CreatePowerToggle, CreatePowerToggleProvider};
use embassy_rp::gpio::Output;
use embassy_rp::watchdog::Watchdog;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use portable_atomic::{AtomicU8, Ordering};

use crate::{create_link_gate, send_control_frame, InertCdc, BOOTSEL_FRAME_MAX};

const REQUEST_PREFIX: &str = "CONDUIT_WAKE_CREATE@1:";
const AUTHORITY_GRANT: &str = "grant/pete-pico-confirmed-off-wake-hil";
const LOW_SETTLE_MS: u32 = 5;
const HIGH_PULSE_MS: u32 = 500;
const TRANSLATOR_SETTLE_MS: u64 = 10;
const WATCHDOG_TIMEOUT_MS: u64 = 2_000;
const IDLE: u8 = 0;
const PREPARED: u8 = 1;
const PENDING: u8 = 2;
const ACTIVE: u8 = 3;
const COMPLETED: u8 = 4;
const FAILED: u8 = 5;
static STATE: AtomicU8 = AtomicU8::new(IDLE);

struct Provider<'a> { output: &'a mut Output<'static> }

impl CreatePowerToggleProvider for Provider<'_> {
    type Error = core::convert::Infallible;
    fn is_available(&self) -> bool { true }
    fn set_output_low(&mut self) -> Result<(), Self::Error> { self.output.set_low(); Ok(()) }
    fn set_output_high(&mut self) -> Result<(), Self::Error> { self.output.set_high(); Ok(()) }
}

pub fn request_matches(request: &[u8]) -> bool {
    let mut expected = String::<BOOTSEL_FRAME_MAX>::new();
    write!(expected, "{REQUEST_PREFIX}{}:{AUTHORITY_GRANT}", env!("CONDUIT_PETE_CAPSTONE_BUILD_ID")).is_ok()
        && request == expected.as_bytes()
}

pub async fn serve(class: &mut InertCdc) {
    if STATE.compare_exchange(IDLE, PREPARED, Ordering::AcqRel, Ordering::Acquire).is_err() { return; }
    let mut accepted = String::<BOOTSEL_FRAME_MAX>::new();
    let _ = write!(accepted, concat!(
        "{{\"schema\":\"conduit.pete/create-power-pulse-accepted@1\",",
        "\"build_id\":\"{}\",\"state\":\"accepted_low\",",
        "\"authority_grant_id\":\"{}\",\"gpio\":18,",
        "\"current_level\":\"low\",\"translator_enabled\":false,",
        "\"uart_commands\":0,\"motion_commanded\":false}}"
    ), env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"), AUTHORITY_GRANT);
    if send_control_frame(class, accepted.as_bytes()).await.is_err() { STATE.store(IDLE, Ordering::Release); return; }
    STATE.store(PENDING, Ordering::Release);
    let deadline = Instant::now() + Duration::from_millis(1_000);
    while Instant::now() < deadline && !matches!(STATE.load(Ordering::Acquire), COMPLETED | FAILED) {
        Timer::after(Duration::from_millis(5)).await;
    }
    let completed = STATE.load(Ordering::Acquire) == COMPLETED;
    let state = if completed { "completed_low" } else { "failed_low" };
    let mut receipt = String::<BOOTSEL_FRAME_MAX>::new();
    let _ = write!(receipt, concat!(
        "{{\"schema\":\"conduit.pete/create-power-pulse@1\",",
        "\"build_id\":\"{}\",\"success\":{},\"state\":\"{}\",",
        "\"authority_grant_id\":\"{}\",\"prior_power_state\":\"confirmed_off\",",
        "\"post_pulse_power_state\":\"awaiting_uart_verification\",",
        "\"gpio\":18,\"low_settle_ms\":5,\"high_pulse_ms\":500,",
        "\"final_level\":\"low\",\"translator_final_level\":\"low\",",
        "\"translator_enabled_during_pulse\":true,\"uart_commands\":0,",
        "\"motion_commanded\":false}}"
    ), env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"), completed, state, AUTHORITY_GRANT);
    let _ = send_control_frame(class, receipt.as_bytes()).await;
    STATE.store(IDLE, Ordering::Release);
}

pub fn claim_pending() -> bool {
    STATE.compare_exchange(PENDING, ACTIVE, Ordering::AcqRel, Ordering::Acquire).is_ok()
}

pub async fn execute(output: &mut Output<'static>, translator_oe: &mut Output<'static>, watchdog: &mut Watchdog) {
    output.set_low();
    create_link_gate::set_translator(translator_oe, true);
    Timer::after(Duration::from_millis(TRANSLATOR_SETTLE_MS)).await;
    watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
    let mut provider = Provider { output };
    let mut toggle = CreatePowerToggle::new(CreatePowerPulseProfile { low_settle_ticks: LOW_SETTLE_MS, high_pulse_ticks: HIGH_PULSE_MS });
    let started = Instant::now().as_millis();
    let completed = match toggle.start(&mut provider, started) {
        Ok(CreatePowerPulseProgress::WaitingLowSettle { raise_at_tick }) => {
            Timer::after(Duration::from_millis(raise_at_tick.saturating_sub(Instant::now().as_millis()))).await;
            match toggle.advance(&mut provider, Instant::now().as_millis()) {
                Ok(CreatePowerPulseProgress::WaitingHighPulse { lower_at_tick }) => {
                    Timer::after(Duration::from_millis(lower_at_tick.saturating_sub(Instant::now().as_millis()))).await;
                    watchdog.feed(Duration::from_millis(WATCHDOG_TIMEOUT_MS));
                    matches!(toggle.advance(&mut provider, Instant::now().as_millis()), Ok(CreatePowerPulseProgress::CompletedLow))
                }
                _ => false,
            }
        }
        _ => false,
    };
    provider.output.set_low();
    create_link_gate::set_translator(translator_oe, false);
    STATE.store(if completed { COMPLETED } else { FAILED }, Ordering::Release);
}
