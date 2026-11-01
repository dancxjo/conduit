//! Structured Host authoring over package truth. No I/O, Make or readiness effects.
use conduit_face::{Face, FaceBasis, FaceInteraction, MaskShow};
use conduit_host_make::{CheckedHostConfiguration, HostConfiguration, MakeCatalog, MakePackageSet};
mod draft;
mod projection;

pub const MAX_HOST_ENCOUNTER_SOURCE_BYTES: usize = 16 * 1024;
pub const MAX_HOST_ENCOUNTER_BASES: usize = 8;
pub const MAX_HOST_ENCOUNTER_INTERACTIONS: u32 = 1024;
const PAGE_SIZE: usize = 8;
const MAX_CHOICES: usize = 256;

/// Actual execution identity supplied by the retained producing owner.
pub struct HostConfigurationEncounterBasis {
    pub host_id: conduit_core::HostId,
    pub boot_id: conduit_core::BootId,
    pub encounter_id: String,
    pub execution: FaceBasis,
}
#[derive(Clone, Copy)]
enum Page {
    Main,
    Targets(usize),
    Bases(usize),
    Initialization(usize),
    Source(usize),
}

pub enum HostConfigurationEncounterOutcome {
    Changed,
    /// Checked construction intent, never a Ready provider or produced image.
    Checked(CheckedHostConfiguration),
}

/// Borrows immutable catalogues; changing package truth requires a new encounter.
/// The selected limits and initialization values are authored explicitly. Target
/// maxima and package defaults are hints, never initialization or authority.
pub struct HostConfigurationEncounter<'a> {
    catalog: &'a MakeCatalog,
    packages: &'a MakePackageSet,
    configuration: Option<HostConfiguration>,
    page: Page,
    revision: u64,
    interactions: u32,
}
impl<'a> HostConfigurationEncounter<'a> {
    pub fn new(catalog: &'a MakeCatalog, packages: &'a MakePackageSet) -> Result<Self, String> {
        if packages
            .contributions()
            .iter()
            .map(|c| match c {
                conduit_host_make::MakeContribution::Anchor(a) => a.targets.len(),
                _ => 0,
            })
            .sum::<usize>()
            > MAX_CHOICES
        {
            return Err("Host target catalogue exceeds encounter bound".into());
        }
        Ok(Self {
            catalog,
            packages,
            configuration: None,
            page: Page::Targets(0),
            revision: 1,
            interactions: 0,
        })
    }
    pub fn load(
        source: &str,
        catalog: &'a MakeCatalog,
        packages: &'a MakePackageSet,
    ) -> Result<Self, String> {
        if source.len() > MAX_HOST_ENCOUNTER_SOURCE_BYTES {
            return Err("Host source exceeds encounter bound".into());
        }
        let configuration = conduit_host_make::parse_host_configuration_conduit(source)
            .map_err(|_| "Host source decode refused")?;
        draft::bounds(&configuration)?;
        conduit_host_make::check_host_configuration(configuration.clone(), catalog, packages)
            .map_err(draft::diagnostics)?;
        let mut result = Self::new(catalog, packages)?;
        result.configuration = Some(configuration);
        result.page = Page::Main;
        Ok(result)
    }
    pub fn configuration(&self) -> Option<&HostConfiguration> {
        self.configuration.as_ref()
    }
    pub fn checked(&self) -> Result<CheckedHostConfiguration, String> {
        let configuration = self.configuration.as_ref().ok_or("Select a target first")?;
        conduit_host_make::check_host_configuration(
            configuration.clone(),
            self.catalog,
            self.packages,
        )
        .map_err(draft::diagnostics)
    }
    pub fn canonical_source(&self) -> Result<String, String> {
        let checked = self.checked()?;
        let source =
            conduit_host_make::canonical_host_configuration_conduit(checked.configuration())
                .map_err(|_| "Canonical Host source refused")?;
        if source.len() > MAX_HOST_ENCOUNTER_SOURCE_BYTES {
            return Err("Canonical Host source exceeds encounter bound".into());
        }
        Ok(source)
    }
    pub fn face(&self, basis: &HostConfigurationEncounterBasis) -> Result<Face, String> {
        projection::project(self, basis)
    }
    pub fn apply_interaction(
        &mut self,
        basis: &HostConfigurationEncounterBasis,
        show: &MaskShow,
        input: &FaceInteraction,
    ) -> Result<HostConfigurationEncounterOutcome, String> {
        input
            .validate_against(&self.face(basis)?, show)
            .map_err(|e| format!("Host interaction refused: {e:?}"))?;
        if self.interactions >= MAX_HOST_ENCOUNTER_INTERACTIONS {
            return Err("Host interaction budget exhausted".into());
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or("Host revision exhausted")?;
        let value = input
            .arguments
            .first()
            .map(|a| std::str::from_utf8(&a.value))
            .transpose()
            .map_err(|_| "Host field requires UTF-8")?
            .unwrap_or("");
        let mut configuration = self.configuration.clone();
        let page = draft::apply(self, &mut configuration, &input.action_id, value)?;
        if let Some(configuration) = &configuration {
            draft::bounds(configuration)?;
        }
        let outcome = if input.action_id == "host-config/check" {
            HostConfigurationEncounterOutcome::Checked(self.checked()?)
        } else {
            HostConfigurationEncounterOutcome::Changed
        };
        self.configuration = configuration;
        self.page = page;
        self.revision = revision;
        self.interactions += 1;
        Ok(outcome)
    }
}
#[cfg(test)]
mod tests;
