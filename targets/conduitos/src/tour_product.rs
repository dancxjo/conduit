//! ConduitOS realization of the shared Tour application controller.

use conduit_presentation::{ApplicationEvent, GraphicsScene};
use conduit_tour_model::{
    CANONICAL_SPECIMEN_ID, TourPointerOutcome, TourRunProof, TourWorkspaceController,
    TourWorkspaceRefusal, TourWorkspaceRequest,
};

use crate::{
    identity::BootIdentities,
    machine::{IdleBase, InterruptBase, MonotonicClockBase, SerialBase},
    offer::HostOffer,
    ordinary_plan::PreparationError,
    tour_play::{TourPlayError, TourPlayEvidence},
    tour_workspace::TourWorkspaceSceneRefusal,
};

#[path = "tour_inspection.rs"]
mod inspection;
#[path = "tour_pointer.rs"]
mod pointer;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TourProductUpdate {
    pub request: TourWorkspaceRequest,
    pub play: Option<TourPlayEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TourProductError {
    Controller(TourWorkspaceRefusal),
    Preparation(PreparationError),
    Play(TourPlayError),
    Scene(TourWorkspaceSceneRefusal),
}

impl TourProductError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Controller(_) => "tour-controller-refused",
            Self::Preparation(error) => error.as_str(),
            Self::Play(_) => "tour-play-refused",
            Self::Scene(_) => "tour-scene-refused",
        }
    }

    pub const fn controller_refusal(self) -> Option<TourWorkspaceRefusal> {
        match self {
            Self::Controller(refusal) => Some(refusal),
            _ => None,
        }
    }
}

pub struct TourProduct {
    controller: TourWorkspaceController,
    inspection: Option<inspection::RunInspection>,
    graph: Result<patchbay_graph::PatchbayGraph, TourWorkspaceSceneRefusal>,
}

impl TourProduct {
    pub fn canonical(revision: u32) -> Self {
        Self {
            controller: TourWorkspaceController::canonical(revision),
            inspection: None,
            graph: crate::tour_workspace::canonical_graph(),
        }
    }

    pub const fn controller(&self) -> &TourWorkspaceController {
        &self.controller
    }

    pub fn select_gear(&mut self, revision: u32, gear: &str) -> Result<(), TourProductError> {
        self.controller
            .select_gear(revision, gear)
            .map_err(TourProductError::Controller)
    }

    pub fn scene(&self, width: u16, height: u16) -> Result<GraphicsScene, TourProductError> {
        crate::tour_workspace::scene_with_graph(
            width,
            height,
            self.controller.state(),
            self.graph
                .as_ref()
                .map_err(|error| TourProductError::Scene(*error))?,
            self.inspection
                .as_ref()
                .map(|snapshot| &snapshot.observations),
        )
        .map_err(TourProductError::Scene)
    }

    pub fn accept_pointer(
        &mut self,
        sample: conduit_semantic_catalog::NormalizedPointerSample,
        width: u16,
        height: u16,
    ) -> Result<TourPointerOutcome, &'static str> {
        let layout =
            crate::tour_workspace::layout_for_state(width, height, self.controller.state())
                .map_err(|_| "tour-pointer-layout-refused")?;
        self.controller
            .accept_pointer(sample, &layout)
            .map_err(|_| "tour-pointer-refused")
    }

    pub fn dismiss_inspector(&mut self) -> Result<bool, TourProductError> {
        self.controller
            .dismiss_inspector()
            .map_err(TourProductError::Controller)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn accept<C, S, I, D>(
        &mut self,
        event: &ApplicationEvent,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        build_id: &str,
        clock: &mut C,
        serial: &mut S,
        interrupts: &mut I,
        idle: &mut D,
    ) -> Result<TourProductUpdate, TourProductError>
    where
        C: MonotonicClockBase,
        S: SerialBase,
        I: InterruptBase,
        D: IdleBase,
    {
        let request = self
            .controller
            .request(event)
            .map_err(TourProductError::Controller)?;
        let play = match request {
            TourWorkspaceRequest::OpenPatchbay => None,
            TourWorkspaceRequest::Run => {
                let mut prepared = crate::tour_play::prepare(identities, offer, build_id)
                    .map_err(TourProductError::Preparation)?;
                let mut inspection = inspection::RunInspection::from_plan(&prepared.plan)
                    .map_err(TourProductError::Preparation)?;
                let evidence =
                    crate::tour_play::run(&mut prepared, clock, serial, interrupts, idle)
                        .map_err(TourProductError::Play)?;
                self.controller
                    .complete_run(run_proof(&evidence))
                    .map_err(TourProductError::Controller)?;
                inspection.observations = evidence.observations.clone();
                self.inspection = Some(inspection);
                Some(evidence)
            }
        };
        Ok(TourProductUpdate { request, play })
    }
}

