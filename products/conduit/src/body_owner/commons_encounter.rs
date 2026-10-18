//! Finite Commons preparation against the installed owner, not a second Body.
//! This assembly is deliberately not initial Workspace discovery. Its supported
//! proof entrance is the owner fixture, including the C3 linear/cross-Mask
//! journey. Installed Commons discovery and native walking remain separate.
use super::{debug, Owner};
use conduit_body::BodyState;
use conduit_core::{kind_id, HostAdvertisement, StructuredInfoValueShape};
use conduit_face::{
    Face, FaceActionAvailability, FaceBasis, FaceDisclosure, FaceDisclosureLevel, FaceInteraction,
    FaceInteractionContext, FaceProperty, FacePropertyValue, FaceRelationship,
    FaceRelationshipKind, FaceRole, FaceSubject, MaskShow,
};
use conduit_std_host::terminal_face_mask::TerminalMaskExecution;
use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;
use sha2::{Digest, Sha256};
use std::{io::Write, path::Path};
#[path = "commons_encounter/source.rs"]
mod source;
use source::{field, text, Source};

/// Only encounter selection and the last effect-acknowledged Show are retained.
/// No copy of Body state, owner actions or lifecycle reducer lives here.
pub(super) struct CommonsEncounter {
    source: Source,
    selected: String,
    generation: u64,
    shown: Option<MaskShow>,
    execution: Option<HostedTerminalMaskExecution>,
}
impl CommonsEncounter {
    pub(super) fn prepare() -> Result<Self, String> {
        let source = Source::prepare()?;
        let selected = text(field(field(&source.places, "hub")?, "identity")?)?.to_owned();
        Ok(Self {
            source,
            selected,
            generation: 0,
            shown: None,
            execution: None,
        })
    }

