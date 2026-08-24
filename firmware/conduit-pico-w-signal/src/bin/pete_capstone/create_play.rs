//! Build-bound Conduit Play entrance and finite physical receipt state.

use core::fmt::Write as _;

use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use portable_atomic::{AtomicI32, AtomicU32, AtomicU8, Ordering};

use crate::{send_control_frame, InertCdc, BOOTSEL_FRAME_MAX};

pub const SPEED_MM_S: i16 = 50;
pub const TTL_MS: u32 = 250;
pub const AUTHORITY_GRANT: &str = "grant/pete-pico-wheels-off-floor-hil";
const CAPSTONE_FORM: &str = "pete-capstone";
const MOTION_REQUEST_PREFIX: &str = "CONDUIT_PLAY@1:";
const HELLO_REQUEST_PREFIX: &str = "CONDUIT_CREATE_HELLO@1:";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RequestKind {
    None = 0,
    Motion = 1,
    Hello = 2,
    Presentation = 3,
    FullStage = 4,
    LightsStage = 5,
}

impl RequestKind {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Motion,
            2 => Self::Hello,
            3 => Self::Presentation,
            4 => Self::FullStage,
            5 => Self::LightsStage,
            _ => Self::None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RequestState {
    Idle = 0,
    Preparing = 1,
    Pending = 2,
    Active = 3,
    Withdrawal = 4,
    Completed = 5,
    Refused = 6,
    Preempted = 7,
}

impl RequestState {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Preparing => "preparing",
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Withdrawal => "withdrawal",
            Self::Completed => "completed",
            Self::Refused => "refused",
            Self::Preempted => "preempted",
        }
    }

    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Preparing,
            2 => Self::Pending,
            3 => Self::Active,
            4 => Self::Withdrawal,
            5 => Self::Completed,
            6 => Self::Refused,
            7 => Self::Preempted,
            _ => Self::Idle,
        }
    }

    pub const fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Refused | Self::Preempted)
    }
}

#[derive(Clone, Copy)]
pub struct RequestSnapshot {
    pub generation: u32,
    pub state: RequestState,
    pub result_code: u8,
    pub safety_generation: u32,
    pub deadline_ms: u32,
    pub selected_linear_microunits: i32,
    pub kernel_decisions: u32,
    pub kernel_signs: u32,
}

static NEXT_GENERATION: AtomicU32 = AtomicU32::new(0);
static REQUEST_GENERATION: AtomicU32 = AtomicU32::new(0);
static REQUEST_STATE: AtomicU8 = AtomicU8::new(RequestState::Idle as u8);
static REQUEST_KIND: AtomicU8 = AtomicU8::new(RequestKind::None as u8);
static RESULT_CODE: AtomicU8 = AtomicU8::new(0);
static RESULT_SAFETY_GENERATION: AtomicU32 = AtomicU32::new(0);
static RESULT_DEADLINE_MS: AtomicU32 = AtomicU32::new(0);
static SELECTED_LINEAR_MICROUNITS: AtomicI32 = AtomicI32::new(0);
static KERNEL_DECISIONS: AtomicU32 = AtomicU32::new(0);
static KERNEL_SIGNS: AtomicU32 = AtomicU32::new(0);

pub fn motion_request_matches(request: &[u8]) -> bool {
    let mut expected = String::<BOOTSEL_FRAME_MAX>::new();
    write!(
        expected,
        "{MOTION_REQUEST_PREFIX}{}:{CAPSTONE_FORM}:{AUTHORITY_GRANT}",
        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID")
    )
    .is_ok()
        && request == expected.as_bytes()
}

pub fn hello_request_matches(request: &[u8]) -> bool {
    let mut expected = String::<BOOTSEL_FRAME_MAX>::new();
    write!(
        expected,
        "{HELLO_REQUEST_PREFIX}{}",
        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID")
    )
    .is_ok()
        && request == expected.as_bytes()
}

