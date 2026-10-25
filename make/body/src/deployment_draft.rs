//! Reviewed deployment choices for a new Body, distinct from artifact effects.
use std::collections::BTreeMap;

use conduit_birth_plot::{
    BirthActionOutcome, BirthActions, BirthDraft, BirthDraftRefusal, BirthSelection,
};
use conduit_core::SignId;
use conduit_face::ApplicationEventKind;
use conduit_host_make::{
    HostConfiguration, HostProfile, MakeBuildSelection, MakeCatalog, MakePackageSet,
    PackageCompositionDiagnostic,
};

use crate::{
    check_body_description, BodyDescriptionDiagnostic, CheckedBodyDescription,
    PreparedBirthRefusal, PreparedBodyBirth,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentScope {
    FirstHost,
    AllHosts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentImageSource {
    ReviewedRelease,
    SourceBuild,
}

#[derive(Debug)]
pub enum DeploymentDraftRefusal {
    StaleReview,
    RevisionExhausted,
    UnknownHost,
    BirthActionNotAvailable,
    BirthDraft(BirthDraftRefusal),
    Birth(PreparedBirthRefusal),
    Construction(Vec<BodyDescriptionDiagnostic>),
    Package(PackageCompositionDiagnostic),
}

/// The encounter owns one draft; the existing Body running Crèche is separate.
/// Host assignments are already checked construction intent, never live Hosts.
pub struct DeploymentDraft {
    birth: BirthDraft,
    construction: CheckedBodyDescription,
    original_host_names: BTreeMap<String, String>,
    first_host: String,
    scope: DeploymentScope,
    source: DeploymentImageSource,
    revision: u64,
}

/// Exact output of the make review. There are no process or device grants here.
pub struct ReviewedDeployment {
    revision: u64,
    construction_id: String,
    selection: BirthSelection,
    profiles: Vec<HostProfile>,
    make_selections: Vec<MakeBuildSelection>,
    first_host: String,
    scope: DeploymentScope,
    source: DeploymentImageSource,
}

/// Born once; every selected target receives the same ordinary biography.
/// No image bytes, boot observation or membership are implied by preparation.
pub struct PreparedDeployment {
    birth: PreparedBodyBirth,
    construction: CheckedBodyDescription,
    first_host: String,
    selected_hosts: Vec<String>,
    image_source: DeploymentImageSource,
}

impl PreparedDeployment {
    pub fn birth(&self) -> &PreparedBodyBirth {
        &self.birth
    }
    pub fn construction(&self) -> &CheckedBodyDescription {
        &self.construction
    }
    pub fn first_host(&self) -> &str {
        &self.first_host
    }
    pub fn selected_hosts(&self) -> &[String] {
        &self.selected_hosts
    }
    pub fn image_source(&self) -> DeploymentImageSource {
        self.image_source
    }
}

impl DeploymentDraft {
    pub fn new(
        birth: BirthDraft,
        construction: CheckedBodyDescription,
        first_host: String,
    ) -> Result<Self, DeploymentDraftRefusal> {
        if !construction
            .hosts()
            .iter()
            .any(|h| h.description.name == first_host)
        {
            return Err(DeploymentDraftRefusal::UnknownHost);
        }
        let original_host_names = construction
            .hosts()
            .iter()
            .map(|host| (host.description.name.clone(), host.description.name.clone()))
            .collect();
        Ok(Self {
            birth,
            construction,
            original_host_names,
            first_host,
            scope: DeploymentScope::FirstHost,
            source: DeploymentImageSource::ReviewedRelease,
            revision: 1,
        })
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn birth(&self) -> &BirthDraft {
        &self.birth
    }
    pub fn construction(&self) -> &CheckedBodyDescription {
        &self.construction
    }
    /// Original declared entry for Host-owned grants; display renames grant no authority.
    pub fn original_host_name(&self, current: &str) -> Option<&str> {
        self.original_host_names.get(current).map(String::as_str)
    }

    pub(crate) fn replace_renamed_host(
        &mut self,
        revision: u64,
        construction: CheckedBodyDescription,
        first_host: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), DeploymentDraftRefusal> {
        let original = self
            .original_host_names
            .get(old_name)
            .cloned()
            .ok_or(DeploymentDraftRefusal::UnknownHost)?;
        self.replace_construction(revision, construction, first_host)?;
        self.original_host_names.remove(old_name);
        self.original_host_names.insert(new_name.into(), original);
        Ok(())
    }

    pub fn first_host(&self) -> &str {
        &self.first_host
    }
    pub fn scope(&self) -> DeploymentScope {
        self.scope
    }
    pub fn image_source(&self) -> DeploymentImageSource {
        self.source
    }

    /// Reuse the existing bounded naming/Plot protocol, without permitting its
    /// zero-Body Birth action to bypass deployment review.
    pub fn apply_birth_event(
        &mut self,
        revision: u64,
        action: &str,
        event: ApplicationEventKind,
        value: &str,
    ) -> Result<(), DeploymentDraftRefusal> {
        let next = self.next_revision(revision)?;
        if action == "creche.birth" {
            return Err(DeploymentDraftRefusal::BirthActionNotAvailable);
        }
        match self
            .birth
            .apply_event(self.birth.revision(), action, event, value)
            .map_err(DeploymentDraftRefusal::BirthDraft)?
        {
            BirthActionOutcome::Changed => self.revision = next,
            BirthActionOutcome::Birth(_) => {
                return Err(DeploymentDraftRefusal::BirthActionNotAvailable)
            }
        }
        Ok(())
    }

    pub fn select_hosts(
        &mut self,
        revision: u64,
        first_host: &str,
        scope: DeploymentScope,
    ) -> Result<(), DeploymentDraftRefusal> {
        let next = self.next_revision(revision)?;
        if !self
            .construction
            .hosts()
            .iter()
            .any(|h| h.description.name == first_host)
        {
            return Err(DeploymentDraftRefusal::UnknownHost);
        }
        self.first_host = first_host.into();
        self.scope = scope;
        self.revision = next;
        Ok(())
    }

    pub fn select_image_source(
        &mut self,
        revision: u64,
        source: DeploymentImageSource,
    ) -> Result<(), DeploymentDraftRefusal> {
        let next = self.next_revision(revision)?;
        self.source = source;
        self.revision = next;
        Ok(())
    }

    /// Target/name edits go through existing construction validation before
    /// replacement. Failed validation leaves the current encounter untouched.
    pub fn replace_construction(
        &mut self,
        revision: u64,
        construction: CheckedBodyDescription,
        first_host: &str,
    ) -> Result<(), DeploymentDraftRefusal> {
        let next = self.next_revision(revision)?;
        if !construction
            .hosts()
            .iter()
            .any(|h| h.description.name == first_host)
        {
            return Err(DeploymentDraftRefusal::UnknownHost);
        }
        self.original_host_names = construction
            .hosts()
            .iter()
            .map(|host| {
                let name = &host.description.name;
                (
                    name.clone(),
                    self.original_host_names
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| name.clone()),
                )
            })
            .collect();
        self.construction = construction;
        self.first_host = first_host.into();
        self.revision = next;
        Ok(())
    }

    pub fn review(
        &self,
        revision: u64,
        packages: &MakePackageSet,
    ) -> Result<ReviewedDeployment, DeploymentDraftRefusal> {
        if revision != self.revision {
            return Err(DeploymentDraftRefusal::StaleReview);
        }
        let selection = self
            .birth
            .selection(self.birth.revision())
            .map_err(DeploymentDraftRefusal::BirthDraft)?;
        Ok(ReviewedDeployment {
            revision,
            construction_id: self.construction.description_id().into(),
            selection,
            profiles: self.profiles(),
            make_selections: self.make_selections(packages)?,
            first_host: self.first_host.clone(),
            scope: self.scope,
            source: self.source,
        })
    }

    /// Consuming the encounter prevents repeated birth for each target. The
    /// retained result is the input to artifact make, which may fail separately.
    pub fn prepare(
        self,
        review: ReviewedDeployment,
        birth_sequence: u64,
        birth_sign: SignId,
        catalog: &MakeCatalog,
        packages: &MakePackageSet,
    ) -> Result<PreparedDeployment, DeploymentDraftRefusal> {
        if review.revision != self.revision
            || review.construction_id != self.construction.description_id()
            || review.selection
                != self
                    .birth
                    .selection(self.birth.revision())
                    .map_err(DeploymentDraftRefusal::BirthDraft)?
            || review.profiles != self.profiles()
            || review.make_selections != self.make_selections(packages)?
            || review.first_host != self.first_host
            || review.scope != self.scope
            || review.source != self.source
        {
            return Err(DeploymentDraftRefusal::StaleReview);
        }
        let birth = PreparedBodyBirth::from_reviewed_draft(
            &self.birth,
            review.selection.revision,
            birth_sequence,
            birth_sign,
        )
        .map_err(DeploymentDraftRefusal::Birth)?;
        let configurations: BTreeMap<String, HostConfiguration> = self
            .construction
            .hosts()
            .iter()
            .map(|h| {
                (
                    h.description.configuration.clone(),
                    h.configuration.configuration().clone(),
                )
            })
            .collect();
        let mut description = self.construction.description().clone();
        description.body.id = birth.evidence().body_id.as_str().into();
        let construction = check_body_description(description, &configurations, catalog, packages)
            .map_err(DeploymentDraftRefusal::Construction)?;
        if review.profiles
            != construction
                .hosts()
                .iter()
                .map(|h| h.configuration.profile().clone())
                .collect::<Vec<_>>()
        {
            return Err(DeploymentDraftRefusal::StaleReview);
        }
        let selected_hosts = construction
            .hosts()
            .iter()
            .filter(|h| {
                self.scope == DeploymentScope::AllHosts || h.description.name == self.first_host
            })
            .map(|h| h.description.name.clone())
            .collect();
        Ok(PreparedDeployment {
            birth,
            construction,
            first_host: self.first_host,
            selected_hosts,
            image_source: self.source,
        })
    }

    fn make_selections(
        &self,
        packages: &MakePackageSet,
    ) -> Result<Vec<MakeBuildSelection>, DeploymentDraftRefusal> {
        self.construction
            .hosts()
            .iter()
            .map(|h| {
                packages
                    .derive_build_selection(h.configuration.profile(), &h.description.spore.output)
                    .map_err(DeploymentDraftRefusal::Package)
            })
            .collect()
    }

    fn profiles(&self) -> Vec<HostProfile> {
        self.construction
            .hosts()
            .iter()
            .map(|h| h.configuration.profile().clone())
            .collect()
    }

    fn next_revision(&self, revision: u64) -> Result<u64, DeploymentDraftRefusal> {
        if revision != self.revision {
            return Err(DeploymentDraftRefusal::StaleReview);
        }
        self.revision
            .checked_add(1)
            .ok_or(DeploymentDraftRefusal::RevisionExhausted)
    }
}
