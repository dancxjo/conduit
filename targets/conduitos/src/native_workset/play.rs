//! Native Host adapter around the one fixed production scheduler.
mod effects;
mod preparation;
#[cfg(test)]
mod tests;

use super::{PreparedNativeWorkset, WorksetRefusal};
use crate::keyboard_text_operations::PlannedOperation;
use conduit_human::{ConduitIntlKeymap, KeyEvent, KeyTransition};
use conduit_kernel::{
    FixedSignLog, FixedValueStore, KernelEvent, NodeId,
    scheduler::{FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus},
};
use conduit_semantic_catalog::BoundedTextState;

const FORMS: usize = 2;
const NODES: usize = 8;
const CORDS: usize = 6;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const SIGN_ITEMS: usize = 768;
type Scheduler = FixedScheduler<
    OperationDriver<PlannedOperation, PORTS>,
    FixedValueStore<64, 256>,
    FixedSignLog<SIGN_ITEMS>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    { NODES * PORTS },
    CORDS,
    NODES,
    NODES,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Effect {
    Keyboard,
    Keymap,
    Upper,
    Edit,
    Presentation,
}
#[derive(Clone, Copy)]
struct Binding {
    form: u8,
    effect: Effect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayRefusal {
    Preparation,
    Kernel,
    Scheduler(conduit_kernel::scheduler::SchedulerError),
    HostFailure(conduit_kernel::Failure),
    Foreground,
    InputPressure,
    WorkBound,
    Cancelled,
}
impl PlayRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preparation => "native-body-play-preparation-refused",
            Self::Kernel => "native-body-kernel-boundary-refused",
            Self::Scheduler(_) => "native-body-kernel-operation-refused",
            Self::HostFailure(failure) => failure.code.as_str(),
            Self::Foreground => "native-body-foreground-unavailable",
            Self::InputPressure => "native-body-input-pressure",
            Self::WorkBound => "native-body-work-bound-exceeded",
            Self::Cancelled => "native-body-play-cancelled",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePresentation {
    bytes: [u8; 256],
    len: u16,
}
impl NativePresentation {
    pub fn text(&self) -> &str {
        core::str::from_utf8(&self.bytes[..usize::from(self.len)]).expect("validated presentation")
    }
    fn new(bytes: &[u8]) -> Result<Self, PlayRefusal> {
        if bytes.len() > 256 || core::str::from_utf8(bytes).is_err() {
            return Err(PlayRefusal::Kernel);
        }
        let mut result = Self {
            bytes: [0; 256],
            len: bytes.len() as u16,
        };
        result.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(result)
    }
}

pub struct NativeWorksetPlay {
    scheduler: Scheduler,
    bindings: [Option<Binding>; NODES],
    keymaps: [ConduitIntlKeymap; FORMS],
    editors: [Option<BoundedTextState>; FORMS],
    pending: [Option<HostOperationRequest>; FORMS],
    held: [Option<u8>; 256],
    presentations: [Option<NativePresentation>; FORMS],
    form_count: usize,
    cancelled: bool,
}

impl NativeWorksetPlay {
    #[cfg(test)]
    pub(crate) fn pending_requests(&self) -> [Option<HostOperationRequest>; FORMS] {
        self.pending
    }
    pub fn prepare(prepared: &PreparedNativeWorkset) -> Result<Self, WorksetRefusal> {
        preparation::prepare(prepared)
    }
    pub fn start(&mut self) -> Result<(), PlayRefusal> {
        self.drive()
    }
    pub fn sign_retention_gap(&self) -> Option<conduit_kernel::SignRetentionGap> {
        conduit_kernel::SignQuery::retention_gap(self.scheduler.signs())
    }
    pub fn take_presentation(&mut self, form: usize) -> Option<NativePresentation> {
        self.presentations.get_mut(form)?.take()
    }
    /// Foreground is supplied by the authoritative workspace selection. A held
    /// key keeps its original owner across subsequent selection changes.
    pub fn input(&mut self, foreground: usize, event: KeyEvent) -> Result<bool, PlayRefusal> {
        if self.cancelled {
            return Err(PlayRefusal::Cancelled);
        }
        if foreground >= self.form_count {
            return Err(PlayRefusal::Foreground);
        }
        let usage = usize::from(event.usage());
        let owner = match (self.held[usage], event.transition()) {
            (Some(owner), _) => usize::from(owner),
            (None, KeyTransition::Pressed) => foreground,
            (None, KeyTransition::Released) => return Ok(false),
        };
        let request = self.pending[owner].ok_or(PlayRefusal::InputPressure)?;
        if self.presentations[owner].is_some() {
            return Err(PlayRefusal::InputPressure);
        }
        self.output(request, Some(&event.encode()))?;
        self.pending[owner] = None;
        self.held[usage] =
            matches!(event.transition(), KeyTransition::Pressed).then_some(owner as u8);
        self.drive()?;
        Ok(true)
    }
    pub fn cancel(&mut self) -> Result<(), PlayRefusal> {
        self.scheduler.cancel().map_err(|_| PlayRefusal::Kernel)?;
        self.pending.fill(None);
        self.held.fill(None);
        for keymap in &mut self.keymaps {
            keymap.reset();
        }
        self.cancelled = true;
        Ok(())
    }
    fn binding(&self, node: NodeId) -> Result<Binding, PlayRefusal> {
        self.bindings
            .get(usize::from(node.0))
            .copied()
            .flatten()
            .ok_or(PlayRefusal::Kernel)
    }
    fn drive(&mut self) -> Result<(), PlayRefusal> {
        for _ in 0..512 {
            while let Some(request) = self.scheduler.next_host_request() {
                let binding = self.binding(request.node)?;
                if binding.effect == Effect::Keyboard {
                    if self.pending[usize::from(binding.form)]
                        .replace(request)
                        .is_some()
                    {
                        return Err(PlayRefusal::Kernel);
                    }
                } else {
                    self.apply(request, binding)?;
                }
            }
            match self.scheduler.step().map_err(PlayRefusal::Scheduler)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle
                    if self.pending[..self.form_count].iter().all(Option::is_some) =>
                {
                    return Ok(());
                }
                SchedulerStatus::Cancelled => return Err(PlayRefusal::Cancelled),
                _ => return Err(PlayRefusal::Kernel),
            }
        }
        Err(PlayRefusal::WorkBound)
    }
}
