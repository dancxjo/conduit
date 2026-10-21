//! Canonical BirthDraft state; no Body creation, execution, device, or Root grant.
use super::{
    data::{self, DraftSnapshot, FaceRequest, Initialize, InteractionRequest},
    wire::*,
};
use alloc::vec::Vec;
use conduit_birth_plot::{
    BirthActionOutcome, BirthActions, BirthDraft, BirthFaceRefusal, BirthWidgetFace,
};
use conduit_face::ApplicationEventKind;
mod presentation;
pub struct State {
    draft: Option<super::native::Controller>,
    incoming: Option<(InputKind, u32, Vec<u8>)>,
    outgoing: Vec<u8>,
    read_offset: u32,
}
impl Default for State {
    fn default() -> Self {
        Self {
            draft: None,
            incoming: None,
            outgoing: Vec::new(),
            read_offset: 0,
        }
    }
}
impl State {
    fn draft(&self) -> Result<&BirthDraft, Refusal> {
        self.draft
            .as_ref()
            .map(|controller| &controller.draft)
            .ok_or(Refusal::Order)
    }
    pub fn revision(&self) -> Result<u32, Refusal> {
        Ok(self.draft()?.revision())
    }
    fn current(&self, revision: u32) -> Result<&BirthDraft, Refusal> {
        let draft = self.draft()?;
        if draft.revision() != revision {
            return Err(Refusal::StaleFace);
        }
        Ok(draft)
    }
    fn publish<T: serde::Serialize>(
        &mut self,
        kind: OutputKind,
        value: &T,
    ) -> Result<Response, Refusal> {
        let revision = self.revision()?;
        self.outgoing = Vec::new();
        self.read_offset = 0;
        self.outgoing = data::encode(value)?;
        Ok(Response::Blob {
            kind,
            revision,
            bytes: self.outgoing.len() as u32,
        })
    }
    fn outcome(&mut self, outcome: BirthActionOutcome) -> Result<Response, Refusal> {
        match outcome {
            BirthActionOutcome::Changed => {
                self.outgoing = Vec::new();
                self.read_offset = 0;
                Ok(Response::Ready {
                    revision: self.revision()?,
                })
            }
            BirthActionOutcome::Birth(selection) => self.publish(OutputKind::Selection, &selection),
        }
    }
    pub fn execute(
        &mut self,
        request: Request<'_>,
        transfer: &mut [u8],
    ) -> Result<Response, Refusal> {
        if transfer.len() != TRANSFER_BYTES as usize {
            return Err(Refusal::Bounds);
        }
        match request {
            Request::Begin { kind, total } => {
                if total == 0 || total > MAX_BLOB_BYTES || self.incoming.is_some() {
                    return Err(Refusal::Order);
                }
                if (kind == InputKind::Initialize) != self.draft.is_none() {
                    return Err(Refusal::Order);
                }
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(total as usize)
                    .map_err(|_| Refusal::Capacity)?;
                self.incoming = Some((kind, total, bytes));
                self.outgoing = Vec::new();
                self.read_offset = 0;
                Ok(Response::Ready {
                    revision: self.draft.as_ref().map_or(1, |draft| draft.revision()),
                })
            }
            Request::Write {
                offset,
                window_offset,
                bytes,
            } => {
                let (_, total, incoming) = self.incoming.as_mut().ok_or(Refusal::Order)?;
                let end = offset
                    .checked_add(bytes)
                    .filter(|n| *n <= *total)
                    .ok_or(Refusal::Bounds)?;
                let window_end = window_offset
                    .checked_add(bytes)
                    .filter(|n| *n <= TRANSFER_BYTES)
                    .ok_or(Refusal::Bounds)?;
                if bytes == 0 || offset as usize != incoming.len() {
                    return Err(Refusal::Order);
                }
                incoming.extend_from_slice(&transfer[window_offset as usize..window_end as usize]);
                if incoming.len() != end as usize {
                    return Err(Refusal::Bounds);
                }
                Ok(Response::Ready {
                    revision: self.draft.as_ref().map_or(1, |draft| draft.revision()),
                })
            }
            Request::Commit => self.commit(),
            Request::NativeKey { revision, event } => {
                if self.incoming.is_some() {
                    return Err(Refusal::Order);
                }
                self.current(revision)?;
                let event =
                    conduit_human::KeyEvent::decode(&event).map_err(|_| Refusal::Malformed)?;
                let outcome = self.draft.as_mut().ok_or(Refusal::Order)?.accept(event);
                self.publish(OutputKind::Native, &outcome)
            }
            Request::AllowIdle => {
                if self.incoming.is_some() {
                    return Err(Refusal::Order);
                }
                let draft = self.draft.take().ok_or(Refusal::Order)?;
                self.draft = Some(draft.allow_idle());
                self.outgoing = Vec::new();
                self.read_offset = 0;
                Ok(Response::Ready {
                    revision: self.revision()?,
                })
            }
            Request::Apply {
                revision,
                event,
                action,
                value,
            } => {
                if self.incoming.is_some() {
                    return Err(Refusal::Order);
                }
                self.current(revision)?;
                let event = match event {
                    Event::Input => ApplicationEventKind::Input,
                    Event::Change => ApplicationEventKind::Change,
                    Event::Activate => ApplicationEventKind::Activate,
                };
                let result = self
                    .draft
                    .as_mut()
                    .ok_or(Refusal::Order)?
                    .apply_event(revision, action, event, value)
                    .map_err(Refusal::from)?;
                self.outcome(result)
            }
            Request::Snapshot { revision } => {
                if self.incoming.is_some() {
                    return Err(Refusal::Order);
                }
                let draft = self.current(revision)?;
                let value = DraftSnapshot {
                    revision,
                    native_focus: self.draft.as_ref().ok_or(Refusal::Order)?.focus as u32,
                    native_refusal: self.draft.as_ref().ok_or(Refusal::Order)?.refusal.clone(),
                    friendly_name: draft.friendly_name().into(),
                    search: draft.search().into(),
                    requested_system: draft.requested_system().into(),
                    naming_systems: draft
                        .naming_systems()
                        .map(|(a, b)| (a.into(), b.into()))
                        .collect(),
                    choices: draft.choices().to_vec(),
                    widget: draft.widget_face().map_err(|_| Refusal::Projection)?,
                };
                self.publish(OutputKind::Snapshot, &value)
            }
            Request::Selection { revision } => {
                if self.incoming.is_some() {
                    return Err(Refusal::Order);
                }
                let value = self
                    .current(revision)?
                    .selection(revision)
                    .map_err(Refusal::from)?;
                self.publish(OutputKind::Selection, &value)
            }
            Request::Read { offset, maximum } => {
                if self.incoming.is_some()
                    || maximum == 0
                    || maximum > TRANSFER_BYTES
                    || offset != self.read_offset
                    || offset as usize >= self.outgoing.len()
                {
                    return Err(Refusal::Order);
                }
                let total = self.outgoing.len() as u32;
                let bytes = maximum.min(total - offset);
                transfer.fill(0);
                transfer[..bytes as usize]
                    .copy_from_slice(&self.outgoing[offset as usize..(offset + bytes) as usize]);
                self.read_offset += bytes;
                Ok(Response::Chunk {
                    offset,
                    bytes,
                    total,
                    complete: self.read_offset == total,
                })
            }
        }
    }
    fn commit(&mut self) -> Result<Response, Refusal> {
        let (kind, total, bytes) = self.incoming.take().ok_or(Refusal::Order)?;
        if bytes.len() != total as usize {
            return Err(Refusal::Order);
        }
        match kind {
            InputKind::NativeScene => self.native_scene(&bytes),
            InputKind::NativeFace => self.native_face(&bytes),
            InputKind::Initialize => {
                let input: Initialize = data::decode(&bytes)?;
                drop(bytes);
                if input.uuid.len() != 36 {
                    return Err(Refusal::NamingUuid);
                }
                // Canonical admission validates choice counts, strings, identities, duplicates,
                // refusal/selection consistency and the exact UUID/naming catalog.
                let mut draft =
                    BirthDraft::new(input.uuid, input.choices).map_err(Refusal::from)?;
                if input.allow_idle_body {
                    draft = draft.allow_idle_body();
                }
                self.draft = Some(super::native::Controller::new(draft));
                Ok(Response::Ready {
                    revision: self.revision()?,
                })
            }
            InputKind::FaceBasis => {
                let input: FaceRequest = data::decode(&bytes)?;
                drop(bytes);
                data::validate_basis(&input.basis)?;
                let face = self
                    .current(input.revision)?
                    .face(&input.basis)
                    .map_err(face_error)?;
                self.publish(OutputKind::Face, &face)
            }
            InputKind::FaceInteraction => {
                let input: InteractionRequest = data::decode(&bytes)?;
                drop(bytes);
                data::validate_basis(&input.basis)?;
                self.current(input.revision)?;
                let outcome = self
                    .draft
                    .as_mut()
                    .ok_or(Refusal::Order)?
                    .apply_face_interaction(&input.basis, &input.show, &input.input)
                    .map_err(face_error)?;
                self.outcome(outcome)
            }
        }
    }
}
fn face_error(error: BirthFaceRefusal) -> Refusal {
    match error {
        BirthFaceRefusal::Draft(error) => error.into(),
        BirthFaceRefusal::Interaction(_) => Refusal::Interaction,
        _ => Refusal::Projection,
    }
}
