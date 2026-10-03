//! One service-owned zero-Body Crèche encounter on the installed Host Boot.
//!
//! The client supplies typed Face interactions, never a replacement selection
//! or Body. The accepted Birth is journaled before the owner is exposed.

use super::{DurableHostRuntime, HostSource};
use conduit_birth_plot::{BirthActionOutcome, BirthDraft, HostOwnedBirthFaceBasis};
use conduit_patchbay_workbench::{PatchbayModel, ZeroBodyFrontDoor};
use conduit_presentation::{FaceInteraction, MaskShow, Presentation};
use patchbay_hosted::HostedPatchbayAdapter;
use std::{path::Path, sync::Arc};

pub(super) struct ServiceBirth {
    door: ZeroBodyFrontDoor,
    draft: BirthDraft,
    basis: HostOwnedBirthFaceBasis,
}

pub(crate) enum BirthTransition {
    Changed(Presentation),
    Born {
        body_id: conduit_body::BodyId,
        presentation: Presentation,
    },
}

impl DurableHostRuntime {
    pub(super) fn birth_face(
        &mut self,
        root: &Path,
    ) -> Result<(Presentation, conduit_core::HostAdvertisement), String> {
        refuse_pending_birth_publication(root)?;
        let HostSource::Bare(host) = &self.host else {
            return Err("installed Host already belongs to a Body".into());
        };
        if crate::durable_host::has_current_body(root)? {
            return Err("installed Host already retains a Body".into());
        }
        if self.birth.is_none() {
            let advertisement = host.advertisement().clone();
            let encounter_id = crate::birth_identity::fresh_uuid()?;
            let door = ZeroBodyFrontDoor::from_model(
                Arc::new(HostedPatchbayAdapter),
                PatchbayModel::from_advertisement(advertisement.clone()),
            )?;
            let draft = door.creche_draft(encounter_id.clone())?;
            self.birth = Some(ServiceBirth {
                door,
                draft,
                basis: HostOwnedBirthFaceBasis {
                    host_id: advertisement.host_id,
                    boot_id: advertisement.boot_id,
                    encounter_id,
                },
            });
        }
        let state = self.birth.as_ref().ok_or("Birth encounter was lost")?;
        let face = state.draft.host_owned_face(&state.basis).map_err(debug)?;
        Ok((face, self.host.advertisement().clone()))
    }

