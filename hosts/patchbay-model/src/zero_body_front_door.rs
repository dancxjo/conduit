//! Truthful Patchbay WORLD state for one Host that bears no Body.

use conduit_body::{Body, BodyMembership, MembershipProofId, SeedId, Wake};
use conduit_core::{BootId, CheckedFormId, HostId, SignId, SourceDocumentId};
use conduit_presentation::Presentation;

use crate::{FormEditor, LocalFrontDoor, PatchbayModel};

pub const MAX_FRONT_DOOR_BODY_CANDIDATES: usize = 16;
pub const MAX_FRONT_DOOR_SEEDS: usize = 16;
pub const MAX_FRONT_DOOR_REFUSAL_SIGNS: usize = 16;

#[derive(Clone)]
pub(super) struct FrontDoorRefusalSign {
    pub sign_id: SignId,
    pub code: String,
}

#[derive(Clone)]
pub struct BodyJoinCandidate {
    pub label: String,
    pub body: Body,
    pub wake: Wake,
    pub membership: BodyMembership,
    pub editor: FormEditor,
    pub proof_id: MembershipProofId,
    pub evidence_sign: SignId,
    pub freshness_sequence: u64,
}

impl BodyJoinCandidate {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        label: impl Into<String>,
        body: Body,
        wake: Wake,
        membership: BodyMembership,
        editor: FormEditor,
        proof_id: MembershipProofId,
        evidence_sign: SignId,
        freshness_sequence: u64,
    ) -> Result<Self, String> {
        body.validate().map_err(|error| error.to_string())?;
        wake.validate().map_err(|error| error.to_string())?;
        membership
            .validate()
            .map_err(|error| format!("{error:?}"))?;
        if body.body_id != wake.body_id
            || body.body_id != membership.body_id
            || editor.view().checked.source_document_id.as_ref() != Some(&body.source_document_id)
            || !editor
                .view()
                .checked
                .forms
                .iter()
                .any(|form| form.checked_form_id == body.checked_form_id)
        {
            return Err("Body join candidate identity chain is inconsistent".into());
        }
        let label = label.into();
        if label.is_empty() || freshness_sequence == 0 {
            return Err("Body join candidate label and freshness must be present".into());
        }
        Ok(Self {
            label,
            body,
            wake,
            membership,
            editor,
            proof_id,
            evidence_sign,
            freshness_sequence,
        })
    }
}

#[derive(Clone)]
pub struct SeedCandidate {
    pub label: String,
    pub seed_id: SeedId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub source_name: String,
    pub source: String,
    pub provenance: String,
    pub evidence_sign: SignId,
    pub freshness_sequence: u64,
}

impl SeedCandidate {
    pub fn from_source(
        label: impl Into<String>,
        source_name: impl Into<String>,
        source: impl Into<String>,
        provenance: impl Into<String>,
        evidence_sign: SignId,
        freshness_sequence: u64,
    ) -> Result<Self, String> {
        let label = label.into();
        let source_name = source_name.into();
        let source = source.into();
        let provenance = provenance.into();
        if label.is_empty() || provenance.is_empty() || freshness_sequence == 0 {
            return Err("Seed label, provenance, and freshness must be present".into());
        }
        let editor = FormEditor::from_source(source_name.clone().into(), source.clone())
            .map_err(|error| error.to_string())?;
        let source_document_id = editor
            .view()
            .checked
            .source_document_id
            .clone()
            .ok_or("Seed source is unchecked")?;
        let checked_form_id = editor
            .view()
            .checked
            .forms
            .first()
            .ok_or("Seed source contains no Form")?
            .checked_form_id
            .clone();
        let seed_id = SeedId::bind(&source_document_id, &checked_form_id);
        Ok(Self {
            label,
            seed_id,
            source_document_id,
            checked_form_id,
            source_name,
            source,
            provenance,
            evidence_sign,
            freshness_sequence,
        })
    }

