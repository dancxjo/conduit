//! Immutable guest projections plus the exact Root-reviewed input catalog.
//! No BirthDraft reducer, naming catalog, Face projector or mutable draft lives here.
use super::{CrecheService, TransportError, errors};
mod native;
use alloc::{string::String, vec::Vec};
use conduit_birth_plot::{
    BirthActionOutcome, BirthActions, BirthDraftRefusal, BirthFaceBasis, BirthFaceRefusal,
    BirthPlotChoice, BirthSelection, BirthWidgetFace,
};
use conduit_face::{
    ApplicationEventKind, Face, FaceInteraction, MaskShow, SemanticApplicationView,
    SemanticFaceRefusal,
};
use conduitos_creche_core::{
    data::{DraftSnapshot as Snapshot, FaceRequest, Initialize, InteractionRequest},
    wire::{Event, InputKind, OutputKind, Request, Response},
};
use core::cell::RefCell;

pub struct CrecheDraft {
    service: RefCell<CrecheService>,
    snapshot: Snapshot,
    reviewed: Vec<BirthPlotChoice>,
}
impl CrecheDraft {
    pub fn new(uuid: String, choices: Vec<BirthPlotChoice>) -> Result<Self, BirthDraftRefusal> {
        let mut service = CrecheService::install_embedded().map_err(errors::draft)?;
        let initialize = Initialize {
            uuid,
            choices: choices.clone(),
            allow_idle_body: false,
        };
        let bytes = encode(&initialize).map_err(errors::draft)?;
        let response = service
            .input(InputKind::Initialize, &bytes)
            .map_err(errors::draft)?;
        let Response::Ready { revision } = response else {
            service.quarantine();
            return Err(BirthDraftRefusal::InvalidInventory);
        };
        let snapshot = fetch_snapshot(&mut service, revision, &choices).map_err(errors::draft)?;
        Ok(Self {
            service: RefCell::new(service),
            snapshot,
            reviewed: choices,
        })
    }
    pub fn allow_idle_body(mut self) -> Self {
        let result =
            self.service
                .get_mut()
                .request(Request::AllowIdle)
                .and_then(|reply| match reply {
                    Response::Ready { revision } => {
                        fetch_snapshot(self.service.get_mut(), revision, &self.reviewed)
                    }
                    _ => self.protocol(),
                });
        match result {
            Ok(snapshot) => self.snapshot = snapshot,
            Err(_) => self.service.get_mut().quarantine(),
        }
        self
    }
    pub fn revision(&self) -> u32 {
        self.snapshot.revision
    }
    pub fn friendly_name(&self) -> &str {
        &self.snapshot.friendly_name
    }
    pub fn search(&self) -> &str {
        &self.snapshot.search
    }
    pub fn requested_system(&self) -> &str {
        &self.snapshot.requested_system
    }
    pub fn choices(&self) -> &[BirthPlotChoice] {
        &self.snapshot.choices
    }
    pub fn naming_systems(&self) -> impl Iterator<Item = (&str, &str)> {
        self.snapshot
            .naming_systems
            .iter()
            .map(|(id, label)| (id.as_str(), label.as_str()))
    }
    pub fn face(&self, basis: &BirthFaceBasis) -> Result<Face, BirthFaceRefusal> {
        let mut service = self
            .service
            .try_borrow_mut()
            .map_err(|_| BirthFaceRefusal::InvalidProjection)?;
        let bytes = encode(&FaceRequest {
            revision: self.revision(),
            basis: basis.clone(),
        })
        .map_err(face_error)?;
        let response = service
            .input(InputKind::FaceBasis, &bytes)
            .map_err(face_error)?;
        let bytes = service
            .blob(response, OutputKind::Face, self.revision())
            .map_err(face_error)?;
        decode(&mut service, &bytes).map_err(face_error)
    }
    pub fn apply_face_interaction(
        &mut self,
        basis: &BirthFaceBasis,
        show: &MaskShow,
        input: &FaceInteraction,
    ) -> Result<BirthActionOutcome, BirthFaceRefusal> {
        let bytes = encode(&InteractionRequest {
            revision: self.revision(),
            basis: basis.clone(),
            show: show.clone(),
            input: input.clone(),
        })
        .map_err(face_error)?;
        let response = self
            .service
            .get_mut()
            .input(InputKind::FaceInteraction, &bytes)
            .map_err(face_error)?;
        self.outcome(response).map_err(face_error)
    }
    fn outcome(&mut self, response: Response) -> Result<BirthActionOutcome, TransportError> {
        match response {
            Response::Ready { revision } => {
                match fetch_snapshot(self.service.get_mut(), revision, &self.reviewed) {
                    Ok(snapshot) => self.snapshot = snapshot,
                    Err(error) => {
                        self.service.get_mut().quarantine();
                        return Err(error);
                    }
                }
                Ok(BirthActionOutcome::Changed)
            }
            Response::Blob {
                kind: OutputKind::Selection,
                revision,
                ..
            } => {
                if revision != self.revision() {
                    return self.protocol();
                }
                let bytes =
                    self.service
                        .get_mut()
                        .blob(response, OutputKind::Selection, revision)?;
                let selected: BirthSelection = decode(self.service.get_mut(), &bytes)?;
                self.bind_selection(selected).map(BirthActionOutcome::Birth)
            }
            _ => self.protocol(),
        }
    }
    fn protocol<T>(&mut self) -> Result<T, TransportError> {
        self.service.get_mut().quarantine();
        Err(TransportError::Protocol)
    }
    fn bind_selection(
        &mut self,
        selection: BirthSelection,
    ) -> Result<BirthSelection, TransportError> {
        let expected = conduit_body::BodyWorkset::from_plots(
            self.reviewed
                .iter()
                .zip(&self.snapshot.choices)
                .filter(|(_, choice)| choice.selected)
                .map(|(reviewed, _)| reviewed.plot.clone()),
        )
        .map_err(|_| TransportError::Protocol)?;
        if selection.revision != self.revision()
            || selection.friendly_name != self.friendly_name().trim()
            || selection.workset != expected
            || self
                .reviewed
                .iter()
                .zip(&self.snapshot.choices)
                .any(|(reviewed, choice)| choice.selected && reviewed.refusal.is_some())
        {
            return self.protocol();
        }
        // Identity and availability come from Root's reviewed catalog. The owning
        // lifecycle boundary still rechecks current execution admission at birth.
        Ok(BirthSelection {
            revision: selection.revision,
            friendly_name: selection.friendly_name,
            workset: expected,
        })
    }
}
impl BirthActions for CrecheDraft {
    fn apply_event(
        &mut self,
        revision: u32,
        action: &str,
        event: ApplicationEventKind,
        value: &str,
    ) -> Result<BirthActionOutcome, BirthDraftRefusal> {
        // Preserve canonical refusal order before the finite wire encoder. A
        // locally oversized action is recoverable input, not a guest fault.
        if revision != self.revision() {
            return Err(BirthDraftRefusal::StaleFace);
        }
        if action.len() > 64 || value.len() > 128 {
            return Err(BirthDraftRefusal::InvalidActionValue);
        }
        let event = match event {
            ApplicationEventKind::Input => Event::Input,
            ApplicationEventKind::Change => Event::Change,
            ApplicationEventKind::Activate => Event::Activate,
            _ => return Err(BirthDraftRefusal::UnknownAction),
        };
        let response = self
            .service
            .get_mut()
            .request(Request::Apply {
                revision,
                event,
                action,
                value,
            })
            .map_err(errors::draft)?;
        self.outcome(response).map_err(errors::draft)
    }
}
impl BirthWidgetFace for CrecheDraft {
    fn widget_face(&self) -> Result<SemanticApplicationView, SemanticFaceRefusal> {
        if !self
            .service
            .try_borrow()
            .is_ok_and(|service| service.is_live())
        {
            return Err(SemanticFaceRefusal::InvalidActionAvailability);
        }
        Ok(self.snapshot.widget.clone())
    }
}
fn fetch_snapshot(
    service: &mut CrecheService,
    revision: u32,
    reviewed: &[BirthPlotChoice],
) -> Result<Snapshot, TransportError> {
    let response = service.request(Request::Snapshot { revision })?;
    let bytes = service.blob(response, OutputKind::Snapshot, revision)?;
    let snapshot: Snapshot = decode(service, &bytes)?;
    if snapshot.revision != revision
        || snapshot.widget.revision != revision
        || snapshot.native_focus > (conduit_body::MAX_BODY_PLOTS as u32 + 5)
        || snapshot
            .native_refusal
            .as_ref()
            .is_some_and(|text| text.len() > 256)
        || snapshot.choices.len() != reviewed.len()
        || snapshot.choices.iter().zip(reviewed).any(|(guest, root)| {
            guest.title != root.title
                || guest.search_text != root.search_text
                || guest.plot != root.plot
                || guest.refusal != root.refusal
                || (guest.selected && root.refusal.is_some())
        })
    {
        service.quarantine();
        return Err(TransportError::Protocol);
    }
    Ok(snapshot)
}
fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, TransportError> {
    conduitos_creche_core::data::encode(value).map_err(|_| TransportError::Bounds)
}
fn decode<T: serde::de::DeserializeOwned>(
    service: &mut CrecheService,
    bytes: &[u8],
) -> Result<T, TransportError> {
    conduitos_creche_core::data::decode(bytes).map_err(|_| {
        service.quarantine();
        TransportError::Protocol
    })
}
fn face_error(error: TransportError) -> BirthFaceRefusal {
    match error {
        TransportError::Refused(conduitos_creche_core::wire::Refusal::Interaction) => {
            BirthFaceRefusal::Interaction(conduit_face::FaceInteractionRefusal::StaleFace)
        }
        TransportError::Refused(conduitos_creche_core::wire::Refusal::Projection) => {
            BirthFaceRefusal::InvalidProjection
        }
        other => BirthFaceRefusal::Draft(errors::draft(other)),
    }
}