    pub(super) fn birth_interaction(
        &mut self,
        root: &Path,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<BirthTransition, String> {
        refuse_pending_birth_publication(root)?;
        let HostSource::Bare(host) = &self.host else {
            return Err("installed Host already belongs to a Body".into());
        };
        if crate::durable_host::has_current_body(root)? {
            return Err("installed Host already retains a Body".into());
        }
        let state = self
            .birth
            .as_mut()
            .ok_or("Birth encounter was not opened")?;
        let advertisement = host.advertisement().clone();
        if state.basis.host_id != advertisement.host_id
            || state.basis.boot_id != advertisement.boot_id
        {
            return Err("Birth encounter belongs to a stale Host Boot".into());
        }
        match state
            .draft
            .apply_host_owned_face_interaction(&state.basis, show, interaction)
            .map_err(debug)?
        {
            BirthActionOutcome::Changed => state
                .draft
                .host_owned_face(&state.basis)
                .map(BirthTransition::Changed)
                .map_err(debug),
            BirthActionOutcome::Birth(selection) => {
                if selection.workset.plots().len() > 1 {
                    return Err("installed-birth-multiple-plots-unsupported".into());
                }
                let primary_source = state
                    .door
                    .primary_selected_source(&selection)?
                    .map(str::to_owned);
                let checked_source = primary_source
                    .as_deref()
                    .map(|source| crate::plot_source::parse(source)?.expand_entry_for_authoring())
                    .transpose()?;
                let friendly_name = selection.friendly_name.clone();
                // Clone the reviewed inventory so a refusal before journaling
                // does not consume this still-current encounter.
                let born = state
                    .door
                    .clone()
                    .birth_from_creche(selection, state.door.revision())?;
                let prepared = crate::durable_host::owner::Owner::prepare_born(
                    born.body().clone(),
                    friendly_name,
                    &advertisement,
                )?;
                let body_id = prepared.body_id().clone();
                // Publication can fail after its recoverable journal is written.
                // The caller must inspect current owner truth, never retry Birth.
                prepared
                    .retain(root, primary_source.as_deref().map(str::as_bytes))
                    .map_err(|_| super::CONTROL_OUTCOME_UNKNOWN.to_owned())?;
                let old = std::mem::replace(&mut self.host, HostSource::Transitioning);
                let HostSource::Bare(host) = old else {
                    self.host = old;
                    return Err(super::CONTROL_OUTCOME_UNKNOWN.into());
                };
                let mut owner =
                    crate::durable_host::owner::Owner::from_prepared_born(*host, prepared);
                let presented = (|| {
                    if let Some(checked) = &checked_source {
                        owner.set_resident_plot_name(checked)?;
                    }
                    owner.local_face_snapshot()
                })();
                self.host = HostSource::Body {
                    owner: Box::new(owner),
                    root: root.to_path_buf(),
                    running: None,
                };
                self.birth = None;
                let presentation =
                    presented.map_err(|_| super::CONTROL_OUTCOME_UNKNOWN.to_owned())?;
                Ok(BirthTransition::Born {
                    body_id,
                    presentation,
                })
            }
        }
    }
}

fn refuse_pending_birth_publication(root: &Path) -> Result<(), String> {
    if root
        .join("body/owner-transaction.json")
        .try_exists()
        .map_err(|_| super::CONTROL_OUTCOME_UNKNOWN.to_owned())?
    {
        return Err(super::CONTROL_OUTCOME_UNKNOWN.into());
    }
    Ok(())
}

fn debug(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(unix)]
pub(crate) fn face(
    state_dir: &Path,
) -> Result<(Presentation, conduit_core::HostAdvertisement), String> {
    use super::{Request, Response, PROTOCOL};
    match super::body::call(
        state_dir,
        Request::BirthFace {
            protocol: PROTOCOL,
            token: super::body::token(state_dir)?,
        },
    )? {
        Response::BirthFace {
            protocol: PROTOCOL,
            presentation,
            advertisement,
        } => Ok((*presentation, advertisement)),
        Response::Refused { code, .. } if code == super::CONTROL_OUTCOME_UNKNOWN => Err(code),
        Response::Refused { code, .. } => Err(format!("installed Birth refused Face: {code}")),
        _ => Err("installed Host returned the wrong Birth Face response".into()),
    }
}

#[cfg(unix)]
pub(crate) fn interact(
    state_dir: &Path,
    show: MaskShow,
    interaction: FaceInteraction,
) -> Result<BirthTransition, String> {
    use super::{Request, Response, PROTOCOL};
    match super::body::call(
        state_dir,
        Request::BirthInteraction {
            protocol: PROTOCOL,
            token: super::body::token(state_dir)?,
            show: Box::new(show),
            interaction,
        },
    )? {
        Response::BirthChanged {
            protocol: PROTOCOL,
            presentation,
        } => Ok(BirthTransition::Changed(*presentation)),
        Response::BirthCompleted {
            protocol: PROTOCOL,
            body_id,
            presentation,
        } => Ok(BirthTransition::Born {
            body_id,
            presentation: *presentation,
        }),
        Response::Refused { code, .. } if code == super::CONTROL_OUTCOME_UNKNOWN => Err(code),
        Response::Refused { code, .. } => Err(format!("installed Birth refused action: {code}")),
        _ => Err("installed Host returned the wrong Birth action response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn face(
    _state_dir: &Path,
) -> Result<(Presentation, conduit_core::HostAdvertisement), String> {
    Err("installed screen-free Birth requires local Unix control".into())
}

#[cfg(not(unix))]
pub(crate) fn interact(
    _state_dir: &Path,
    _show: MaskShow,
    _interaction: FaceInteraction,
) -> Result<BirthTransition, String> {
    Err("installed screen-free Birth requires local Unix control".into())
}
