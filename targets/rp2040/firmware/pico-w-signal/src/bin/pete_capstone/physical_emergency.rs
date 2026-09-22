//! Dedicated Pete physical emergency switch below ordinary Play interpretation.

use core::fmt::Write as _;

use embassy_rp::gpio::Input;
use embassy_rp::peripherals::PIN_22;
use embassy_rp::Peri;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use portable_atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

use crate::{create_link_gate, create_play, send_control_frame, InertCdc, BOOTSEL_FRAME_MAX};

const ARM_PREFIX: &str = "CONDUIT_PHYSICAL_EMERGENCY_ARM@1:";
const QUERY_PREFIX: &str = "CONDUIT_PHYSICAL_EMERGENCY_QUERY@1:";
const SCHEMA: &str = "conduit.pete/physical-emergency-receipt@1";
const PROVIDER: &str = "rp2040/gpio22-active-low@1";
const DIGEST_BYTES: usize = 64;
const DEBOUNCE_MS: u64 = 30;

static ARMED: AtomicBool = AtomicBool::new(false);
static TRIGGERED: AtomicBool = AtomicBool::new(false);
static OBSERVED_AT_MILLIS: AtomicU32 = AtomicU32::new(0);
static BINDING: [AtomicU8; DIGEST_BYTES] = [const { AtomicU8::new(0) }; DIGEST_BYTES];

fn request_suffix<'a>(request: &'a [u8], prefix: &str) -> Option<&'a [u8]> {
    request.strip_prefix(prefix.as_bytes())
}

fn build_bound_suffix<'a>(request: &'a [u8], prefix: &str) -> Option<&'a [u8]> {
    let suffix = request_suffix(request, prefix)?;
    let build = env!("CONDUIT_PETE_CAPSTONE_BUILD_ID").as_bytes();
    suffix.strip_prefix(build)
}

pub fn arm_request_matches(request: &[u8]) -> bool {
    let Some(suffix) = build_bound_suffix(request, ARM_PREFIX) else {
        return false;
    };
    suffix.len() == DIGEST_BYTES + 1
        && suffix[0] == b':'
        && suffix[1..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

pub fn query_request_matches(request: &[u8]) -> bool {
    matches!(build_bound_suffix(request, QUERY_PREFIX), Some([]))
}

pub fn arm(request: &[u8]) -> bool {
    if !arm_request_matches(request) || TRIGGERED.load(Ordering::Acquire) {
        return false;
    }
    let digest = &request[request.len() - DIGEST_BYTES..];
    for (slot, byte) in BINDING.iter().zip(digest) {
        slot.store(*byte, Ordering::Relaxed);
    }
    ARMED.store(true, Ordering::Release);
    true
}

fn binding() -> String<DIGEST_BYTES> {
    let mut value = String::new();
    if ARMED.load(Ordering::Acquire) {
        for slot in &BINDING {
            let _ = value.push(char::from(slot.load(Ordering::Relaxed)));
        }
    }
    value
}

pub async fn serve_receipt(class: &mut InertCdc) {
    let mut receipt = String::<BOOTSEL_FRAME_MAX>::new();
    let triggered = TRIGGERED.load(Ordering::Acquire);
    let _ = write!(
        receipt,
        "{{\"schema\":\"{}\",\"build_id\":\"{}\",\"binding_sha256\":\"{}\",\"provider_id\":\"{}\",\"gpio\":22,\"active_low\":true,\"triggered\":{},\"observed_at_millis\":{},\"translator_disabled\":{},\"play_preempted\":{}}}",
        SCHEMA,
        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
        binding(),
        PROVIDER,
        triggered,
        OBSERVED_AT_MILLIS.load(Ordering::Acquire),
        create_link_gate::emergency_latched() && !create_link_gate::translator_enabled(),
        triggered && create_play::snapshot().state == create_play::RequestState::Preempted,
    );
    let _ = send_control_frame(class, receipt.as_bytes()).await;
}

#[embassy_executor::task]
pub async fn task(pin: Peri<'static, PIN_22>) {
    let mut input = Input::new(pin, embassy_rp::gpio::Pull::Up);
    loop {
        input.wait_for_low().await;
        Timer::after(Duration::from_millis(DEBOUNCE_MS)).await;
        if ARMED.load(Ordering::Acquire)
            && input.is_low()
            && TRIGGERED
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            // Disable the electrical authority boundary before touching any
            // ordinary request bookkeeping.
            create_link_gate::latch_physical_emergency();
            create_play::preempt_for_physical_emergency();
            OBSERVED_AT_MILLIS.store(
                (Instant::now().as_millis() as u32).max(1),
                Ordering::Release,
            );
        }
        input.wait_for_high().await;
    }
}