fn run_proof(evidence: &TourPlayEvidence) -> TourRunProof {
    TourRunProof {
        specimen_id: CANONICAL_SPECIMEN_ID.into(),
        source_document_id: evidence.source_document_id.clone(),
        checked_form_id: evidence.checked_form_id.clone(),
        expanded_form_id: evidence.expanded_form_id.clone(),
        plan_id: evidence.plan_id.clone(),
        active_play_id: evidence.active_play_id.clone(),
        result: evidence.result.into(),
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use conduit_presentation::{ApplicationEventKind, GraphicsPaintRole};
    use conduit_tour_model::{
        CANONICAL_RESULT, OPEN_PATCHBAY_ACTION_ID, RUN_ACTION_ID, TourWorkspacePhase,
    };

    use super::*;
    use crate::{
        machine::{BaseError, InterruptState},
        offer::CpuFeatures,
    };

    #[derive(Default)]
    struct Clock(u64);
    impl MonotonicClockBase for Clock {
        fn now(&mut self) -> u64 {
            self.0 += 1;
            self.0
        }
    }

    #[derive(Default)]
    struct Serial(Vec<Vec<u8>>);
    impl SerialBase for Serial {
        fn present(&mut self, bytes: &[u8]) -> Result<(), BaseError> {
            self.0.push(bytes.into());
            Ok(())
        }

        fn presentation_count(&self) -> u32 {
            self.0.len() as u32
        }
    }

    #[derive(Default)]
    struct Interrupts(bool);
    impl InterruptBase for Interrupts {
        fn enable(&mut self) {
            self.0 = true;
        }

        fn disable(&mut self) -> InterruptState {
            let state = InterruptState { enabled: self.0 };
            self.0 = false;
            state
        }

        fn restore(&mut self, state: InterruptState) {
            self.0 = state.enabled;
        }

        fn is_enabled(&self) -> bool {
            self.0
        }
    }

    #[derive(Default)]
    struct Idle(u32);
    impl IdleBase for Idle {
        fn wait_for_interrupt(&mut self) -> Result<(), BaseError> {
            self.0 += 1;
            Ok(())
        }

        fn idle_count(&self) -> u32 {
            self.0
        }
    }

    fn event(revision: u32, action: &str) -> ApplicationEvent {
        ApplicationEvent {
            revision,
            action: action.into(),
            kind: ApplicationEventKind::Activate,
            value: Vec::new(),
        }
    }

    fn fixture() -> (BootIdentities, HostOffer<'static>) {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            256 * 1024,
        );
        (identities, offer)
    }

    #[test]
    fn run_action_executes_the_canonical_play_before_showing_the_result() {
        let (identities, offer) = fixture();
        let mut product = TourProduct::canonical(10);
        let mut clock = Clock::default();
        let mut serial = Serial::default();
        let mut interrupts = Interrupts::default();
        let mut idle = Idle::default();

        let update = product
            .accept(
                &event(10, RUN_ACTION_ID),
                &identities,
                &offer,
                "build",
                &mut clock,
                &mut serial,
                &mut interrupts,
                &mut idle,
            )
            .unwrap();

        assert_eq!(update.request, TourWorkspaceRequest::Run);
        let evidence = update.play.unwrap();
        let graph = product.graph.as_ref().unwrap();
        assert_eq!(graph.source_document_id, evidence.source_document_id);
        assert_eq!(graph.checked_form_id, evidence.checked_form_id);
        assert_eq!(graph.expanded_form_id, evidence.expanded_form_id);
        assert_eq!(evidence.result, CANONICAL_RESULT);
        assert_eq!(evidence.observations.upper_input.text(), Some("hello"));
        assert_eq!(evidence.observations.upper_output.text(), Some("HELLO"));
        assert_eq!(
            evidence.observations.presentation_input.text(),
            Some("HELLO")
        );
        assert_eq!(serial.0, [CANONICAL_RESULT.as_bytes()]);
        assert_eq!(
            product.controller().state().phase,
            TourWorkspacePhase::ResultVisible
        );
        assert_eq!(product.controller().state().revision, 12);
        assert_eq!(
            product.controller().last_run().unwrap().plan_id,
            evidence.plan_id
        );
        let scene = product.scene(640, 480).unwrap();
        assert_eq!(scene.commands()[6].paint, GraphicsPaintRole::Accent);
        assert!(scene.commands()[7].payload().contains("Result visible"));
        let change_index = scene
            .commands()
            .iter()
            .position(|command| command.payload().starts_with("change\n"))
            .unwrap();
        let change = scene.commands()[change_index].payload();
        assert!(change.contains("Last run"));
        assert!(change.contains("= \"hello\""));
        assert!(change.contains("= \"HELLO\""));
        let literal_index = scene
            .commands()
            .iter()
            .position(|command| command.payload().starts_with("words\n"))
            .unwrap();
        let literal = scene.commands()[literal_index].payload();
        assert!(literal.contains("= unobserved"));
        assert!(!literal.contains("= \"hello\""));
        product
            .controller
            .request(&event(12, OPEN_PATCHBAY_ACTION_ID))
            .unwrap();
        product.select_gear(13, "meet-one-gear/change").unwrap();
        let inspector = product.inspector_presentation().unwrap().unwrap();
        assert!(
            inspector
                .text
                .iter()
                .any(|item| item.subject.ends_with("/state")
                    && item.text == "Last run\nin text: \"hello\"\nout text: \"HELLO\"")
        );
        let prepared = crate::tour_play::prepare(&identities, &offer, "build").unwrap();
        let placement = prepared
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| placement.kind_id.as_str() == "text/upper")
            .unwrap();
        assert!(
            inspector
                .text
                .iter()
                .any(|item| item.subject.ends_with("/implementation")
                    && item.text == placement.implementation_id.as_str())
        );
        assert!(
            inspector
                .text
                .iter()
                .any(|item| item.subject.ends_with("/placement")
                    && item.text == placement.placement_id.as_str())
        );
        assert!(
            inspector
                .subjects
                .iter()
                .any(|subject| subject.identity == evidence.plan_id.as_str())
        );
        assert!(
            inspector
                .subjects
                .iter()
                .any(|subject| subject.identity == evidence.active_play_id.as_str())
        );
        assert!(inspector.basis.body_id.is_none());
        let mut invalid = prepared.plan;
        invalid.fragments[0].placements.pop();
        assert!(inspection::RunInspection::from_plan(&invalid).is_err());
    }

    #[test]
    fn patchbay_action_changes_the_shared_state_without_inventing_a_play() {
        let (identities, offer) = fixture();
        let mut product = TourProduct::canonical(3);
        let update = product
            .accept(
                &event(3, OPEN_PATCHBAY_ACTION_ID),
                &identities,
                &offer,
                "build",
                &mut Clock::default(),
                &mut Serial::default(),
                &mut Interrupts::default(),
                &mut Idle::default(),
            )
            .unwrap();
        assert_eq!(update.request, TourWorkspaceRequest::OpenPatchbay);
        assert_eq!(update.play, None);
        assert_eq!(
            product.controller().state().phase,
            TourWorkspacePhase::PatchbayOpen
        );
        let outcome = product
            .accept_pointer(
                conduit_semantic_catalog::NormalizedPointerSample {
                    position_x: 700_000,
                    position_y: 100_000,
                    delta_x: 200_000,
                    delta_y: -400_000,
                    primary_pressed: true,
                    coalesced: 0,
                    dropped: 0,
                    queue_capacity: 2,
                    sequence: 1,
                },
                640,
                480,
            )
            .unwrap();
        assert_eq!(
            outcome,
            conduit_tour_model::TourPointerOutcome::Selected {
                subject: "meet-one-gear/change".into()
            }
        );
        assert!(
            product
                .scene(640, 480)
                .unwrap()
                .commands()
                .iter()
                .any(|command| command.payload().contains("selected meet-one-gear/change"))
        );
        let layout =
            crate::tour_workspace::layout_for_state(640, 480, product.controller().state())
                .unwrap();
        let x = u32::from(layout.patchbay.x) + u32::from(layout.patchbay.width) * 5 / 6;
        let outcome = product
            .accept_pointer(
                conduit_semantic_catalog::NormalizedPointerSample {
                    position_x: i64::from(x * 1_000_000 / 640),
                    position_y: 200_000,
                    delta_x: 0,
                    delta_y: 0,
                    primary_pressed: true,
                    coalesced: 0,
                    dropped: 0,
                    queue_capacity: 2,
                    sequence: 2,
                },
                640,
                480,
            )
            .unwrap();
        assert_eq!(
            outcome,
            conduit_tour_model::TourPointerOutcome::Selected {
                subject: "meet-one-gear/result".into()
            }
        );
    }
}
