//! Native Host adapter around the one fixed production scheduler.
mod effects;
mod preparation;
#[cfg(all(target_arch = "x86_64", target_os = "none"))]
mod protection;
#[cfg(test)]
mod tests;

use super::application_delivery::NativeApplication;
use super::{AdmittedPlotInput, PreparedNativeWorkset, WorksetRefusal};
use crate::keyboard_text_backs::PlannedBack;
use alloc::boxed::Box;
use conduit_human::{ConduitIntlKeymap, KeyEvent, KeyTransition};
use conduit_kernel::{
    FixedSignLog, FixedValueStore, KernelEvent, NodeId,
    scheduler::{FixedScheduler, HostCallRequest, SchedulerStatus},
};
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
use conduit_semantic_catalog::BoundedTextState;

const PLOTS: usize = super::NATIVE_PLOT_CAPACITY;
const NODES: usize = 14;
const CORDS: usize = 10;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const SIGN_ITEMS: usize = 1024;
type Scheduler = FixedScheduler<
    PlannedBack,
    FixedValueStore<10, 3072>,
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
    ApplicationEvent,
    Application,
    ApplicationPresentation,
}
#[derive(Clone, Copy)]
struct Binding {
    plot: u8,
    effect: Effect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayRefusal {
    Preparation,
    Protection(crate::composition::MachineRunError),
    Kernel,
    Scheduler(conduit_kernel::scheduler::SchedulerError),
    HostFailure(conduit_kernel::Failure),
    Foreground,
    InputOwnership,
    InputPressure,
    WorkBound,
    Cancelled,
}
impl PlayRefusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Protection(error) => error.as_str(),
            Self::Preparation => "native-body-play-preparation-refused",
            Self::Kernel => "native-body-kernel-boundary-refused",
            Self::Scheduler(_) => "native-body-kernel-step-refused",
            Self::HostFailure(failure) => failure.code.as_str(),
            Self::Foreground => "native-body-foreground-unavailable",
            Self::InputOwnership => "native-body-input-ownership-unavailable",
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
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    pure_results: [Option<crate::text_protection::PureKeyboardOutput>; PLOTS],
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    protected: [Option<
        crate::text_protection::ProtectedText<crate::protected_region::BodyRegionBinding>,
    >; PLOTS],
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    protection_admissions: [Option<crate::text_protection::BodyTextAdmission>; PLOTS],
    scheduler: Box<Scheduler>,
    bindings: [Option<Binding>; NODES],
    keymaps: [ConduitIntlKeymap; PLOTS],
    #[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
    editors: [Option<BoundedTextState>; PLOTS],
    pending: [Option<HostCallRequest>; PLOTS],
    held: [Option<u8>; 256],
    presentations: [Option<NativePresentation>; PLOTS],
    application_views: [Option<conduit_presentation::ApplicationView>; PLOTS],
    applications: [Option<NativeApplication>; PLOTS],
    application_requests: [Option<super::NativeApplicationRequest>; PLOTS],
    input_owners: [Option<AdmittedPlotInput>; PLOTS],
    plot_count: usize,
    cancelled: bool,
    admitted_plan: conduit_core::PlanId,
    active_body_play: Option<conduit_body::BodyPlayIdentity>,
}