    pub(super) fn project(
        &mut self,
        owner: Option<&Owner>,
        host: &HostAdvertisement,
    ) -> Result<Face, String> {
        let (body_id, life, revision, original) = match owner {
            Some(owner) => {
                if owner.host.advertisement() != host {
                    return Err("commons/owner-host-changed".into());
                }
                let evidence = owner.session.evidence();
                let current = evidence.membership.parts.iter().any(|part| {
                    part.state == conduit_body::MembershipState::Admitted
                        && part.current.as_ref().is_some_and(|observation| {
                            observation.host_id == host.host_id
                                && observation.boot_id == host.boot_id
                                && observation.offer_generation == host.offer_generation
                        })
                });
                if !current {
                    return Err("commons/current-owner-observation-unavailable".into());
                }
                let life = match evidence.body.state {
                    BodyState::Lulled => "lulled",
                    BodyState::Awake { .. } => "awake",
                    BodyState::Fulfilled { .. } => "fulfilled",
                };
                (
                    evidence.body_id.as_str(),
                    life,
                    evidence.last_sequence(),
                    Some(owner.local_face_snapshot()?),
                )
            }
            None => ("", "none", 0, None),
        };
        let value = self.source.encounter(
            body_id,
            life,
            revision,
            &self.selected,
            &self.selected,
            false,
        )?;
        let selected = text(field(&value, "selected")?)?;
        let body_actions = matches!(field(&value, "body_actions")?.shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == [1]);
        let graph = field(&value, "places")?;
        let hub = field(graph, "hub")?;
        let mut face = match original {
            Some(face) => face,
            None => Face::new(
                0,
                FaceBasis {
                    body_id: None,
                    wake_id: None,
                    source_document_id: Some(self.source.checked.source_document_id.clone()),
                    checked_plot_id: Some(
                        self.source
                            .checked
                            .plots
                            .iter()
                            .find(|plot| plot.name == "commons/encounter")
                            .ok_or("commons/checked-root")?
                            .checked_plot_id
                            .clone(),
                    ),
                    expanded_plot_id: None,
                    plan_id: None,
                    active_play_id: None,
                    sign_ids: vec![],
                },
                vec![FaceSubject {
                    identity: text(field(hub, "identity")?)?.into(),
                    name: text(field(hub, "name")?)?.into(),
                    role: FaceRole::Semantic(kind_id(text(field(hub, "role")?)?)),
                }],
                vec![],
                vec![],
                vec![],
            )
            .map_err(debug)?,
        };
        for slot in ["hub", "workshop", "creche"] {
            let place = field(graph, slot)?;
            let identity = text(field(place, "identity")?)?;
            let subject = FaceSubject {
                identity: identity.into(),
                name: text(field(place, "name")?)?.into(),
                role: FaceRole::Semantic(kind_id(text(field(place, "role")?)?)),
            };
            match face
                .subjects
                .iter()
                .find(|existing| existing.identity == identity)
            {
                Some(existing) if *existing != subject => {
                    return Err("commons/place-identity-collision".into())
                }
                Some(_) => {}
                None => face.subjects.push(subject),
            }
            face.disclosures.push(FaceDisclosure {
                subject: identity.into(),
                level: FaceDisclosureLevel::Primary,
            });
            face.properties.push(FaceProperty {
                subject: identity.into(),
                name: "selected".into(),
                value: FacePropertyValue::Flag(identity == selected),
            });
        }
        for slot in ["containment", "reachability"] {
            let StructuredInfoValueShape::Collection(relations) = field(graph, slot)?.shape()
            else {
                return Err("commons/source-relations".into());
            };
            for relation in relations {
                face.relationships.push(FaceRelationship {
                    source: text(field(relation, "source")?)?.into(),
                    target: text(field(relation, "target")?)?.into(),
                    kind: FaceRelationshipKind::Semantic(kind_id(text(field(
                        relation, "meaning",
                    )?)?)),
                });
            }
        }
        for (name, value) in [
            (
                "commons/owner-life",
                text(field(field(&value, "owner")?, "life")?)?.to_owned(),
            ),
            (
                "commons/source",
                self.source.checked.source_document_id.as_str().to_owned(),
            ),
            ("commons/context-generation", self.generation.to_string()),
            ("commons/current-host", host.host_id.as_str().to_owned()),
            ("commons/current-boot", host.boot_id.as_str().to_owned()),
            (
                "commons/current-offers",
                host.offer_generation.0.to_string(),
            ),
            (
                "commons/current-advertisement",
                format!(
                    "sha256:{:x}",
                    Sha256::digest(serde_json::to_vec(host).map_err(debug)?)
                ),
            ),
        ] {
            face.properties.push(FaceProperty {
                subject: selected.into(),
                name: name.into(),
                value: FacePropertyValue::Text(value),
            });
        }
        let inspection = face.actions.iter().any(|action| {
            action.intent == "conduit.intent/open-inspection@1"
                && action.availability.is_available()
        });
        face.properties.push(FaceProperty {
            subject: selected.into(),
            name: "commons/inspection".into(),
            value: FacePropertyValue::Text(
                if !body_actions {
                    "context-unavailable"
                } else if inspection {
                    "owner-return-unavailable"
                } else {
                    "not-currently-offered"
                }
                .into(),
            ),
        });
        // Refer to the actual current owner's exact contract. The initial
        // vertical accepts its existing checked clock replacement operation;
        // unsupported return routes remain explicit, including generic Wake.
        for action in &mut face.actions {
            if action.availability.is_available() {
                let admitted =
                    owner.is_some_and(|owner| owner.commons_clock_change_admitted(action));
                if !body_actions || !admitted {
                    action.availability = FaceActionAvailability::Unavailable {
                        reason_code: if !body_actions {
                            "commons/context-withholds-owner-action"
                        } else {
                            "commons/current-owner-return-unavailable"
                        }
                        .into(),
                        explanation:
                            "This encounter has no currently admitted return for this owner action."
                                .into(),
                    };
                }
            }
        }
        let composition = face.composition;
        Face::new_with_semantics_and_temporal(
            face.revision,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
            face.actions,
            face.disclosures,
            face.temporal_references,
            face.temporal_facts,
        )
        .and_then(|face| face.with_composition(composition))
        .and_then(|face| {
            face.with_interaction_context(FaceInteractionContext {
                identity: format!("commons/context/{selected}/{}", self.generation),
                basis: vec![],
            })
        })
        .map_err(debug)
    }

