//! Canonical Face encounter for preparing another Body from an existing Body.
use crate::{
    DeploymentDraft, DeploymentDraftRefusal, DeploymentImageSource, DeploymentScope,
    ReviewedDeployment,
};
use conduit_birth_plot::{BirthFaceBasis, BirthFaceRefusal};
use conduit_core::{BootId, HostId, PlotIdentity};
use conduit_face::{
    ApplicationEventKind, Face, FaceBasis, FaceInteraction, FaceInteractionRefusal, MaskShow,
};
use conduit_host_make::{CheckedHostConfiguration, MakeCatalog, MakePackageSet};
mod assignments;
mod projection;
pub use assignments::MAX_DEPLOYMENT_PROFILES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentPage {
    Body,
    Hosts,
    Make,
}

/// Actual producing execution in the current Body, supplied by its owner.
/// The Body being prepared is not this producing Body.
pub struct DeploymentEncounterBasis {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub encounter_id: String,
    pub execution: FaceBasis,
}

#[derive(Debug)]
pub enum DeploymentEncounterRefusal {
    InvalidBasis,
    InvalidCatalogue,
    InvalidHostName,
    RevisionExhausted,
    InvalidPage,
    Projection(conduit_face::FaceError),
    BirthFace(BirthFaceRefusal),
    Interaction(FaceInteractionRefusal),
    Draft(DeploymentDraftRefusal),
}

pub enum DeploymentEncounterOutcome {
    Changed,
    /// Exact review for the owner to prepare once through its make operation.
    /// This does not report successful artifact production.
    Reviewed(ReviewedDeployment),
}

pub struct DeploymentEncounter {
    draft: DeploymentDraft,
    profiles: Vec<CheckedHostConfiguration>,
    page: DeploymentPage,
    revision: u64,
}

impl DeploymentEncounterBasis {
    fn birth_basis(&self) -> Result<BirthFaceBasis, DeploymentEncounterRefusal> {
        let e = &self.execution;
        if e.body_id.is_none() {
            return Err(DeploymentEncounterRefusal::InvalidBasis);
        }
        Ok(BirthFaceBasis {
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            encounter_id: self.encounter_id.clone(),
            producer_plot: PlotIdentity {
                source_document_id: e
                    .source_document_id
                    .clone()
                    .ok_or(DeploymentEncounterRefusal::InvalidBasis)?,
                checked_plot_id: e
                    .checked_plot_id
                    .clone()
                    .ok_or(DeploymentEncounterRefusal::InvalidBasis)?,
                expanded_plot_id: e
                    .expanded_plot_id
                    .clone()
                    .ok_or(DeploymentEncounterRefusal::InvalidBasis)?,
            },
            producer_plan_id: e
                .plan_id
                .clone()
                .ok_or(DeploymentEncounterRefusal::InvalidBasis)?,
        })
    }
}

impl DeploymentEncounter {
    pub fn new(draft: DeploymentDraft) -> Self {
        let mut profiles = draft
            .construction()
            .hosts()
            .iter()
            .map(|h| h.configuration.clone())
            .collect::<Vec<_>>();
        profiles.sort_by(|a, b| a.configuration_id().cmp(b.configuration_id()));
        profiles.dedup_by(|a, b| a.configuration_id() == b.configuration_id());
        Self {
            draft,
            profiles,
            page: DeploymentPage::Body,
            revision: 1,
        }
    }
    pub fn draft(&self) -> &DeploymentDraft {
        &self.draft
    }
    pub fn page(&self) -> DeploymentPage {
        self.page
    }
    pub fn into_draft(self) -> DeploymentDraft {
        self.draft
    }

    pub fn face(
        &self,
        basis: &DeploymentEncounterBasis,
    ) -> Result<Face, DeploymentEncounterRefusal> {
        projection::project(self, basis)
    }

