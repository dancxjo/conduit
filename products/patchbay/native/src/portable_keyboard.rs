//! Bounded native-window entrance at the portable key-event seam.
mod button;
pub use button::append_offers as append_button_offers;

use conduit_core::{
    resource_offer, resource_requirement, ArtifactId, CapabilityId, CapabilityOffer,
    ExecutionProfileId, HostAdvertisement, ImplementationId, ResourceOffer, INPUT_RESOURCE_CLASS,
};
use conduit_human::{KeyEvent, KeyModifiers, KeyTransition};
use std::sync::{Arc, Mutex};
use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

pub const NATIVE_KEYBOARD_IMPLEMENTATION: &str = conduit_std_offers::HOSTED_KEYBOARD_IMPLEMENTATION;
pub const NATIVE_KEYBOARD_PROFILE: &str = conduit_std_offers::HOSTED_KEYBOARD_EXECUTION_PROFILE;
pub const NATIVE_KEYBOARD_ARTIFACT: &str = "patchbay-native/winit-physical-key-adapter@1";
pub const WINDOW_INPUT_RESOURCE: &str = "patchbay-native.resource/window-input-base@1";
pub const EVENT_QUEUE_RESOURCE: &str = "patchbay-native.resource/key-event-slot@1";
pub const OPERATION_RESOURCE: &str = "patchbay-native.resource/input-operation-slot@1";
pub const EVENT_CAPACITY: usize = conduit_semantic_catalog::KEYBOARD_MAX_QUEUE_ITEMS as usize;
const HELD_CAPACITY: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeKeyboardFailure {
    UnidentifiedPhysicalKey,
    UnsupportedPhysicalKey,
    RepeatedPlatformEvent,
    DuplicatePress,
    ReleaseWithoutPress,
    QueuePressure,
    FocusLost,
    Cancelled,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    Available,
    Failed(NativeKeyboardFailure),
    #[cfg_attr(not(test), allow(dead_code))]
    Cancelled,
    Closed,
}

/// Fixed native input state. `PhysicalKey::Code` supplies physical identity;
/// localized `Key`, text, window handles, and timestamps never enter values.
struct NativeKeyboardState {
    values: [Option<KeyEvent>; EVENT_CAPACITY],
    read: usize,
    len: usize,
    held: [Option<u8>; HELD_CAPACITY],
    modifiers: u8,
    lifecycle: Lifecycle,
}

pub struct NativeKeyboardInput {
    state: Arc<Mutex<NativeKeyboardState>>,
}

#[derive(Clone)]
pub struct NativeKeyboardReader {
    state: Arc<Mutex<NativeKeyboardState>>,
}