pub async fn serve_motion(class: &mut InertCdc) {
    let mut response = String::<BOOTSEL_FRAME_MAX>::new();
    match submit(RequestKind::Motion) {
        Ok(generation) => {
            let deadline = Instant::now() + Duration::from_millis(2_000);
            loop {
                let motion = snapshot();
                if motion.generation == generation && motion.state.terminal() {
                    let success = motion.state == RequestState::Completed;
                    let _ = write!(
                        response,
                        concat!(
                            "{{\"schema\":\"conduit.play/physical-receipt@1\",",
                            "\"build_id\":\"{}\",\"success\":{},\"generation\":{},",
                            "\"state\":\"{}\",\"result_code\":{},",
                            "\"form\":\"pete-capstone\",",
                            "\"kernel\":\"conduit-kernel\",\"oi_exposed\":false,",
                            "\"selected_linear_microunits\":{},",
                            "\"linear_mm_s\":{},\"angular_mrad_s\":0,\"ttl_ms\":250,",
                            "\"safety_generation\":{},\"deadline_ms\":{},",
                            "\"kernel_decisions\":{},\"kernel_signs\":{},",
                            "\"authority_grant_id\":\"{}\",",
                            "\"setup\":\"wheels-off-floor\",\"final_zero_confirmed\":{}}}"
                        ),
                        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
                        success,
                        generation,
                        motion.state.name(),
                        motion.result_code,
                        motion.selected_linear_microunits,
                        if motion.selected_linear_microunits == 0 { 0 } else { SPEED_MM_S },
                        motion.safety_generation,
                        motion.deadline_ms,
                        motion.kernel_decisions,
                        motion.kernel_signs,
                        AUTHORITY_GRANT,
                        success,
                    );
                    let _ = send_control_frame(class, response.as_bytes()).await;
                    release(generation);
                    break;
                }
                if Instant::now() >= deadline {
                    timeout(generation);
                    let motion = snapshot();
                    let _ = write!(
                        response,
                        "{{\"schema\":\"conduit.play/physical-receipt@1\",\"build_id\":\"{}\",\"success\":false,\"generation\":{},\"state\":\"{}\",\"result_code\":{},\"form\":\"pete-capstone\",\"oi_exposed\":false,\"setup\":\"wheels-off-floor\",\"final_zero_confirmed\":false}}",
                        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"), generation,
                        motion.state.name(), motion.result_code,
                    );
                    let _ = send_control_frame(class, response.as_bytes()).await;
                    release(generation);
                    break;
                }
                Timer::after(Duration::from_millis(5)).await;
            }
        }
        Err(()) => {
            let _ = write!(
                response,
                "{{\"schema\":\"conduit.play/physical-receipt@1\",\"build_id\":\"{}\",\"success\":false,\"state\":\"busy\",\"result_code\":8,\"form\":\"pete-capstone\",\"oi_exposed\":false,\"setup\":\"wheels-off-floor\",\"final_zero_confirmed\":false}}",
                env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
            );
            let _ = send_control_frame(class, response.as_bytes()).await;
        }
    }
}

pub async fn serve_hello(class: &mut InertCdc) {
    let mut response = String::<BOOTSEL_FRAME_MAX>::new();
    match submit(RequestKind::Hello) {
        Ok(generation) => {
            let deadline = Instant::now() + Duration::from_millis(10_000);
            loop {
                let request = snapshot();
                if request.generation == generation && request.state.terminal() {
                    let success = request.state == RequestState::Completed;
                    let cue_sent = crate::create_acquisition::ready_cue_command_sent();
                    let _ = write!(
                        response,
                        "{{\"schema\":\"conduit.pete/create-hello-receipt@1\",\"build_id\":\"{}\",\"success\":{},\"generation\":{},\"state\":\"{}\",\"result_code\":{},\"observed_oi_mode\":\"{}\",\"final_oi_mode\":\"{}\",\"ready_cue_command_sent\":{},\"motion_authority_granted\":false}}",
                        env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
                        success,
                        generation,
                        request.state.name(),
                        request.result_code,
                        if cue_sent { "full" } else { "unknown" },
                        if success { "safe" } else { "unknown" },
                        cue_sent,
                    );
                    let _ = send_control_frame(class, response.as_bytes()).await;
                    release(generation);
                    break;
                }
                if Instant::now() >= deadline {
                    timeout(generation);
                    continue;
                }
                Timer::after(Duration::from_millis(5)).await;
            }
        }
        Err(()) => {
            let _ = write!(
                response,
                "{{\"schema\":\"conduit.pete/create-hello-receipt@1\",\"build_id\":\"{}\",\"success\":false,\"state\":\"busy\",\"result_code\":8,\"motion_authority_granted\":false}}",
                env!("CONDUIT_PETE_CAPSTONE_BUILD_ID"),
            );
            let _ = send_control_frame(class, response.as_bytes()).await;
        }
    }
}

