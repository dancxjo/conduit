use conduit_core::{
    kind_id, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConnectionProvider, HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId,
    ImplementationId, OfferGeneration, PlatformEffect, PROTOCOL_VERSION,
};
use conduit_planner::{default_placements, plan};
use conduit_runtime::HostRuntime;
use conduit_signal::{
    pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
    pulse_outputs, pulse_resource_requirements, show_contract_revision, show_execution_profile,
    show_host_operation_requirements, show_inputs, show_resource_requirements,
    signal_profile_catalog, signal_registry, signal_resource_offers, PULSE_KIND, SHOW_KIND,
};
use std::cell::RefCell;

const FRAME_CAPACITY: usize = 4_096;
const MAXIMUM_RECEIPTS: u32 = 16;
const EFFECT_NONE: i32 = 0;
const EFFECT_WAIT: i32 = 1;
const EFFECT_PRESENT: i32 = 2;
const STATUS_RUNNING: i32 = 0;
const STATUS_COMPLETE: i32 = 1;
const ERROR_NOT_STARTED: i32 = -1;
const ERROR_INVALID_HOST: i32 = -2;
const ERROR_START: i32 = -3;
const ERROR_NO_EFFECT: i32 = -4;
const ERROR_COMPLETION_SIZE: i32 = -5;
const ERROR_COMPLETION_IDENTITY: i32 = -6;
const ERROR_UNSUPPORTED_EFFECT: i32 = -7;
const ERROR_RECEIPT_CAPACITY: i32 = -8;

thread_local! {
    static SESSION: RefCell<Option<BrowserSession>> = const { RefCell::new(None) };
    static INPUT: RefCell<[u8; FRAME_CAPACITY]> = const { RefCell::new([0; FRAME_CAPACITY]) };
}

struct BrowserSession {
    runtime: HostRuntime,
    pending: Vec<PlatformEffect>,
    current: Option<PlatformEffect>,
    output: [u8; FRAME_CAPACITY],
    output_len: usize,
    expected_completion: [u8; FRAME_CAPACITY],
    expected_completion_len: usize,
    receipts: u32,
    complete: bool,
    error: i32,
}

