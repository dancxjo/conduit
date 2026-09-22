//! Reviewed ordinary form worksets for one durable Pete Body.

use conduit_body::{BodyWorkset, BodyWorksetError, ResidentForm};
use conduit_core::{KindId, PortTemporal};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, ProfileCatalog, StartupCatalog,
};

pub const PETE_SITUATION_FORM_SOURCE: &str =
    include_str!("../../../forms/pete-situation/main.conduit");
pub const PETE_MEMORY_FORM_SOURCE: &str = include_str!("../../../forms/pete-memory/main.conduit");
pub const BOUNDED_TYPED_HISTORY_FORM_SOURCE: &str =
    include_str!("../../../forms/bounded-typed-history/main.conduit");
pub const HOUSE_CONVERSATION_FORM_SOURCE: &str =
    include_str!("../../../forms/house-conversation/main.conduit");
pub const BOUNDED_NAVIGATION_FORM_SOURCE: &str =
    include_str!("../../../forms/bounded-navigation/main.conduit");
pub const HOMEOSTASIS_FORM_SOURCE: &str = include_str!("../forms/homeostasis.conduit");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeteWorkloadRole {
    Situation,
    AutobiographicalMemory,
    HistoricalIndex,
    Conversation,
    Navigation,
    Homeostasis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeteResidentForm {
    pub role: PeteWorkloadRole,
    pub form: ResidentForm,
    pub required_kinds: Vec<KindId>,
    pub may_request_motion: bool,
    /// Exact checked expansion supplied to ordinary planning; not a placement.
    pub expanded: conduit_form::ExpandedCanonicalForm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewedPeteWorkload {
    /// Safe non-actuating workload used at birth revision zero.
    pub initial: BodyWorkset,
    /// The ordinary navigation Form is available for later explicit admission.
    pub navigation: ResidentForm,
    pub resident_forms: Vec<PeteResidentForm>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PeteWorkloadRefusal {
    Catalog(String),
    InvalidForm,
    Workset(BodyWorksetError),
    MotionUnavailable,
    AuthorityAbsent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PeteWorkloadProfile {
    ObservationOnly,
    Conversational,
    EmbodiedAttended,
}

impl ReviewedPeteWorkload {
    pub fn with_navigation(&self) -> Result<BodyWorkset, PeteWorkloadRefusal> {
        let mut workset = self.initial.clone();
        workset
            .add(self.navigation.clone())
            .map_err(PeteWorkloadRefusal::Workset)?;
        Ok(workset)
    }

    pub fn for_profile(
        &self,
        profile: PeteWorkloadProfile,
        motion_available: bool,
        motion_authorized: bool,
    ) -> Result<BodyWorkset, PeteWorkloadRefusal> {
        match profile {
            PeteWorkloadProfile::ObservationOnly => BodyWorkset::from_forms(
                self.resident_forms
                    .iter()
                    .filter(|item| {
                        matches!(
                            item.role,
                            PeteWorkloadRole::Situation
                                | PeteWorkloadRole::AutobiographicalMemory
                                | PeteWorkloadRole::HistoricalIndex
                                | PeteWorkloadRole::Homeostasis
                        )
                    })
                    .map(|item| item.form.clone()),
            )
            .map_err(PeteWorkloadRefusal::Workset),
            PeteWorkloadProfile::Conversational => Ok(self.initial.clone()),
            PeteWorkloadProfile::EmbodiedAttended if !motion_available => {
                Err(PeteWorkloadRefusal::MotionUnavailable)
            }
            PeteWorkloadProfile::EmbodiedAttended if !motion_authorized => {
                Err(PeteWorkloadRefusal::AuthorityAbsent)
            }
            PeteWorkloadProfile::EmbodiedAttended => self.with_navigation(),
        }
    }
}

pub fn reviewed_pete_workload() -> Result<ReviewedPeteWorkload, PeteWorkloadRefusal> {
    let (startup, profile) = workload_catalogs().map_err(PeteWorkloadRefusal::Catalog)?;
    let sources = [
        (
            PeteWorkloadRole::Situation,
            "pete-situation",
            PETE_SITUATION_FORM_SOURCE,
            false,
        ),
        (
            PeteWorkloadRole::AutobiographicalMemory,
            "pete-memory",
            PETE_MEMORY_FORM_SOURCE,
            false,
        ),
        (
            PeteWorkloadRole::HistoricalIndex,
            "bounded-typed-history",
            BOUNDED_TYPED_HISTORY_FORM_SOURCE,
            false,
        ),
        (
            PeteWorkloadRole::Conversation,
            "house-conversation",
            HOUSE_CONVERSATION_FORM_SOURCE,
            false,
        ),
        (
            PeteWorkloadRole::Homeostasis,
            "pete-homeostasis",
            HOMEOSTASIS_FORM_SOURCE,
            false,
        ),
        (
            PeteWorkloadRole::Navigation,
            "bounded-navigation",
            BOUNDED_NAVIGATION_FORM_SOURCE,
            true,
        ),
    ];
    let mut resident_forms = Vec::with_capacity(sources.len());
    for (role, name, source, may_request_motion) in sources {
        resident_forms.push(check_resident(
            &startup,
            &profile,
            role,
            name,
            source,
            may_request_motion,
        )?);
    }
    let navigation = resident_forms
        .iter()
        .find(|resident| resident.role == PeteWorkloadRole::Navigation)
        .ok_or(PeteWorkloadRefusal::InvalidForm)?
        .form
        .clone();
    let initial = BodyWorkset::from_forms(
        resident_forms
            .iter()
            .filter(|resident| !resident.may_request_motion)
            .map(|resident| resident.form.clone()),
    )
    .map_err(PeteWorkloadRefusal::Workset)?;
    Ok(ReviewedPeteWorkload {
        initial,
        navigation,
        resident_forms,
    })
}

fn check_resident(
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
    role: PeteWorkloadRole,
    expected_name: &str,
    source: &str,
    may_request_motion: bool,
) -> Result<PeteResidentForm, PeteWorkloadRefusal> {
    let parsed = parse_syntax_document(source);
    let checked = check_syntax_document(&parsed, startup).map_err(|error| {
        PeteWorkloadRefusal::Catalog(format!("check {expected_name}: {error:?}"))
    })?;
    let [form] = checked.forms.as_slice() else {
        return Err(PeteWorkloadRefusal::InvalidForm);
    };
    if form.name != expected_name {
        return Err(PeteWorkloadRefusal::InvalidForm);
    }
    let mut expansion_profile = profile.clone();
    for selector in checked
        .forms
        .iter()
        .flat_map(|form| &form.cords)
        .flat_map(|cord| &cord.stages)
        .filter_map(|stage| match stage {
            CheckedCordStage::StructuredSelector { selector, .. } => Some(selector),
            _ => None,
        })
    {
        expansion_profile
            .insert(structured_selector_definition(
                selector,
                PortTemporal::Value,
            ))
            .map_err(|error| {
                PeteWorkloadRefusal::Catalog(format!("install {expected_name} selector: {error:?}"))
            })?;
    }
    let expanded = expand_canonical_form_for_authoring(&checked, expected_name, &expansion_profile)
        .map_err(|error| {
            PeteWorkloadRefusal::Catalog(format!("expand {expected_name}: {error:?}"))
        })?;
    let mut required_kinds: Vec<_> = expanded
        .expanded
        .gears
        .iter()
        .map(|gear| gear.kind_id.clone())
        .collect();
    required_kinds.sort();
    required_kinds.dedup();
    if required_kinds.is_empty() {
        return Err(PeteWorkloadRefusal::InvalidForm);
    }
    Ok(PeteResidentForm {
        role,
        form: ResidentForm::new(
            checked.source_document_id.clone(),
            form.checked_form_id.clone(),
        ),
        required_kinds,
        may_request_motion,
        expanded: expanded.expanded,
    })
}

fn workload_catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    let (mut startup, mut profile) = crate::catalogs()?;
    crate::install_pete_situation_catalog(&mut startup, &mut profile)?;
    crate::install_pete_memory_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_robotics_structured_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_navigation_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_experience_catalogs(&mut startup, &mut profile)?;
    crate::install_homeostasis_catalogs(&mut startup, &mut profile)?;
    conduit_time::install_historical_timeline_catalog(&mut startup, &mut profile)?;
    conduit_text::install_text_catalogs(&mut startup, &mut profile)?;
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile)?;
    conduit_ai::install_model_text_catalog(&mut startup, &mut profile)?;
    conduit_tongues::install_house_conversation_catalog(&mut startup, &mut profile)?;
    Ok((startup, profile))
}

#[cfg(test)]
#[path = "pete_workload_tests.rs"]
mod tests;
