//! Attended one-command Create 1 PLAY-LED isolation probe.

use core::fmt::Write as _;

use conduit_create_oi::{
    presentation_bytes_are_motion_free, require_provider, CreateUartProvider, PRESENTATION_FULL,
    PRESENTATION_SAFE, PRESENTATION_START,
};
use embassy_rp::gpio::Output;
use embassy_rp::watchdog::Watchdog;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use portable_atomic::{AtomicBool, AtomicU8, Ordering};

use super::create_control::{watchdog_delay, Provider};
use crate::{create_link_gate, create_play, send_control_frame, InertCdc, BOOTSEL_FRAME_MAX};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_SINGLE_LED@1:";
pub const AUTHORITY_GRANT: &str = "grant/pete-create-single-led-no-motion-hil";
const START_SETTLE_MS: u64 = 20;
const TRANSLATOR_SETTLE_MS: u64 = 10;
pub const HOLD_MS: u64 = 60_000;
pub const PLAY_ONLY_LED: [u8; 4] = [139, 0x02, 0, 0];

static START_SENT: AtomicBool = AtomicBool::new(false);
static FULL_SENT: AtomicBool = AtomicBool::new(false);
static LED_SENT: AtomicBool = AtomicBool::new(false);
static HOLD_COMPLETED: AtomicBool = AtomicBool::new(false);
static TRANSLATOR_LOW_DURING_HOLD: AtomicBool = AtomicBool::new(false);
static SAFE_SENT: AtomicBool = AtomicBool::new(false);
static UART_TX_BYTES: AtomicU8 = AtomicU8::new(0);

const _: () = {
    assert!(PLAY_ONLY_LED[0] == 139);
    assert!(PLAY_ONLY_LED[1] == 0x02);
    assert!(PLAY_ONLY_LED[2] == 0);
    assert!(PLAY_ONLY_LED[3] == 0);
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_START));
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_FULL));
    assert!(presentation_bytes_are_motion_free(&PLAY_ONLY_LED));
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_SAFE));
};

fn reset_report() {
    START_SENT.store(false, Ordering::Release);
    FULL_SENT.store(false, Ordering::Release);
    LED_SENT.store(false, Ordering::Release);
    HOLD_COMPLETED.store(false, Ordering::Release);
    TRANSLATOR_LOW_DURING_HOLD.store(false, Ordering::Release);
    SAFE_SENT.store(false, Ordering::Release);
    UART_TX_BYTES.store(0, Ordering::Release);
}

fn write_exact(provider: &mut Provider, bytes: &[u8]) -> bool {
    if !presentation_bytes_are_motion_free(bytes) || require_provider(provider).is_err() {
        return false;
    }
    if provider.write_all(bytes).is_err() {
        return false;
    }
    UART_TX_BYTES.fetch_add(bytes.len() as u8, Ordering::AcqRel);
    true
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
    match create_play::submit(create_play::RequestKind::SingleLed) {
        Ok(generation) => {
            let deadline = Instant::now() + Duration::from_millis(HOLD_MS + 5_000);
            loop {
                let request = create_play::snapshot();
                if request.generation == generation && request.state.terminal() {
                    let success = request.state == create_play::RequestState::Completed;
                    let _ = write!(
                        response,
                        concat!(
                            "{{\"schema\":\"conduit.pete/create-single-led-receipt@1\",",
                            "\"build_id\":\"{}\",\"success\":{},\"generation\":{},",
                            "\"state\":\"{}\",\"result_code\":{},",
                            "\"start_command_sent\":{},\"full_command_sent\":{},",
                            "\"led_command\":[139,2,0,0],\"led_command_sent\":{},",
                            "\"requested_indicator\":\"play\",",
                            "\"power_color\":0,\"power_intensity\":0,",
                            "\"hold_ms\":60000,\"hold_completed\":{},",
                            "\"translator_low_during_hold\":{},",
                            "\"mode_observed\":false,\"physical_led_observed\":false,",
                            "\"safe_cleanup_command_sent\":{},",
                            "\"translator_final_level\":\"low\",",
                            "\"uart_tx_bytes\":{},\"uart_rx_required\":false,",
                            "\"music_commands_sent\":0,\"motion_authority_granted\":false,",
                            "\"authority_grant_id\":\"{}\"}}"
                        ),
                        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
                        success,
                        generation,
                        request.state.name(),
                        request.result_code,
                        START_SENT.load(Ordering::Acquire),
                        FULL_SENT.load(Ordering::Acquire),
                        LED_SENT.load(Ordering::Acquire),
                        HOLD_COMPLETED.load(Ordering::Acquire),
                        TRANSLATOR_LOW_DURING_HOLD.load(Ordering::Acquire),
                        SAFE_SENT.load(Ordering::Acquire),
                        UART_TX_BYTES.load(Ordering::Acquire),
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
                "{{\"schema\":\"conduit.pete/create-single-led-receipt@1\",\"build_id\":\"{}\",\"success\":false,\"state\":\"busy\",\"result_code\":8,\"translator_final_level\":\"low\",\"motion_authority_granted\":false}}",
                env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
            );
            let _ = send_control_frame(class, response.as_bytes()).await;
        }
    }
}

/// Execute after the caller raises and settles OE. This operation lowers OE
/// before the observation hold and owns the bounded cleanup re-enable itself.
pub async fn execute(
    provider: &mut Provider,
    translator_oe: &mut Output<'static>,
    watchdog: &mut Watchdog,
) -> bool {
    reset_report();
    create_play::set_state(create_play::RequestState::Active);

    let mut completed = create_link_gate::authorized()
        && write_exact(provider, &PRESENTATION_START);
    START_SENT.store(completed, Ordering::Release);
    if completed {
        watchdog_delay(watchdog, START_SETTLE_MS).await;
        completed = create_link_gate::authorized() && write_exact(provider, &PRESENTATION_FULL);
        FULL_SENT.store(completed, Ordering::Release);
    }
    if completed {
        watchdog_delay(watchdog, START_SETTLE_MS).await;
        completed = create_link_gate::authorized() && write_exact(provider, &PLAY_ONLY_LED);
        LED_SENT.store(completed, Ordering::Release);
    }

    create_link_gate::set_translator(translator_oe, false);
    let isolated = !create_link_gate::translator_enabled();
    TRANSLATOR_LOW_DURING_HOLD.store(isolated, Ordering::Release);
    if completed && isolated {
        watchdog_delay(watchdog, HOLD_MS).await;
        HOLD_COMPLETED.store(true, Ordering::Release);
    }

    let safe = if create_link_gate::authorized() {
        create_link_gate::set_translator(translator_oe, true);
        watchdog_delay(watchdog, TRANSLATOR_SETTLE_MS).await;
        let safe = write_exact(provider, &PRESENTATION_SAFE);
        create_link_gate::set_translator(translator_oe, false);
        watchdog_delay(watchdog, START_SETTLE_MS).await;
        safe
    } else {
        false
    };
    SAFE_SENT.store(safe, Ordering::Release);
    create_link_gate::set_translator(translator_oe, false);

    completed
        && isolated
        && HOLD_COMPLETED.load(Ordering::Acquire)
        && safe
        && UART_TX_BYTES.load(Ordering::Acquire) == 8
}