impl Default for NativeKeyboardInput {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeKeyboardInput {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(NativeKeyboardState {
                values: [None; EVENT_CAPACITY],
                read: 0,
                len: 0,
                held: [None; HELD_CAPACITY],
                modifiers: 0,
                lifecycle: Lifecycle::Available,
            })),
        }
    }

    pub fn reader(&self) -> NativeKeyboardReader {
        NativeKeyboardReader {
            state: Arc::clone(&self.state),
        }
    }

    pub fn observe(
        &mut self,
        physical_key: PhysicalKey,
        state: ElementState,
        repeat: bool,
    ) -> Result<KeyEvent, NativeKeyboardFailure> {
        let mut keyboard = self
            .state
            .lock()
            .expect("native keyboard state is not poisoned");
        require_available(keyboard.lifecycle)?;
        if repeat {
            return Err(NativeKeyboardFailure::RepeatedPlatformEvent);
        }
        let PhysicalKey::Code(code) = physical_key else {
            return Err(NativeKeyboardFailure::UnidentifiedPhysicalKey);
        };
        let Some(usage) = usage_for(code) else {
            return Err(NativeKeyboardFailure::UnsupportedPhysicalKey);
        };
        if keyboard.len == EVENT_CAPACITY {
            keyboard.lifecycle = Lifecycle::Failed(NativeKeyboardFailure::QueuePressure);
            return Err(NativeKeyboardFailure::QueuePressure);
        }
        let transition = match state {
            ElementState::Pressed => {
                if keyboard.held.contains(&Some(usage)) {
                    return Err(NativeKeyboardFailure::DuplicatePress);
                }
                let Some(slot) = keyboard.held.iter_mut().find(|slot| slot.is_none()) else {
                    keyboard.lifecycle = Lifecycle::Failed(NativeKeyboardFailure::QueuePressure);
                    return Err(NativeKeyboardFailure::QueuePressure);
                };
                *slot = Some(usage);
                KeyTransition::Pressed
            }
            ElementState::Released => {
                let Some(slot) = keyboard.held.iter_mut().find(|slot| **slot == Some(usage)) else {
                    return Err(NativeKeyboardFailure::ReleaseWithoutPress);
                };
                *slot = None;
                KeyTransition::Released
            }
        };
        if let Some(bit) = modifier_bit(usage) {
            match transition {
                KeyTransition::Pressed => keyboard.modifiers |= bit,
                KeyTransition::Released => keyboard.modifiers &= !bit,
            }
        }
        let value = KeyEvent::new(
            usage,
            transition,
            KeyModifiers::from_bits(keyboard.modifiers),
        )
        .expect("native usage and after-transition modifier state are canonical");
        let write = (keyboard.read + keyboard.len) % EVENT_CAPACITY;
        keyboard.values[write] = Some(value);
        keyboard.len += 1;
        Ok(value)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn next(&mut self) -> Result<Option<KeyEvent>, NativeKeyboardFailure> {
        next_from(&self.state)
    }

    pub fn focus_lost(&mut self) {
        let mut state = self
            .state
            .lock()
            .expect("native keyboard state is not poisoned");
        if matches!(state.lifecycle, Lifecycle::Available) {
            state.values = [None; EVENT_CAPACITY];
            state.read = 0;
            state.len = 0;
            state.held = [None; HELD_CAPACITY];
            state.modifiers = 0;
            state.lifecycle = Lifecycle::Failed(NativeKeyboardFailure::FocusLost);
        }
    }

    pub fn focus_gained(&mut self) {
        let mut state = self
            .state
            .lock()
            .expect("native keyboard state is not poisoned");
        if matches!(
            state.lifecycle,
            Lifecycle::Failed(NativeKeyboardFailure::FocusLost)
        ) {
            state.lifecycle = Lifecycle::Available;
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn cancel(&mut self) {
        let mut state = self
            .state
            .lock()
            .expect("native keyboard state is not poisoned");
        state.values = [None; EVENT_CAPACITY];
        state.len = 0;
        state.lifecycle = Lifecycle::Cancelled;
    }

    pub fn close(&mut self) {
        self.state
            .lock()
            .expect("native keyboard state is not poisoned")
            .lifecycle = Lifecycle::Closed;
    }
}

fn require_available(lifecycle: Lifecycle) -> Result<(), NativeKeyboardFailure> {
    match lifecycle {
        Lifecycle::Available => Ok(()),
        Lifecycle::Failed(failure) => Err(failure),
        Lifecycle::Cancelled => Err(NativeKeyboardFailure::Cancelled),
        Lifecycle::Closed => Err(NativeKeyboardFailure::Closed),
    }
}

fn next_from(
    state: &Arc<Mutex<NativeKeyboardState>>,
) -> Result<Option<KeyEvent>, NativeKeyboardFailure> {
    let mut state = state.lock().expect("native keyboard state is not poisoned");
    require_available(state.lifecycle)?;
    if state.len == 0 {
        return Ok(None);
    }
    let read = state.read;
    let value = state.values[read].take();
    state.read = (read + 1) % EVENT_CAPACITY;
    state.len -= 1;
    Ok(value)
}

impl conduit_std_host::hosted_keyboard::HostedKeyboardAdapter for NativeKeyboardReader {
    fn poll_next(&mut self) -> conduit_std_host::hosted_keyboard::HostedKeyboardPoll {
        use conduit_std_host::hosted_keyboard::HostedKeyboardPoll;
        match next_from(&self.state) {
            Ok(Some(event)) => HostedKeyboardPoll::Event(event),
            Ok(None) => HostedKeyboardPoll::Pending,
            Err(NativeKeyboardFailure::Cancelled | NativeKeyboardFailure::Closed) => {
                HostedKeyboardPoll::Cancelled
            }
            Err(failure) => HostedKeyboardPoll::Failed(failure as u16),
        }
    }
}

pub fn append_offer(advertisement: &mut HostAdvertisement) -> Result<(), String> {
    if advertisement.capabilities.iter().any(|offer| {
        offer.kind_id.as_str() == conduit_semantic_catalog::KEYBOARD_KIND
            || offer.implementation.implementation_id.as_str() == NATIVE_KEYBOARD_IMPLEMENTATION
    }) {
        return Err("native keyboard offer duplicates an existing implementation".into());
    }
    let base = format!("native-window-input/{}", advertisement.boot_id.as_str());
    let resources = [
        resource_offer(&base, WINDOW_INPUT_RESOURCE, 1),
        resource_offer(
            &format!("{base}/events"),
            EVENT_QUEUE_RESOURCE,
            EVENT_CAPACITY as u32,
        ),
        resource_offer(&format!("{base}/operation"), OPERATION_RESOURCE, 1),
        resource_offer(&format!("{base}/input"), INPUT_RESOURCE_CLASS, 1),
    ];
    append_resources(&mut advertisement.resources, resources)?;
    let contract = conduit_semantic_catalog::keyboard_contract();
    let mut requirements = vec![
        resource_requirement(INPUT_RESOURCE_CLASS, 1),
        resource_requirement(WINDOW_INPUT_RESOURCE, 1),
        resource_requirement(EVENT_QUEUE_RESOURCE, EVENT_CAPACITY as u32),
        resource_requirement(OPERATION_RESOURCE, 1),
    ];
    requirements.sort();
    advertisement.capabilities.push(CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("patchbay-native/input-keyboard@1"),
        kind_id: contract.kind_id,
        kind_contract_revision: conduit_semantic_catalog::keyboard_contract_revision(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(NATIVE_KEYBOARD_PROFILE),
            implementation_id: ImplementationId::from(NATIVE_KEYBOARD_IMPLEMENTATION),
            artifact_id: ArtifactId::from(NATIVE_KEYBOARD_ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![conduit_std_offers::next_key_event_host_operation_requirement()],
        resource_requirements: requirements,
        authority_requirements: Vec::new(),
        limits: contract.limits,
    });
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    Ok(())
}

fn append_resources<const N: usize>(
    destination: &mut Vec<ResourceOffer>,
    resources: [ResourceOffer; N],
) -> Result<(), String> {
    for resource in resources {
        if destination
            .iter()
            .any(|current| current.pool_id == resource.pool_id)
        {
            return Err(format!(
                "duplicate native input resource {}",
                resource.pool_id.as_str()
            ));
        }
        destination.push(resource);
    }
    destination.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    Ok(())
}

const fn modifier_bit(usage: u8) -> Option<u8> {
    if usage >= 0xe0 && usage <= 0xe7 {
        Some(1 << (usage - 0xe0))
    } else {
        None
    }
}

pub const fn usage_for(code: KeyCode) -> Option<u8> {
    Some(match code {
        KeyCode::KeyA => 0x04,
        KeyCode::KeyB => 0x05,
        KeyCode::KeyC => 0x06,
        KeyCode::KeyD => 0x07,
        KeyCode::KeyE => 0x08,
        KeyCode::KeyF => 0x09,
        KeyCode::KeyG => 0x0a,
        KeyCode::KeyH => 0x0b,
        KeyCode::KeyI => 0x0c,
        KeyCode::KeyJ => 0x0d,
        KeyCode::KeyK => 0x0e,
        KeyCode::KeyL => 0x0f,
        KeyCode::KeyM => 0x10,
        KeyCode::KeyN => 0x11,
        KeyCode::KeyO => 0x12,
        KeyCode::KeyP => 0x13,
        KeyCode::KeyQ => 0x14,
        KeyCode::KeyR => 0x15,
        KeyCode::KeyS => 0x16,
        KeyCode::KeyT => 0x17,
        KeyCode::KeyU => 0x18,
        KeyCode::KeyV => 0x19,
        KeyCode::KeyW => 0x1a,
        KeyCode::KeyX => 0x1b,
        KeyCode::KeyY => 0x1c,
        KeyCode::KeyZ => 0x1d,
        KeyCode::Digit1 => 0x1e,
        KeyCode::Digit2 => 0x1f,
        KeyCode::Digit3 => 0x20,
        KeyCode::Digit4 => 0x21,
        KeyCode::Digit5 => 0x22,
        KeyCode::Digit6 => 0x23,
        KeyCode::Digit7 => 0x24,
        KeyCode::Digit8 => 0x25,
        KeyCode::Digit9 => 0x26,
        KeyCode::Digit0 => 0x27,
        KeyCode::Enter => 0x28,
        KeyCode::Escape => 0x29,
        KeyCode::Space => 0x2c,
        KeyCode::Minus => 0x2d,
        KeyCode::Equal => 0x2e,
        KeyCode::BracketLeft => 0x2f,
        KeyCode::BracketRight => 0x30,
        KeyCode::Backslash => 0x31,
        KeyCode::Semicolon => 0x33,
        KeyCode::Quote => 0x34,
        KeyCode::Backquote => 0x35,
        KeyCode::Comma => 0x36,
        KeyCode::Period => 0x37,
        KeyCode::Slash => 0x38,
        KeyCode::ControlLeft => 0xe0,
        KeyCode::ShiftLeft => 0xe1,
        KeyCode::AltLeft => 0xe2,
        KeyCode::SuperLeft => 0xe3,
        KeyCode::ControlRight => 0xe4,
        KeyCode::ShiftRight => 0xe5,
        KeyCode::AltRight => 0xe6,
        KeyCode::SuperRight => 0xe7,
        _ => return None,
    })
}