impl BrowserSession {
    fn start(host_index: u32) -> Result<Self, i32> {
        let (host_id, boot_id) = match host_index {
            0 => ("browser-host-a", "browser-boot-a"),
            1 => ("browser-host-b", "browser-boot-b"),
            _ => return Err(ERROR_INVALID_HOST),
        };
        let advertisement = build_advertisement(host_id, boot_id);
        let registry = signal_registry(
            ImplementationId::from("browser/pulse-v1"),
            ImplementationId::from("browser/dom-show-signal-v1"),
        )
        .map_err(|_| ERROR_START)?;
        let mut runtime = HostRuntime::new(advertisement.clone(), registry, 256);
        let form = conduit_form::parse(
            include_str!("../../../examples/signal-demo.form"),
            &signal_profile_catalog(),
        )
        .map_err(|_| ERROR_START)?;
        let realm = [advertisement];
        let placements = default_placements(&form, &realm).map_err(|_| ERROR_START)?;
        let mut planned = plan(&form, &realm, &placements, &[ConnectionProvider::Local])
            .map_err(|_| ERROR_START)?;
        let fragment = planned.fragments.pop().ok_or(ERROR_START)?;
        let plan_id = fragment.plan_id.clone();
        let prepared = runtime.handle(HostCommand::Prepare(fragment));
        if prepared
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::PreparationRejected { .. }))
        {
            return Err(ERROR_START);
        }
        let activated = runtime.handle(HostCommand::Activate(plan_id));
        if activated
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::ActivationRejected { .. }))
        {
            return Err(ERROR_START);
        }
        let complete = activated
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::PlanCompleted { .. }));
        let mut session = Self {
            runtime,
            pending: activated.effects,
            current: None,
            output: [0; FRAME_CAPACITY],
            output_len: 0,
            expected_completion: [0; FRAME_CAPACITY],
            expected_completion_len: 0,
            receipts: 0,
            complete,
            error: STATUS_RUNNING,
        };
        session.advance()?;
        Ok(session)
    }

    fn advance(&mut self) -> Result<(), i32> {
        self.output_len = 0;
        self.expected_completion_len = 0;
        self.current = self.pending.pop();
        let Some(effect) = self.current.as_ref() else {
            if !self.complete {
                self.error = ERROR_NO_EFFECT;
                return Err(ERROR_NO_EFFECT);
            }
            return Ok(());
        };

        let mut output = FrameWriter::new(&mut self.output);
        let mut expected = FrameWriter::new(&mut self.expected_completion);
        match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                duration_ms,
            } => {
                output.byte(EFFECT_WAIT as u8)?;
                output.text(plan_id.as_str())?;
                output.text(placement_id.as_str())?;
                output.u64(*duration_ms)?;
                expected.byte(EFFECT_WAIT as u8)?;
                expected.text(plan_id.as_str())?;
                expected.text(placement_id.as_str())?;
            }
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                presentation_kind,
                value,
            } => {
                output.byte(EFFECT_PRESENT as u8)?;
                output.text(plan_id.as_str())?;
                output.text(active_play_id.as_str())?;
                output.text(presentation_id.as_str())?;
                output.text(placement_id.as_str())?;
                output.text(presentation_kind.as_str())?;
                output.text(value.value_kind.as_str())?;
                output.bytes(&value.encoded)?;
                expected.byte(EFFECT_PRESENT as u8)?;
                expected.text(plan_id.as_str())?;
                expected.text(active_play_id.as_str())?;
                expected.text(presentation_id.as_str())?;
                expected.text(placement_id.as_str())?;
                expected.text(value.value_kind.as_str())?;
                expected.bytes(&value.encoded)?;
            }
            PlatformEffect::TransmitConnection { .. } => {
                self.error = ERROR_UNSUPPORTED_EFFECT;
                return Err(ERROR_UNSUPPORTED_EFFECT);
            }
        }
        self.output_len = output.len();
        self.expected_completion_len = expected.len();
        Ok(())
    }

    fn complete_current(&mut self, completion: &[u8]) -> Result<(), i32> {
        if self.error < 0 || self.current.is_none() {
            return Err(ERROR_NO_EFFECT);
        }
        if completion != &self.expected_completion[..self.expected_completion_len] {
            self.error = ERROR_COMPLETION_IDENTITY;
            return Err(ERROR_COMPLETION_IDENTITY);
        }
        let effect = self.current.take().ok_or(ERROR_NO_EFFECT)?;
        let (follow_up, presented) = match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                ..
            } => (
                self.runtime.handle(HostCommand::CompleteWait {
                    plan_id,
                    placement_id,
                }),
                false,
            ),
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
                ..
            } => (
                self.runtime.handle(HostCommand::CompletePresentation {
                    plan_id,
                    active_play_id,
                    presentation_id,
                    placement_id,
                    value,
                    success: true,
                    message: None,
                }),
                true,
            ),
            PlatformEffect::TransmitConnection { .. } => {
                self.error = ERROR_UNSUPPORTED_EFFECT;
                return Err(ERROR_UNSUPPORTED_EFFECT);
            }
        };
        if presented {
            self.receipts = self
                .receipts
                .checked_add(1)
                .filter(|count| *count <= MAXIMUM_RECEIPTS)
                .ok_or_else(|| {
                    self.error = ERROR_RECEIPT_CAPACITY;
                    ERROR_RECEIPT_CAPACITY
                })?;
        }
        self.complete |= follow_up
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::PlanCompleted { .. }));
        self.pending.extend(follow_up.effects.into_iter().rev());
        self.advance().inspect_err(|code| {
            self.error = *code;
        })
    }

    fn status(&self) -> i32 {
        if self.error < 0 {
            self.error
        } else if self.complete && self.current.is_none() && self.pending.is_empty() {
            STATUS_COMPLETE
        } else {
            STATUS_RUNNING
        }
    }

    fn effect_kind(&self) -> i32 {
        match self.current {
            Some(PlatformEffect::Wait { .. }) => EFFECT_WAIT,
            Some(PlatformEffect::PresentValue { .. }) => EFFECT_PRESENT,
            Some(PlatformEffect::TransmitConnection { .. }) => ERROR_UNSUPPORTED_EFFECT,
            None => EFFECT_NONE,
        }
    }
}

struct FrameWriter<'a> {
    target: &'a mut [u8],
    offset: usize,
}

impl<'a> FrameWriter<'a> {
    fn new(target: &'a mut [u8]) -> Self {
        Self { target, offset: 0 }
    }

    fn len(&self) -> usize {
        self.offset
    }

    fn byte(&mut self, value: u8) -> Result<(), i32> {
        self.write(&[value])
    }

    fn u64(&mut self, value: u64) -> Result<(), i32> {
        self.write(&value.to_le_bytes())
    }

