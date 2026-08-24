//! Stage-four attended Create Full-mode supervision-light pattern.

use core::fmt::Write as _;

use conduit_create_oi::{
    presentation_bytes_are_motion_free, presentation_lights, require_provider,
    CreateUartProvider, PRESENTATION_FULL, PRESENTATION_LIGHTS_OFF, PRESENTATION_LIGHT_STEPS,
    PRESENTATION_LIGHT_STEP_MS, PRESENTATION_SAFE, PRESENTATION_START,
};
use embassy_rp::watchdog::Watchdog;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use portable_atomic::{AtomicBool, AtomicU8, Ordering};

use super::create_control::{watchdog_delay, Provider};
use crate::{create_link_gate, create_play, send_control_frame, InertCdc, BOOTSEL_FRAME_MAX};

const REQUEST_PREFIX: &str = "CONDUIT_CREATE_LIGHTS_STAGE@1:";
pub const AUTHORITY_GRANT: &str = "grant/pete-create-lights-no-motion-hil";
const START_SETTLE_MS: u64 = 20;
const MODE_SETTLE_MS: u64 = 100;

static START_SENT: AtomicBool = AtomicBool::new(false);
static FULL_SENT: AtomicBool = AtomicBool::new(false);
static LIGHT_STEPS: AtomicU8 = AtomicU8::new(0);
static LIGHTS_OFF_SENT: AtomicBool = AtomicBool::new(false);
static SAFE_SENT: AtomicBool = AtomicBool::new(false);

fn reset_report() {
    START_SENT.store(false, Ordering::Release);
    FULL_SENT.store(false, Ordering::Release);
    LIGHT_STEPS.store(0, Ordering::Release);
    LIGHTS_OFF_SENT.store(false, Ordering::Release);
    SAFE_SENT.store(false, Ordering::Release);
}

fn write_exact(provider: &mut Provider, bytes: &[u8]) -> bool {
    presentation_bytes_are_motion_free(bytes)
        && !bytes.contains(&140)
        && !bytes.contains(&141)
        && require_provider(provider).is_ok()
        && provider.write_all(bytes).is_ok()
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
    match create_play::submit(create_play::RequestKind::LightsStage) {
        Ok(generation) => {
            let deadline = Instant::now() + Duration::from_millis(8_000);
            loop {
                let request = create_play::snapshot();
                if request.generation == generation && request.state.terminal() {
                    let success = request.state == create_play::RequestState::Completed;
                    let _ = write!(
                        response,
                        concat!(
                            "{{\"schema\":\"conduit.pete/create-lights-stage-receipt@1\",",
                            "\"build_id\":\"{}\",\"success\":{},\"generation\":{},",
                            "\"state\":\"{}\",\"result_code\":{},",
                            "\"pattern\":\"netherwick-healthy-lights@1\",",
                            "\"start_command_sent\":{},\"full_command_sent\":{},",
                            "\"mode_observed\":false,\"light_steps\":{},",
                            "\"light_step_ms\":800,\"music_commands_sent\":0,",
                            "\"lights_off_sent\":{},\"safe_cleanup_command_sent\":{},",
                            "\"translator_final_level\":\"low\",",
                            "\"motion_authority_granted\":false,",
                            "\"authority_grant_id\":\"{}\"}}"
                        ),
                        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
                        success,
                        generation,
                        request.state.name(),
                        request.result_code,
                        START_SENT.load(Ordering::Acquire),
                        FULL_SENT.load(Ordering::Acquire),
                        LIGHT_STEPS.load(Ordering::Acquire),
                        LIGHTS_OFF_SENT.load(Ordering::Acquire),
                        SAFE_SENT.load(Ordering::Acquire),
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
                "{{\"schema\":\"conduit.pete/create-lights-stage-receipt@1\",\"build_id\":\"{}\",\"success\":false,\"state\":\"busy\",\"result_code\":8,\"translator_final_level\":\"low\",\"motion_authority_granted\":false}}",
                env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
            );
            let _ = send_control_frame(class, response.as_bytes()).await;
        }
    }
}

/// Execute after the caller raises and settles OE. The caller lowers OE before
/// publishing the terminal state.
pub async fn execute(provider: &mut Provider, watchdog: &mut Watchdog) -> bool {
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
        watchdog_delay(watchdog, MODE_SETTLE_MS).await;
    }

    let mut step = 0;
    while completed && step < PRESENTATION_LIGHT_STEPS {
        completed = write_exact(provider, &presentation_lights(step));
        if completed {
            LIGHT_STEPS.fetch_add(1, Ordering::AcqRel);
            watchdog_delay(watchdog, PRESENTATION_LIGHT_STEP_MS).await;
        }
        step += 1;
    }

    let lights_off = write_exact(provider, &PRESENTATION_LIGHTS_OFF);
    LIGHTS_OFF_SENT.store(lights_off, Ordering::Release);
    let safe = write_exact(provider, &PRESENTATION_SAFE);
    SAFE_SENT.store(safe, Ordering::Release);
    watchdog_delay(watchdog, START_SETTLE_MS).await;

    completed
        && LIGHT_STEPS.load(Ordering::Acquire) == PRESENTATION_LIGHT_STEPS
        && lights_off
        && safe
}