impl NativeWorksetPlay {
    #[cfg(test)]
    pub(crate) fn pending_requests(&self) -> [Option<HostCallRequest>; PLOTS] {
        self.pending
    }
    pub fn prepare(prepared: &PreparedNativeWorkset) -> Result<Self, WorksetRefusal> {
        preparation::prepare(prepared, None)
    }
    pub fn prepare_with_biography(
        prepared: &PreparedNativeWorkset,
        evidence: &conduit_body::BodyBiographyEvidence,
    ) -> Result<Self, WorksetRefusal> {
        preparation::prepare(prepared, Some(evidence))
    }
    pub fn refresh_tutorial(
        &mut self,
        evidence: &conduit_body::BodyBiographyEvidence,
    ) -> Result<(), PlayRefusal> {
        for plot in 0..self.plot_count {
            if !matches!(
                self.applications[plot],
                Some(NativeApplication::Tutorial(_))
            ) {
                continue;
            }
            let application = super::tutorial_application::TutorialApplication::prepare(
                evidence,
                conduit_tutorial_plot::TutorialPlayback::Playing,
            )
            .map_err(|_| PlayRefusal::Kernel)?;
            self.applications[plot] = Some(NativeApplication::Tutorial(application));
            if let Some(request) = self.pending[plot].take() {
                let _ = self.take_application_view(plot);
                self.output(request, Some(&[]))?;
                self.drive()?;
            }
        }
        Ok(())
    }
    pub fn start_for(
        &mut self,
        plan: &conduit_body::BodyPlan,
        play: &conduit_body::BodyPlayIdentity,
    ) -> Result<(), PlayRefusal> {
        if self.cancelled
            || self.active_body_play.is_some()
            || plan.plan_id != self.admitted_plan
            || plan.verify_seal().is_err()
            || !play.validate_for(plan)
        {
            return Err(PlayRefusal::Preparation);
        }
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        if let Err(error) = self.activate_protection(plan, play) {
            let _ = self.cancel();
            return Err(error);
        }
        self.active_body_play = Some(play.clone());
        if let Err(error) = self.drive() {
            let _ = self.cancel();
            return Err(error);
        }
        Ok(())
    }
    pub fn sign_retention_gap(&self) -> Option<conduit_kernel::SignRetentionGap> {
        conduit_kernel::SignQuery::retention_gap(self.scheduler.signs())
    }
    pub fn take_presentation(&mut self, plot: usize) -> Option<NativePresentation> {
        self.presentations.get_mut(plot)?.take()
    }
    pub fn application_view(&self, plot: usize) -> Option<&conduit_presentation::ApplicationView> {
        self.application_views.get(plot)?.as_ref()
    }
    pub fn patchbay_graph(&self, plot: usize) -> Option<&patchbay_graph::PatchbayGraph> {
        match self.applications.get(plot)?.as_ref()? {
            NativeApplication::Patchbay(application) => application.selected_graph(),
            _ => None,
        }
    }
    pub fn select_patchbay_target(
        &mut self,
        patchbay: usize,
        target: usize,
    ) -> Result<(), PlayRefusal> {
        match self.applications.get_mut(patchbay).and_then(Option::as_mut) {
            Some(NativeApplication::Patchbay(application)) => application.select(target)?,
            _ => return Err(PlayRefusal::Foreground),
        }
        let _ = self.take_application_view(patchbay);
        let request = self.pending[patchbay].ok_or(PlayRefusal::InputPressure)?;
        self.output(request, Some(&[]))?;
        self.pending[patchbay] = None;
        self.drive()
    }
    pub fn take_application_view(
        &mut self,
        plot: usize,
    ) -> Option<conduit_presentation::ApplicationView> {
        self.application_views.get_mut(plot)?.take()
    }
    pub fn take_application_request(
        &mut self,
        plot: usize,
    ) -> Option<super::NativeApplicationRequest> {
        self.application_requests.get_mut(plot)?.take()
    }
    pub fn set_mask_topology(
        &mut self,
        topology: &patchbay_application::PatchbayMaskTopology,
    ) -> Result<(), PlayRefusal> {
        for plot in 0..self.plot_count {
            let is_patchbay = match self.applications[plot].as_mut() {
                Some(NativeApplication::Patchbay(application)) => {
                    application.set_mask_topology(topology);
                    true
                }
                _ => false,
            };
            if !is_patchbay {
                continue;
            }
            let _ = self.take_application_view(plot);
            let request = self.pending[plot].ok_or(PlayRefusal::InputPressure)?;
            self.output(request, Some(&[]))?;
            self.pending[plot] = None;
            self.drive()?;
        }
        Ok(())
    }
    pub fn complete_tour_run(
        &mut self,
        tour: usize,
        proof: conduit_tour_model::TourRunProof,
    ) -> Result<(), PlayRefusal> {
        match self.applications.get_mut(tour).and_then(Option::as_mut) {
            Some(NativeApplication::Tour(application)) => application
                .complete_run(proof)
                .map_err(|_| PlayRefusal::Kernel)?,
            _ => return Err(PlayRefusal::Foreground),
        }
        let _ = self.take_application_view(tour);
        let request = self.pending[tour].ok_or(PlayRefusal::InputPressure)?;
        self.output(request, Some(&[]))?;
        self.pending[tour] = None;
        self.drive()
    }
    pub fn application_event(
        &mut self,
        foreground: usize,
        encoded: &[u8],
    ) -> Result<(), PlayRefusal> {
        if self.cancelled {
            return Err(PlayRefusal::Cancelled);
        }
        if foreground >= self.plot_count {
            return Err(PlayRefusal::Foreground);
        }
        let owner = self.input_owners[foreground]
            .as_ref()
            .ok_or(PlayRefusal::InputOwnership)?;
        if owner.value_kind.as_str() != conduit_presentation::APPLICATION_EVENT_INFO_ID {
            return Err(PlayRefusal::InputOwnership);
        }
        let request = self.pending[foreground].ok_or(PlayRefusal::InputPressure)?;
        self.output(request, Some(encoded))?;
        self.pending[foreground] = None;
        self.drive()
    }
    /// A captured release still belongs here when another surface has focus.
    pub fn owns_release(&self, event: KeyEvent) -> bool {
        !self.cancelled
            && event.transition() == KeyTransition::Released
            && self.held[usize::from(event.usage())].is_some()
    }
    pub fn input_owner(&self, plot: usize) -> Option<&AdmittedPlotInput> {
        self.input_owners.get(plot)?.as_ref()
    }
    /// Foreground is supplied by the authoritative workspace selection. A held
    /// key keeps its original owner across subsequent selection changes.
    pub fn input(&mut self, foreground: usize, event: KeyEvent) -> Result<bool, PlayRefusal> {
        if self.cancelled {
            return Err(PlayRefusal::Cancelled);
        }
        if foreground >= self.plot_count {
            return Err(PlayRefusal::Foreground);
        }
        let usage = usize::from(event.usage());
        let owner = match (self.held[usage], event.transition()) {
            (Some(owner), _) => usize::from(owner),
            (None, KeyTransition::Pressed) => foreground,
            (None, KeyTransition::Released) => return Ok(false),
        };
        if self.input_owners[owner].is_none() {
            return Err(PlayRefusal::InputOwnership);
        }
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
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        self.revoke_protection(crate::protection_domain::KernelRevocationCause::PlayCancelled);
        self.scheduler.cancel().map_err(|_| PlayRefusal::Kernel)?;
        self.pending.fill(None);
        self.application_requests.fill(None);
        self.held.fill(None);
        for keymap in &mut self.keymaps {
            keymap.reset();
        }
        self.active_body_play = None;
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
        let result = self.drive_inner();
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        if result.is_err() {
            self.revoke_protection(crate::protection_domain::KernelRevocationCause::PlayFailed);
        }
        result
    }
    fn drive_inner(&mut self) -> Result<(), PlayRefusal> {
        for _ in 0..512 {
            while let Some(request) = self.scheduler.next_host_request() {
                let binding = self.binding(request.node)?;
                if matches!(binding.effect, Effect::Keyboard | Effect::ApplicationEvent) {
                    if self.pending[usize::from(binding.plot)]
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
                    if self.pending[..self.plot_count].iter().all(Option::is_some) =>
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