    fn text(&mut self, value: &str) -> Result<(), i32> {
        self.bytes(value.as_bytes())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), i32> {
        let length = u16::try_from(value.len()).map_err(|_| ERROR_COMPLETION_SIZE)?;
        self.write(&length.to_le_bytes())?;
        self.write(value)
    }

    fn write(&mut self, value: &[u8]) -> Result<(), i32> {
        let end = self
            .offset
            .checked_add(value.len())
            .filter(|end| *end <= self.target.len())
            .ok_or(ERROR_COMPLETION_SIZE)?;
        self.target[self.offset..end].copy_from_slice(value);
        self.offset = end;
        Ok(())
    }
}

fn build_advertisement(host_id: &str, boot_id: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser-wasm"),
        resources: signal_resource_offers("browser/timer", "browser/dom", 16),
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from("pulse-1"),
                kind_id: kind_id(PULSE_KIND),
                kind_contract_revision: pulse_contract_revision(),
                execution_profile_id: pulse_execution_profile(),
                implementation_id: ImplementationId::from("browser/pulse-v1"),
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
                inputs: vec![],
                outputs: pulse_outputs(),
                host_operations: pulse_host_operation_requirements(),
                resource_requirements: pulse_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from("dom-show-1"),
                kind_id: kind_id(SHOW_KIND),
                kind_contract_revision: show_contract_revision(),
                execution_profile_id: show_execution_profile(),
                implementation_id: ImplementationId::from("browser/dom-show-signal-v1"),
                artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
                inputs: show_inputs(),
                outputs: vec![],
                host_operations: show_host_operation_requirements(),
                resource_requirements: show_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
        ],
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_start(host_index: u32) -> i32 {
    match BrowserSession::start(host_index) {
        Ok(session) => {
            SESSION.with(|slot| *slot.borrow_mut() = Some(session));
            STATUS_RUNNING
        }
        Err(code) => code,
    }
}

#[no_mangle]
pub extern "C" fn conduit_browser_status() -> i32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(BrowserSession::status)
            .unwrap_or(ERROR_NOT_STARTED)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_effect_kind() -> i32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(BrowserSession::effect_kind)
            .unwrap_or(ERROR_NOT_STARTED)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_output_ptr() -> *const u8 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.output.as_ptr())
            .unwrap_or(std::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_output_len() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.output_len as u32)
            .unwrap_or(0)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_input_ptr() -> *mut u8 {
    INPUT.with(|input| input.borrow_mut().as_mut_ptr())
}

#[no_mangle]
pub extern "C" fn conduit_browser_input_capacity() -> u32 {
    FRAME_CAPACITY as u32
}

#[no_mangle]
pub extern "C" fn conduit_browser_complete(completion_len: u32) -> i32 {
    let completion_len = completion_len as usize;
    if completion_len > FRAME_CAPACITY {
        return ERROR_COMPLETION_SIZE;
    }
    INPUT.with(|input| {
        SESSION.with(|slot| {
            let input = input.borrow();
            let mut slot = slot.borrow_mut();
            let Some(session) = slot.as_mut() else {
                return ERROR_NOT_STARTED;
            };
            match session.complete_current(&input[..completion_len]) {
                Ok(()) => session.status(),
                Err(code) => code,
            }
        })
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_receipt_count() -> u32 {
    SESSION.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|session| session.receipts)
            .unwrap_or(0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_completion_frame_advances_and_mutated_frame_fails_closed() {
        let mut session = BrowserSession::start(0).expect("browser session starts");
        assert_eq!(session.effect_kind(), EFFECT_PRESENT);
        let exact = session.expected_completion[..session.expected_completion_len].to_vec();
        session
            .complete_current(&exact)
            .expect("exact frame advances");
        assert_eq!(session.receipts, 1);
        assert_eq!(session.effect_kind(), EFFECT_WAIT);
        let mut changed = session.expected_completion[..session.expected_completion_len].to_vec();
        changed[1] ^= 1;
        assert_eq!(
            session.complete_current(&changed),
            Err(ERROR_COMPLETION_IDENTITY)
        );
        assert_eq!(session.status(), ERROR_COMPLETION_IDENTITY);
    }

    #[test]
    fn host_identity_is_bounded_to_the_two_page_instances() {
        assert!(BrowserSession::start(0).is_ok());
        assert!(BrowserSession::start(1).is_ok());
        assert!(matches!(BrowserSession::start(2), Err(ERROR_INVALID_HOST)));
    }
}