    /// The source checks ENTER against authored edges; BACK is context return.
    /// Navigation can never call an owner lifecycle or workset operation.
    pub(super) fn navigate(
        &mut self,
        owner: Option<&Owner>,
        host: &HostAdvertisement,
        expected: &Face,
        target: &str,
        back: bool,
    ) -> Result<Face, String> {
        if self.project(owner, host)? != *expected {
            return Err("commons/stale-context".into());
        }
        let (body_id, life, revision) = match owner {
            Some(owner) => {
                let evidence = owner.session.evidence();
                (
                    evidence.body_id.as_str(),
                    match evidence.body.state {
                        BodyState::Lulled => "lulled",
                        BodyState::Awake { .. } => "awake",
                        BodyState::Fulfilled { .. } => "fulfilled",
                    },
                    evidence.last_sequence(),
                )
            }
            None => ("", "none", 0),
        };
        let result =
            self.source
                .encounter(body_id, life, revision, &self.selected, target, back)?;
        let selected = text(field(&result, "selected")?)?.to_owned();
        let generation = self
            .generation
            .checked_add(1)
            .ok_or("commons/context-exhausted")?;
        self.retire()?;
        // Projection refusal cannot commit even local selection.
        let old = std::mem::replace(&mut self.selected, selected);
        self.generation = generation;
        match self.project(owner, host) {
            Ok(face) => {
                self.shown = None;
                Ok(face)
            }
            Err(error) => {
                self.selected = old;
                self.generation -= 1;
                Err(error)
            }
        }
    }

    /// Actual software terminal effect acknowledgement, using the existing Mask
    /// execution entrance. No serialized Show or Mask-provided observation can
    /// install the private pending Show. This is not native scanout proof.
    pub(super) fn present(
        &mut self,
        owner: Option<&Owner>,
        host: &HostAdvertisement,
        output: &mut dyn Write,
    ) -> Result<(Face, MaskShow), String> {
        use conduit_std_host::terminal_face_mask::TerminalFaceMask;
        self.retire()?;
        let face = self.project(owner, host)?;
        let mut mask = TerminalFaceMask::prepare(face.clone(), 80, 24).map_err(debug)?;
        let mut execution = HostedTerminalMaskExecution::new(host).map_err(debug)?;
        mask.present(&mut execution, output).map_err(debug)?;
        let show = mask.show().ok_or("commons/missing-show")?.clone();
        self.shown = Some(show.clone());
        self.execution = Some(execution);
        Ok((face, show))
    }

    fn retire(&mut self) -> Result<(), String> {
        self.shown = None;
        if let Some(mut execution) = self.execution.take() {
            execution
                .cancel()
                .map_err(|error| format!("commons/retire-failed:{error:?}"))?;
        }
        Ok(())
    }

    pub(super) fn apply_clock_change(
        &mut self,
        owner: &mut Owner,
        root: &Path,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<serde_json::Value, String> {
        if self.shown.as_ref() != Some(show) {
            return Err("commons/show-not-current".into());
        }
        let host = owner.host.advertisement().clone();
        let face = self.project(Some(owner), &host)?;
        interaction
            .validate_against(&face, show)
            .map_err(|error| format!("commons/interaction:{error:?}"))?;
        let offered = face
            .resolve_action(face.revision, &interaction.action_id)
            .map_err(debug)?;
        let original = owner.local_face_snapshot()?;
        let action = original
            .resolve_action(original.revision, &interaction.action_id)
            .map_err(debug)?;
        if action != offered || !owner.commons_clock_change_admitted(action) {
            return Err("commons/owner-action-changed".into());
        }
        self.shown = None; // one accepted request; failure also retires custody
        let mut execution = self
            .execution
            .take()
            .ok_or("commons/missing-current-producer")?;
        let correlated = execution
            .interact(interaction.clone())
            .map_err(|error| format!("commons/interaction-effect:{error:?}"))?;
        owner.apply_admitted_clock_interval(root, &correlated.interaction)
    }
}
impl Owner {
    fn commons_clock_change_admitted(&self, action: &conduit_face::FaceAction) -> bool {
        if action.intent != super::clock_interval::CLOCK_INTERVAL_ACTION
            || !action.availability.is_available()
            || self.host.is_playing()
            || self.session.realization().is_some()
            || self.session.evidence().body.state != BodyState::Lulled
        {
            return false;
        }
        // Exact current Host/Boot/offer feasibility, before advertising the
        // existing operation. Applying it repeats its retained-source checks.
        let Some(interval) = self.clock_interval_ms else {
            return false;
        };
        let Some(resident) = self.resident.as_ref() else {
            return false;
        };
        [
            include_str!("../../../../plots/clock/main.conduit").to_owned(),
            super::clock_interval::source_for(interval),
        ]
        .into_iter()
        .any(|source| {
            let Ok(plot) = crate::plot_source::parse(&source)
                .and_then(|source| source.expand_entry_for_authoring())
            else {
                return false;
            };
            plot.expanded.source_document_id == resident.source_document_id
                && plot.expanded.checked_plot_id == resident.checked_plot_id
                && self.plan_partition(&plot, resident).is_ok()
        })
    }
}
#[cfg(test)]
#[path = "commons_encounter/tests.rs"]
mod tests;