pub fn submit(kind: RequestKind) -> Result<u32, ()> {
    REQUEST_STATE
        .compare_exchange(RequestState::Idle as u8, RequestState::Preparing as u8, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| ())?;
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed).wrapping_add(1).max(1);
    REQUEST_GENERATION.store(generation, Ordering::Release);
    REQUEST_KIND.store(kind as u8, Ordering::Release);
    RESULT_CODE.store(0, Ordering::Release);
    RESULT_SAFETY_GENERATION.store(0, Ordering::Release);
    RESULT_DEADLINE_MS.store(0, Ordering::Release);
    SELECTED_LINEAR_MICROUNITS.store(0, Ordering::Release);
    KERNEL_DECISIONS.store(0, Ordering::Release);
    KERNEL_SIGNS.store(0, Ordering::Release);
    set_state(RequestState::Pending);
    Ok(generation)
}

pub fn request_kind() -> RequestKind {
    RequestKind::from_raw(REQUEST_KIND.load(Ordering::Acquire))
}

pub fn snapshot() -> RequestSnapshot {
    RequestSnapshot {
        generation: REQUEST_GENERATION.load(Ordering::Acquire),
        state: RequestState::from_raw(REQUEST_STATE.load(Ordering::Acquire)),
        result_code: RESULT_CODE.load(Ordering::Acquire),
        safety_generation: RESULT_SAFETY_GENERATION.load(Ordering::Acquire),
        deadline_ms: RESULT_DEADLINE_MS.load(Ordering::Acquire),
        selected_linear_microunits: SELECTED_LINEAR_MICROUNITS.load(Ordering::Acquire),
        kernel_decisions: KERNEL_DECISIONS.load(Ordering::Acquire),
        kernel_signs: KERNEL_SIGNS.load(Ordering::Acquire),
    }
}

pub fn claim_pending(kind: RequestKind) -> bool {
    request_kind() == kind
        && REQUEST_STATE.compare_exchange(RequestState::Pending as u8, RequestState::Preparing as u8, Ordering::AcqRel, Ordering::Acquire).is_ok()
}

pub fn set_state(state: RequestState) { REQUEST_STATE.store(state as u8, Ordering::Release); }
pub fn set_result(code: u8) { RESULT_CODE.store(code, Ordering::Release); }
pub fn set_safety_generation(value: u32) { RESULT_SAFETY_GENERATION.store(value, Ordering::Release); }
pub fn set_deadline(value: u32) { RESULT_DEADLINE_MS.store(value, Ordering::Release); }
pub fn set_selected(value: i32) { SELECTED_LINEAR_MICROUNITS.store(value, Ordering::Release); }
pub fn set_kernel_metrics(decisions: u32, signs: u32) {
    KERNEL_DECISIONS.store(decisions, Ordering::Release);
    KERNEL_SIGNS.store(signs, Ordering::Release);
}

pub(crate) fn release(generation: u32) {
    if REQUEST_GENERATION.load(Ordering::Acquire) == generation && snapshot().state.terminal() {
        set_state(RequestState::Idle);
        REQUEST_KIND.store(RequestKind::None as u8, Ordering::Release);
    }
}

pub(crate) fn timeout(generation: u32) {
    if REQUEST_GENERATION.load(Ordering::Acquire) == generation
        && matches!(snapshot().state, RequestState::Preparing | RequestState::Pending)
    {
        set_result(6);
        set_state(RequestState::Refused);
    }
}