    pub(super) fn editor(&self) -> Result<FormEditor, String> {
        FormEditor::from_source(self.source_name.clone().into(), self.source.clone())
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenedFrontDoorSubject {
    Body {
        body_id: conduit_body::BodyId,
        observed_at: u64,
    },
    Seed {
        seed_id: SeedId,
        observed_at: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZeroBodyFrontDoorProjection {
    pub presentation: Presentation,
}

#[derive(Clone)]
pub struct ZeroBodyFrontDoor {
    pub(super) model: PatchbayModel,
    pub(super) body_candidates: Vec<BodyJoinCandidate>,
    pub(super) seeds: Vec<SeedCandidate>,
    pub(super) opened: Option<OpenedFrontDoorSubject>,
    pub(super) refusals: Vec<FrontDoorRefusalSign>,
    pub(super) revision: u64,
}

impl ZeroBodyFrontDoor {
    pub fn fresh() -> Result<Self, String> {
        let model = PatchbayModel::fresh_with_composition(
            conduit_std_host::StdHostComposition::minimal().with_text(),
        );
        Self::from_model(model)
    }

    pub fn with_identity(host_id: HostId, boot_id: BootId) -> Result<Self, String> {
        Self::from_model(PatchbayModel::with_identity_and_composition(
            host_id,
            boot_id,
            conduit_std_host::StdHostComposition::minimal().with_text(),
        ))
    }

    pub fn from_model(model: PatchbayModel) -> Result<Self, String> {
        let seed = SeedCandidate::from_source(
            "Patchbay entrance specimen",
            "patchbay-front-door.conduit",
            include_str!("../../../examples/patchbay-front-door.conduit"),
            "repository example; opening is inert and BE BORN remains explicit",
            SignId::from("patchbay/front-door/seed-available"),
            1,
        )?;
        Ok(Self {
            model,
            body_candidates: Vec::new(),
            seeds: vec![seed],
            opened: None,
            refusals: Vec::new(),
            revision: 1,
        })
    }

    pub fn advertisement(&self) -> &conduit_core::HostAdvertisement {
        self.model.advertisement()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn opened(&self) -> Option<&OpenedFrontDoorSubject> {
        self.opened.as_ref()
    }

    pub fn seed_ids(&self) -> Vec<SeedId> {
        self.seeds.iter().map(|seed| seed.seed_id.clone()).collect()
    }

    pub fn record_refusal(&mut self, code: &str) -> Result<SignId, String> {
        if !matches!(code, "StalePresentation" | "StaleDiscovery" | "StaleSeed") {
            return Err("unsupported front-door refusal code".into());
        }
        if self.refusals.len() == MAX_FRONT_DOOR_REFUSAL_SIGNS {
            return Err("front-door refusal Sign capacity exhausted".into());
        }
        let sign_id = SignId::from(format!(
            "patchbay/front-door/refused/{}/{code}",
            self.revision
        ));
        self.refusals.push(FrontDoorRefusalSign {
            sign_id: sign_id.clone(),
            code: code.into(),
        });
        self.advance()?;
        Ok(sign_id)
    }

    pub fn observe_body_candidate(&mut self, candidate: BodyJoinCandidate) -> Result<(), String> {
        if self.body_candidates.len() == MAX_FRONT_DOOR_BODY_CANDIDATES {
            return Err("front-door Body candidate capacity exhausted".into());
        }
        if self
            .body_candidates
            .iter()
            .any(|value| value.body.body_id == candidate.body.body_id)
        {
            return Err("front-door Body candidate is already observed".into());
        }
        self.body_candidates.push(candidate);
        self.advance()
    }

    pub fn add_seed(&mut self, seed: SeedCandidate) -> Result<(), String> {
        if self.seeds.len() == MAX_FRONT_DOOR_SEEDS {
            return Err("front-door Seed capacity exhausted".into());
        }
        if self.seeds.iter().any(|value| value.seed_id == seed.seed_id) {
            return Err("front-door Seed is already available".into());
        }
        self.seeds.push(seed);
        self.advance()
    }

    pub fn open_body(
        &mut self,
        body_id: &conduit_body::BodyId,
        revision: u64,
    ) -> Result<(), String> {
        self.require_revision(revision)?;
        let candidate = self
            .body_candidates
            .iter()
            .find(|candidate| &candidate.body.body_id == body_id)
            .ok_or("unknown Body candidate")?;
        self.opened = Some(OpenedFrontDoorSubject::Body {
            body_id: body_id.clone(),
            observed_at: candidate.freshness_sequence,
        });
        self.advance()
    }

    pub fn open_seed(&mut self, seed_id: &SeedId, revision: u64) -> Result<(), String> {
        self.require_revision(revision)?;
        let seed = self
            .seeds
            .iter()
            .find(|seed| &seed.seed_id == seed_id)
            .ok_or("unknown Seed")?;
        self.opened = Some(OpenedFrontDoorSubject::Seed {
            seed_id: seed_id.clone(),
            observed_at: seed.freshness_sequence,
        });
        self.advance()
    }

    pub fn open_subject(&mut self, subject: &str, revision: u64) -> Result<(), String> {
        if let Some(candidate) = self
            .body_candidates
            .iter()
            .find(|candidate| format!("body/{}", candidate.body.body_id.as_str()) == subject)
        {
            let body_id = candidate.body.body_id.clone();
            return self.open_body(&body_id, revision);
        }
        if let Some(seed) = self
            .seeds
            .iter()
            .find(|seed| format!("seed/{}", seed.seed_id.as_str()) == subject)
        {
            let seed_id = seed.seed_id.clone();
            return self.open_seed(&seed_id, revision);
        }
        Err("OPEN requires a current Body or Seed subject".into())
    }

    pub fn join_open_body(self, revision: u64) -> Result<LocalFrontDoor, String> {
        self.require_revision(revision)?;
        let OpenedFrontDoorSubject::Body {
            body_id,
            observed_at,
        } = self.opened.clone().ok_or("JOIN requires an opened Body")?
        else {
            return Err("JOIN requires an opened Body".into());
        };
        let candidate = self
            .body_candidates
            .into_iter()
            .find(|candidate| candidate.body.body_id == body_id)
            .ok_or("opened Body is no longer available")?;
        if candidate.freshness_sequence != observed_at {
            return Err("opened Body observation is stale".into());
        }
        LocalFrontDoor::join_existing(self.model, candidate, self.revision)
    }

    pub fn be_born(self, revision: u64) -> Result<LocalFrontDoor, String> {
        self.require_revision(revision)?;
        let OpenedFrontDoorSubject::Seed {
            seed_id,
            observed_at,
        } = self
            .opened
            .clone()
            .ok_or("BE BORN requires an opened Seed")?
        else {
            return Err("BE BORN requires an opened Seed".into());
        };
        let seed = self
            .seeds
            .into_iter()
            .find(|candidate| candidate.seed_id == seed_id)
            .ok_or("opened Seed is no longer available")?;
        if seed.freshness_sequence != observed_at {
            return Err("opened Seed observation is stale".into());
        }
        LocalFrontDoor::born_from_seed(self.model, seed, self.revision)
    }

    fn require_revision(&self, revision: u64) -> Result<(), String> {
        if revision != self.revision {
            Err(format!(
                "stale front-door revision {revision}; current is {}",
                self.revision
            ))
        } else {
            Ok(())
        }
    }

    fn advance(&mut self) -> Result<(), String> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("front-door presentation revision exhausted")?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "zero_body_front_door_tests.rs"]
mod tests;
