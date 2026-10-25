//! Checked Host-name/profile edits; no live Host or provider is initialized.
use super::*;
use conduit_host_make::{CheckedHostConfiguration, HostConfiguration, MakeCatalog, MakePackageSet};
use std::collections::BTreeMap;

pub const MAX_DEPLOYMENT_PROFILES: usize = 64;

impl DeploymentEncounter {
    /// The owner supplies actual checked recipes from the composed package
    /// catalogue. Unoffered configurations cannot be introduced by text input.
    pub fn with_profiles(
        draft: DeploymentDraft,
        mut profiles: Vec<CheckedHostConfiguration>,
    ) -> Result<Self, DeploymentEncounterRefusal> {
        if profiles.len() > MAX_DEPLOYMENT_PROFILES {
            return Err(DeploymentEncounterRefusal::InvalidCatalogue);
        }
        profiles.sort_by(|a, b| a.configuration_id().cmp(b.configuration_id()));
        if profiles.windows(2).any(|pair| {
            pair[0].configuration_id() == pair[1].configuration_id()
                && pair[0].profile() != pair[1].profile()
        }) {
            return Err(DeploymentEncounterRefusal::InvalidCatalogue);
        }
        profiles.dedup_by(|a, b| a.configuration_id() == b.configuration_id());
        if draft.construction().hosts().iter().any(|h| {
            !profiles.iter().any(|p| {
                p.configuration_id() == h.configuration.configuration_id()
                    && p.profile() == h.configuration.profile()
            })
        }) {
            return Err(DeploymentEncounterRefusal::InvalidCatalogue);
        }
        Ok(Self {
            draft,
            profiles,
            page: DeploymentPage::Body,
            revision: 1,
        })
    }

    pub(super) fn rename_host(
        &mut self,
        index: usize,
        name: &str,
        catalog: &MakeCatalog,
        packages: &MakePackageSet,
    ) -> Result<(), DeploymentEncounterRefusal> {
        use DeploymentEncounterRefusal as E;
        if name.trim().is_empty() || name.len() > 64 || name.chars().any(char::is_control) {
            return Err(E::InvalidHostName);
        }
        let mut description = self.draft.construction().description().clone();
        let host = description.hosts.get_mut(index).ok_or(E::InvalidPage)?;
        let old = host.name.clone();
        host.name = name.into();
        let first = if self.draft.first_host() == old {
            name.into()
        } else {
            self.draft.first_host().to_owned()
        };
        let checked =
            crate::check_body_description(description, &self.configurations(), catalog, packages)
                .map_err(|e| E::Draft(DeploymentDraftRefusal::Construction(e)))?;
        self.draft
            .replace_renamed_host(self.draft.revision(), checked, &first, &old, name)
            .map_err(E::Draft)
    }

    pub(super) fn select_profile(
        &mut self,
        index: usize,
        id: &str,
        catalog: &MakeCatalog,
        packages: &MakePackageSet,
    ) -> Result<(), DeploymentEncounterRefusal> {
        use DeploymentEncounterRefusal as E;
        let profile = self
            .profiles
            .iter()
            .find(|p| p.configuration_id() == id)
            .ok_or(E::InvalidCatalogue)?;
        let descriptor = packages
            .target_descriptor(&profile.profile().target.key())
            .ok_or(E::InvalidCatalogue)?;
        let digest = profile
            .configuration_id()
            .rsplit(':')
            .next()
            .ok_or(E::InvalidCatalogue)?;
        let key = format!("profiles/{digest}.host.conduit");
        let mut configurations = self.configurations();
        configurations.insert(key.clone(), profile.configuration().clone());
        let mut description = self.draft.construction().description().clone();
        let host = description.hosts.get_mut(index).ok_or(E::InvalidPage)?;
        host.configuration = key;
        host.spore.output = descriptor.default_output.clone();
        let checked =
            crate::check_body_description(description, &configurations, catalog, packages)
                .map_err(|e| E::Draft(DeploymentDraftRefusal::Construction(e)))?;
        let first = self.draft.first_host().to_owned();
        self.draft
            .replace_construction(self.draft.revision(), checked, &first)
            .map_err(E::Draft)
    }

    fn configurations(&self) -> BTreeMap<String, HostConfiguration> {
        self.draft
            .construction()
            .hosts()
            .iter()
            .map(|h| {
                (
                    h.description.configuration.clone(),
                    h.configuration.configuration().clone(),
                )
            })
            .collect()
    }
}
