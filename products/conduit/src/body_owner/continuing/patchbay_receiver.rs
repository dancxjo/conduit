//! One installed owner's pending Patchbay command and bounded worker receipt.
use conduit_face::{Face, FaceContributionBasis, FaceInteraction, MaskShow};
use conduit_kernel::{Failure, FailureCode, HostCallDisposition, HostCallOutcome};
use conduit_std_host::{
    body_execution::{BodyForeOutputAdapter, PatchbayInteractionHost},
    BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery,
};
use patchbay_application::{
    patchbay_interaction_outcome, PatchbayApplicationPort, PatchbayApplicationRequest,
    PatchbayCommandRefusal, PreparedPatchbayCommand,
};
use std::sync::{Arc, Mutex};

pub(super) type PatchbayResult = Result<Option<PatchbayApplicationRequest>, PatchbayCommandRefusal>;

#[derive(Clone)]
pub(in super::super) struct PreparedPatchbay {
    pub(super) queue: BodyLiveForeQueue,
    pub(super) receiver: PatchbayReceiver,
    basis: FaceContributionBasis,
}

pub(super) fn prepare(
    host: &conduit_std_host::StdHost,
    wake: &conduit_body::Wake,
    plan: &conduit_body::BodyPlan,
    sources: &[super::super::super::retained_source::RetainedPlotSource],
    control: conduit_std_host::RunControl,
) -> Result<Option<PreparedPatchbay>, String> {
    let mut owners = plan.plots.iter().flat_map(|part| {
        part.plan.fragments.iter().flat_map(move |fragment| {
            fragment.placements.iter().filter_map(move |gear| {
                (gear.kind_id.as_str() == patchbay_application::PATCHBAY_INTERACTION_KIND)
                    .then_some((part, gear))
            })
        })
    });
    let Some((part, gear)) = owners.next() else {
        return Ok(None);
    };
    if owners.next().is_some() || host.image_identity().is_none() {
        return Err("Patchbay requires one resident owner on an IMAGE-bound Host".into());
    }
    let play = host.prospective_body_play(wake, plan)?;
    let subjects = plan
        .plots
        .iter()
        .map(|part| {
            let plot = sources
                .iter()
                .map(|source| &source.plot.expanded)
                .find(|plot| {
                    plot.checked_plot_id == part.plot.checked_plot_id
                        && plot.source_document_id == part.plot.source_document_id
                })
                .ok_or("Patchbay resident Source is unavailable")?;
            Ok((plot, plot.name.as_str()))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let port = PatchbayApplicationPort::open_body_residents(
        plan,
        &play.active_play_id,
        subjects.into_iter(),
        part.plot.checked_plot_id.as_str(),
    )?;
    let queue =
        BodyLiveForeQueue::for_face_interaction_plan(&part.plan, &gear.placement_id, control, 256)?;
    Ok(Some(PreparedPatchbay {
        queue,
        receiver: PatchbayReceiver::new(port),
        basis: FaceContributionBasis {
            checked_plot_id: part.plot.checked_plot_id.clone(),
            plan_id: play.plan_id,
            active_play_id: play.active_play_id,
            required_interaction_context: None,
        },
    }))
}

impl PreparedPatchbay {
    pub(in super::super) fn matches_plot(&self, plot: &conduit_core::CheckedPlotId) -> bool {
        self.basis.checked_plot_id == *plot && !self.queue.status().play_terminal
    }

    pub(in super::super) fn apply(
        &self,
        face: Face,
        show: &MaskShow,
        interaction: &FaceInteraction,
        deadline: std::time::Instant,
    ) -> Result<Option<PatchbayApplicationRequest>, String> {
        if self.receiver.submit(
            &self.queue,
            face,
            show.clone(),
            self.basis.clone(),
            interaction,
        )? == BodyLiveForeAdmission::Full
        {
            return Err("Patchbay interaction Fore is full".into());
        }
        loop {
            if let Some(result) = self.receiver.take_result()? {
                return result.map_err(|error| format!("Patchbay action refused: {error:?}"));
            }
            if self.queue.status().play_terminal || std::time::Instant::now() >= deadline {
                return Err("control-outcome-unknown".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    pub(in super::super) fn fragment(
        &self,
        body: &conduit_body::BodyId,
        play: Option<&conduit_core::ActivePlayId>,
        focused: bool,
    ) -> Result<Option<conduit_face::FaceFragment>, String> {
        let status = self.queue.status();
        if play != Some(&self.basis.active_play_id) || status.play_terminal {
            return Ok(None);
        }
        let state = self
            .receiver
            .0
            .lock()
            .map_err(|_| "Patchbay receiver unavailable")?;
        state
            .port
            .face_fragment(
                self.basis.clone(),
                body,
                focused
                    && !status.close_requested
                    && state.pending.is_none()
                    && state.result.is_none(),
            )
            .map(Some)
            .map_err(|error| format!("Patchbay Face projection: {error:?}"))
    }
}

#[derive(Clone)]
pub(super) struct PatchbayReceiver(Arc<Mutex<ReceiverState>>);

struct ReceiverState {
    port: PatchbayApplicationPort,
    bytes: Vec<u8>,
    pending: Option<Pending>,
    result: Option<PatchbayResult>,
}

struct Pending {
    face: Face,
    show: MaskShow,
    basis: FaceContributionBasis,
    command: PreparedPatchbayCommand,
}

impl PatchbayReceiver {
    pub(super) fn new(port: PatchbayApplicationPort) -> Self {
        Self(Arc::new(Mutex::new(ReceiverState {
            port,
            bytes: Vec::with_capacity(conduit_face::MAX_FACE_INTERACTION_BYTES),
            pending: None,
            result: None,
        })))
    }

    pub(super) fn submit(
        &self,
        queue: &BodyLiveForeQueue,
        face: Face,
        show: MaskShow,
        basis: FaceContributionBasis,
        interaction: &FaceInteraction,
    ) -> Result<BodyLiveForeAdmission, String> {
        let mut state = self.0.lock().map_err(|_| "Patchbay receiver unavailable")?;
        if state.pending.is_some() || state.result.is_some() {
            return Err("Patchbay action is awaiting its owner receipt".into());
        }
        let body = face
            .basis
            .body_id
            .as_ref()
            .ok_or("Patchbay Face has no Body")?;
        let command = state
            .port
            .admit_face_command(body, basis.clone(), &face, &show, interaction)
            .map_err(|error| format!("Patchbay action refused: {error:?}"))?;
        let bytes = interaction.encode();
        let admission = queue.submit(&bytes)?;
        if let BodyLiveForeAdmission::Accepted { .. } = admission {
            state.bytes.clear();
            state.bytes.extend_from_slice(&bytes);
            state.pending = Some(Pending {
                face,
                show,
                basis,
                command,
            });
        }
        Ok(admission)
    }

    pub(super) fn take_result(&self) -> Result<Option<PatchbayResult>, String> {
        Ok(self
            .0
            .lock()
            .map_err(|_| "Patchbay receiver unavailable")?
            .result
            .take())
    }
}

impl BodyForeOutputAdapter for PatchbayReceiver {
    fn deliver(&mut self, _: &ExternalForeDelivery) -> Result<(), String> {
        Err("Patchbay interaction Fore declares no output".into())
    }

    fn patchbay_interaction_host(&mut self) -> Option<&mut dyn PatchbayInteractionHost> {
        Some(self)
    }
}

impl PatchbayInteractionHost for PatchbayReceiver {
    fn apply_interaction(&mut self, input: &[u8]) -> HostCallOutcome {
        let Ok(mut state) = self.0.lock() else {
            return unavailable();
        };
        if state.bytes != input {
            return unavailable();
        }
        let Some(pending) = state.pending.take() else {
            return unavailable();
        };
        let Some(body) = &pending.face.basis.body_id else {
            return unavailable();
        };
        let result = state.port.execute_face_command(
            pending.command,
            body,
            &pending.basis,
            &pending.face,
            &pending.show,
        );
        let outcome = patchbay_interaction_outcome(&result);
        state.result = Some(result);
        outcome
    }
}

fn unavailable() -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Denied,
        output: None,
        failure: Some(Failure {
            code: FailureCode::HostCallDenied,
            detail: 71,
        }),
    }
}