    /// Always validate against the current owner projection and acknowledged
    /// Show before interpreting an action; caller-supplied Faces are not truth.
    pub fn apply_interaction(
        &mut self,
        basis: &DeploymentEncounterBasis,
        show: &MaskShow,
        input: &FaceInteraction,
        catalog: &MakeCatalog,
        packages: &MakePackageSet,
    ) -> Result<DeploymentEncounterOutcome, DeploymentEncounterRefusal> {
        use DeploymentEncounterRefusal as E;
        let face = self.face(basis)?;
        input
            .validate_against(&face, show)
            .map_err(E::Interaction)?;
        let next = self.revision.checked_add(1).ok_or(E::RevisionExhausted)?;
        let value = match input.arguments.first() {
            None => "",
            Some(a) if a.value_kind == "value/bool" => {
                if a.value == [1] {
                    "true"
                } else {
                    "false"
                }
            }
            Some(a) => std::str::from_utf8(&a.value)
                .map_err(|_| E::Interaction(FaceInteractionRefusal::MalformedEncoding))?,
        };
        match input.action_id.as_str() {
            "creche.deploy.next" => {
                self.page = match self.page {
                    DeploymentPage::Body => {
                        self.draft
                            .birth()
                            .selection(self.draft.birth().revision())
                            .map_err(|e| E::Draft(DeploymentDraftRefusal::BirthDraft(e)))?;
                        DeploymentPage::Hosts
                    }
                    DeploymentPage::Hosts => DeploymentPage::Make,
                    DeploymentPage::Make => return Err(E::InvalidPage),
                }
            }
            "creche.deploy.back" => {
                self.page = match self.page {
                    DeploymentPage::Hosts => DeploymentPage::Body,
                    DeploymentPage::Make => DeploymentPage::Hosts,
                    DeploymentPage::Body => return Err(E::InvalidPage),
                }
            }
            "creche.deploy.first" => self
                .draft
                .select_hosts(self.draft.revision(), value, self.draft.scope())
                .map_err(E::Draft)?,
            "creche.deploy.scope" => {
                let scope = match value {
                    "first" => DeploymentScope::FirstHost,
                    "all" => DeploymentScope::AllHosts,
                    _ => return Err(E::InvalidPage),
                };
                let first = self.draft.first_host().to_owned();
                self.draft
                    .select_hosts(self.draft.revision(), &first, scope)
                    .map_err(E::Draft)?;
            }
            "creche.deploy.source" => {
                let source = match value {
                    "release" => DeploymentImageSource::ReviewedRelease,
                    "build" => DeploymentImageSource::SourceBuild,
                    _ => return Err(E::InvalidPage),
                };
                self.draft
                    .select_image_source(self.draft.revision(), source)
                    .map_err(E::Draft)?;
            }
            "creche.deploy.review" => {
                return self
                    .draft
                    .review(self.draft.revision(), packages)
                    .map(DeploymentEncounterOutcome::Reviewed)
                    .map_err(E::Draft);
            }
            action if action.starts_with("creche.deploy.host-name.") => {
                let index = action
                    .strip_prefix("creche.deploy.host-name.")
                    .and_then(|v| v.parse().ok())
                    .ok_or(E::InvalidPage)?;
                self.rename_host(index, value, catalog, packages)?;
            }
            action if action.starts_with("creche.deploy.profile.") => {
                let index = action
                    .strip_prefix("creche.deploy.profile.")
                    .and_then(|v| v.parse().ok())
                    .ok_or(E::InvalidPage)?;
                self.select_profile(index, value, catalog, packages)?;
            }
            action => {
                let event = if action == "creche.name" || action == "creche.search" {
                    ApplicationEventKind::Input
                } else if action == "creche.suggest" {
                    ApplicationEventKind::Activate
                } else {
                    ApplicationEventKind::Change
                };
                self.draft
                    .apply_birth_event(self.draft.revision(), action, event, value)
                    .map_err(E::Draft)?;
            }
        }
        self.revision = next;
        Ok(DeploymentEncounterOutcome::Changed)
    }
}
