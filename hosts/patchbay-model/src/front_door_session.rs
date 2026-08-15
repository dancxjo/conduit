//! One live local Body session that supplies canonical front-door truth.

use conduit_body::{
    AdmissionChallenge, AdmissionManager, AdmissionSigns, AmbientAdmissionProof,
    AuthenticatedHostObservation, Body, BodyMembership, CandidateId, CandidateInventory,
    CandidateObservation, MembershipCredential, MembershipProofId, PartId, Wake, WakeLifecycle,
};
use conduit_core::{
    bind_active_play, ActivePlayId, ActivePlayIdentity, BootId, HostId, LineAvailability, LineId,
    LineOffer, OfferGeneration, PlanId, SignId,
};
use conduit_presentation::Presentation;
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig, ThreadTimer};

use crate::{
    front_door_topology::FrontDoorTopology, FormEditor, PartsView, PatchbayGraph, PatchbayModel,
    PatchbayPresentation, PatchbayRequestId, PlanDocument, PlayDocument, PortableProjectionError,
    RendererProjectionError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFrontDoorProjection {
    pub presentation: Presentation,
    pub parts: PartsView,
}

#[derive(Clone)]
pub struct LocalFrontDoor {
    pub(super) model: PatchbayModel,
    pub(super) editor: FormEditor,
    pub(super) form_name: String,
    pub(super) body: Body,
    pub(super) wake: Option<Wake>,
    pub(super) membership: BodyMembership,
    pub(super) candidates: CandidateInventory,
    pub(super) admissions: AdmissionManager,
    pub(super) here: PartId,
    pub(super) composition: StdHostComposition,
    pub(super) plan: Option<PlanDocument>,
    pub(super) play: Option<PlayDocument>,
    pub(super) active_play: Option<ActivePlayIdentity>,
    pub(super) topology: FrontDoorTopology,
    pub(super) revision: u64,
}

impl LocalFrontDoor {
    pub fn fresh() -> Result<Self, String> {
        let model =
            PatchbayModel::fresh_with_composition(StdHostComposition::minimal().with_text());
        Self::from_model(model)
    }

    pub fn with_identity(host_id: HostId, boot_id: BootId) -> Result<Self, String> {
        Self::from_model(PatchbayModel::with_identity_and_composition(
            host_id,
            boot_id,
            StdHostComposition::minimal().with_text(),
        ))
    }

    fn from_model(model: PatchbayModel) -> Result<Self, String> {
        let composition = StdHostComposition::minimal().with_text();
        let editor = FormEditor::from_source(
            "patchbay-front-door.conduit".into(),
            include_str!("../../../examples/patchbay-front-door.conduit").into(),
        )
        .map_err(|error| error.to_string())?;
        let checked = editor
            .view()
            .checked
            .forms
            .iter()
            .find(|form| form.name == "patchbay-front-door")
            .ok_or("canonical front-door Form is absent")?
            .checked_form_id
            .clone();
        let source = editor
            .view()
            .checked
            .source_document_id
            .clone()
            .ok_or("canonical front-door Form is unchecked")?;
        let body = Body::born(source, checked, 0, SignId::from("patchbay/front-door/born"))
            .map_err(|error| error.to_string())?;
        let (body, wake) = body
            .wake(1, SignId::from("patchbay/front-door/woke"))
            .map_err(|error| error.to_string())?;
        let mut membership =
            BodyMembership::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
        let here = PartId::bind(&body.body_id, model.advertisement().host_id.as_str(), 0)
            .map_err(|error| format!("{error:?}"))?;
        let proof = MembershipProofId::bind("patchbay/front-door/local-birth")
            .map_err(|error| format!("{error:?}"))?;
        membership
            .admit(
                &body.body_id,
                membership.revision,
                here.clone(),
                proof.clone(),
                SignId::from("patchbay/front-door/local-admitted"),
            )
            .map_err(|error| format!("{error:?}"))?;
        membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &here,
                AuthenticatedHostObservation {
                    host_id: model.advertisement().host_id.clone(),
                    boot_id: model.advertisement().boot_id.clone(),
                    offer_generation: model.advertisement().offer_generation,
                    proof_id: proof,
                    sequence: 0,
                },
                SignId::from("patchbay/front-door/local-present"),
            )
            .map_err(|error| format!("{error:?}"))?;
        let candidates =
            CandidateInventory::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
        let admissions =
            AdmissionManager::new(body.body_id.clone()).map_err(|error| format!("{error:?}"))?;
        Ok(Self {
            model,
            editor,
            form_name: "patchbay-front-door".into(),
            body,
            wake: Some(wake),
            membership,
            candidates,
            admissions,
            here,
            composition,
            plan: None,
            play: None,
            active_play: None,
            topology: FrontDoorTopology::default(),
            revision: 1,
        })
    }

    pub fn body(&self) -> &Body {
        &self.body
    }

    pub fn wake(&self) -> Option<&Wake> {
        self.wake.as_ref()
    }

    pub fn advertisement(&self) -> &conduit_core::HostAdvertisement {
        self.model.advertisement()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn current_plan_id(&self) -> Option<&PlanId> {
        self.plan.as_ref().map(|document| &document.plan_id)
    }

    pub fn current_play_id(&self) -> Option<&ActivePlayId> {
        self.active_play
            .as_ref()
            .map(|identity| &identity.active_play_id)
    }

    pub fn observe_candidate(
        &mut self,
        observation: CandidateObservation,
    ) -> Result<CandidateId, String> {
        let candidate = self
            .candidates
            .observe(observation)
            .map_err(|error| format!("observe candidate: {error:?}"))?;
        self.advance()?;
        Ok(candidate)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn begin_ambient_admission(
        &mut self,
        candidate_id: &CandidateId,
        verifying_key: [u8; 32],
        nonce: [u8; 32],
        now_millis: u64,
        expires_at_millis: u64,
        requested: SignId,
    ) -> Result<AdmissionChallenge, String> {
        let challenge = self
            .admissions
            .begin_ambient(
                &mut self.candidates,
                candidate_id,
                verifying_key,
                nonce,
                now_millis,
                expires_at_millis,
                requested,
            )
            .map_err(|error| format!("begin ambient admission: {error:?}"))?;
        self.advance()?;
        Ok(challenge)
    }

    pub fn complete_ambient_admission(
        &mut self,
        proof: &AmbientAdmissionProof,
        now_millis: u64,
        signs: AdmissionSigns,
    ) -> Result<MembershipCredential, String> {
        let credential = self
            .admissions
            .complete_ambient(
                &mut self.candidates,
                &mut self.membership,
                proof,
                now_millis,
                signs,
            )
            .map_err(|error| format!("complete ambient admission: {error:?}"))?;
        self.advance()?;
        Ok(credential)
    }

    pub fn observe_part_offline(
        &mut self,
        part_id: &PartId,
        boot_id: &BootId,
        sign_id: SignId,
    ) -> Result<(), String> {
        self.membership
            .observe_offline(
                &self.body.body_id,
                self.membership.revision,
                part_id,
                boot_id,
                sign_id,
            )
            .map_err(|error| format!("observe Part offline: {error:?}"))?;
        self.advance()
    }

    /// Observe one exact finite Line without granting membership, authority,
    /// or admission into any Plan.
    pub fn observe_line(&mut self, offer: LineOffer) -> Result<(), String> {
        self.topology
            .observe_line(offer, self.model.advertisement(), &self.candidates)?;
        self.advance()
    }

    /// Revise only current Line availability. The Line binding and contract
    /// remain exact and immutable, as do any Plans that admitted them.
    pub fn observe_line_availability(
        &mut self,
        line_id: &LineId,
        availability: LineAvailability,
        sign_id: SignId,
    ) -> Result<(), String> {
        self.topology
            .observe_availability(line_id, availability, sign_id)?;
        self.advance()
    }

    /// Replace current boot/offer truth for the durable local Part. Existing
    /// Plans remain immutable; explicit replanning is a separate operation.
    pub fn observe_local_restart(
        &mut self,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Result<(), String> {
        let host_id = self.model.advertisement().host_id.clone();
        let next_model = PatchbayModel::with_identity_composition_and(
            host_id.clone(),
            boot_id.clone(),
            self.composition,
            |advertisement| {
                advertisement.offer_generation = offer_generation;
                Ok(())
            },
        )?;
        self.membership
            .observe_present(
                &self.body.body_id,
                self.membership.revision,
                &self.here,
                AuthenticatedHostObservation {
                    host_id,
                    boot_id,
                    offer_generation,
                    proof_id: MembershipProofId::bind(&format!(
                        "patchbay/front-door/local-continuity/{}",
                        self.membership.revision.0
                    ))
                    .map_err(|error| format!("continuity proof: {error:?}"))?,
                    sequence: self.membership.revision.0,
                },
                SignId::from(format!(
                    "patchbay/front-door/local-restarted/{}",
                    self.membership.revision.0
                )),
            )
            .map_err(|error| format!("observe local restart: {error:?}"))?;
        self.model = next_model;
        self.advance()
    }

    pub fn observe_local_offline(&mut self) -> Result<(), String> {
        let boot_id = self
            .membership
            .parts
            .iter()
            .find(|part| part.part_id == self.here)
            .and_then(|part| part.current.as_ref())
            .map(|current| current.boot_id.clone())
            .ok_or("local Part is already offline")?;
        self.membership
            .observe_offline(
                &self.body.body_id,
                self.membership.revision,
                &self.here,
                &boot_id,
                SignId::from(format!(
                    "patchbay/front-door/local-offline/{}",
                    self.membership.revision.0
                )),
            )
            .map_err(|error| format!("observe local offline: {error:?}"))?;
        self.advance()
    }

    /// Plan the ordinary entrance Form. Replanning is explicit and only
    /// follows canonical unsatisfied truth; it never mutates an active Plan.
    pub fn plan_form(&mut self) -> Result<PlanId, String> {
        let wake = self
            .wake
            .as_ref()
            .ok_or("planning requires an explicit Wake after Birth")?;
        let advertisement = self.model.advertisement().clone();
        let config = StdHostConfig {
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
        };
        let host = StdHost::new_with_composition(config, self.composition);
        let expanded = self
            .editor
            .expand_form(&self.form_name)
            .map_err(|error| error.to_string())?;
        let plan = host
            .plan_expanded_local(&expanded)
            .map_err(|error| error.to_string())?;
        let awaiting = match wake.lifecycle {
            WakeLifecycle::AwaitingPlan => wake.clone(),
            WakeLifecycle::Playing => {
                let prior = self
                    .current_plan_id()
                    .ok_or("playing Wake has no retained Plan")?;
                wake.became_unsatisfied(
                    prior,
                    SignId::from(format!("patchbay/front-door/unsatisfied/{}", self.revision)),
                )
                .map_err(|error| error.to_string())?
            }
            lifecycle => {
                return Err(format!(
                    "planning is unavailable while Wake is {lifecycle:?}"
                ))
            }
        };
        let planned = awaiting
            .plan_ready(
                &plan,
                SignId::from(format!("patchbay/front-door/planned/{}", self.revision)),
            )
            .map_err(|error| error.to_string())?;
        let plan_document = PlanDocument::from_plan(
            PatchbayRequestId::new(format!("patchbay/front-door/plan/{}", self.revision))
                .map_err(|error| format!("plan request: {error:?}"))?,
            &plan,
        )
        .map_err(|error| format!("plan document: {error:?}"))?;
        self.wake = Some(planned);
        self.plan = Some(plan_document);
        self.play = None;
        self.active_play = None;
        self.advance()?;
        Ok(plan.plan_id)
    }

    /// Execute the current exact Plan through the installed std Host.
    pub fn play_plan(&mut self) -> Result<ActivePlayId, String> {
        let wake = self
            .wake
            .as_ref()
            .ok_or("Play requires an explicit Wake after Birth")?;
        if wake.lifecycle != WakeLifecycle::AwaitingPlay {
            return Err(format!(
                "Play is unavailable while Wake is {:?}",
                wake.lifecycle
            ));
        }
        let advertisement = self.model.advertisement().clone();
        let config = StdHostConfig {
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            offer_generation: advertisement.offer_generation,
        };
        let mut host = StdHost::new_with_composition(config, self.composition);
        let plan = self
            .plan
            .as_ref()
            .map(|document| document.exact.clone())
            .ok_or("Play requires a current exact Plan")?;
        let play_identity = bind_active_play(
            &plan.plan_id,
            &advertisement.host_id,
            &advertisement.boot_id,
            0,
        );
        let playing = self
            .wake
            .as_ref()
            .expect("explicit Wake checked above")
            .play_started(
                &play_identity,
                SignId::from(format!("patchbay/front-door/playing/{}", self.revision)),
            )
            .map_err(|error| error.to_string())?;
        let fragment = plan
            .fragments
            .first()
            .cloned()
            .ok_or("front-door Plan has no local fragment")?;
        let report = host.run_fragment_to(fragment, &mut Vec::new(), &mut ThreadTimer)?;
        let play_document = PlayDocument::from_report(&plan, &report)
            .map_err(|error| format!("play document: {error:?}"))?;
        self.wake = Some(playing);
        self.play = Some(play_document);
        self.active_play = Some(play_identity.clone());
        self.advance()?;
        Ok(play_identity.active_play_id)
    }

    pub fn plan_and_play(&mut self) -> Result<(PlanId, ActivePlayId), String> {
        let plan_id = self.plan_form()?;
        let play_id = self.play_plan()?;
        Ok((plan_id, play_id))
    }

    pub fn wake_body(&mut self) -> Result<conduit_body::WakeId, String> {
        if self.wake.is_some() {
            return Err("Body already has a current Wake".into());
        }
        let (body, wake) = self
            .body
            .wake(
                self.revision,
                SignId::from(format!("patchbay/front-door/woke/{}", self.revision)),
            )
            .map_err(|error| error.to_string())?;
        let wake_id = wake.wake_id.clone();
        self.body = body;
        self.wake = Some(wake);
        self.advance()?;
        Ok(wake_id)
    }

    pub fn project(&self) -> Result<LocalFrontDoorProjection, String> {
        let parts = PartsView::project(
            &self.body,
            &self.membership,
            &self.candidates,
            &self.here,
            self.plan.as_ref().map(|document| &document.exact),
            self.active_play.as_ref(),
            self.wake.is_some(),
        )
        .map_err(|error| format!("{error:?}"))?;
        let snapshot = self.topology.snapshot(
            self.model.startup_snapshot(),
            &self.candidates,
            &self.membership,
        );
        let topology = conduit_observatory::build_report(&snapshot)?;
        let graph = PatchbayGraph::from_expanded(
            &self
                .editor
                .expand_form(&self.form_name)
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let projection = PatchbayPresentation::new(
            self.revision,
            self.editor.view(),
            self.plan.clone(),
            self.play.clone(),
            Some(topology),
            Vec::new(),
        )
        .map_err(display_projection)?
        .with_graph(graph)
        .map_err(display_projection)?;
        let presentation = match &self.wake {
            Some(wake) => projection
                .to_portable_front_door(&self.body, wake, &parts)
                .map_err(display_portable)?,
            None => projection
                .to_portable_lulled_front_door(&self.body, &parts)
                .map_err(display_portable)?,
        };
        Ok(LocalFrontDoorProjection {
            presentation,
            parts,
        })
    }

    fn advance(&mut self) -> Result<(), String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("front-door presentation revision exhausted")?;
        Ok(())
    }
}

fn display_projection(error: RendererProjectionError) -> String {
    error.to_string()
}

fn display_portable(error: PortableProjectionError) -> String {
    error.to_string()
}
