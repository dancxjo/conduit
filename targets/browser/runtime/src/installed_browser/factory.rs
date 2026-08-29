//! Browser-owned offer, factory, and host-operation installation catalog.

use super::{linguistics, logic, math, morse, morse_composition, presentation, text, values};
use conduit_core::{
    resource_offer, BaseImplementationId, BootId, CapabilityOffer, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, OfferGeneration, PlannerCapabilityOffer, PlannerLimits,
    PlannerProfileId, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use conduit_planner::BROWSER_PLANNER_PROFILE;

pub(crate) const BOOK_LOCAL_BASE: &str = "conduit.base/local@1";
pub(crate) struct BrowserManifestation {
    pub kind_id: &'static str,
    pub canonical_value: Vec<u8>,
}

pub(crate) struct BrowserHostResult {
    pub output: Option<Vec<u8>>,
    pub manifestation: Option<BrowserManifestation>,
}

pub(crate) type BrowserHostOperation =
    fn(&conduit_core::PlannedGear, &[u8]) -> Result<BrowserHostResult, String>;

pub(crate) struct BrowserInstallation {
    pub implementation_id: &'static str,
    pub offer: fn() -> CapabilityOffer,
    pub prepare: fn(
        &conduit_core::PlannedGear,
        &mut conduit_kernel::HostedValueStore,
    ) -> Result<super::BrowserOperation, String>,
    pub perform: Option<BrowserHostOperation>,
}

static INSTALLATIONS: &[&BrowserInstallation] = &[
    &text::LITERAL,
    &text::UPPER,
    &text::JOIN,
    &text::PRESENTATION,
    &linguistics::TOKENIZE,
    &linguistics::ANNOTATE,
    &linguistics::PRESENTATION,
    &values::SCALAR_LITERAL,
    &values::BOOL_LITERAL,
    &values::SCALAR_PRESENTATION,
    &values::BOOL_PRESENTATION,
    &math::CLAMP,
    &math::SCALE,
    &math::DEADBAND,
    &logic::COMPARE,
    &logic::NOT,
    &morse::DIRECT,
    &morse_composition::TEXT_CHARACTERS,
    &morse_composition::LOOKUP,
    &morse_composition::INTERSPERSE,
    &morse_composition::FLATTEN,
    &morse_composition::SYMBOLS_TO_PATTERN,
    &morse_composition::PATTERN_TO_SYMBOLS,
    &morse_composition::SYMBOLS_TO_TEXT,
    &presentation::INDICATOR,
];

pub(crate) fn catalogs(
) -> Result<(conduit_form::StartupCatalog, conduit_form::ProfileCatalog), String> {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)?;
    linguistics::install_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)?;
    startup.insert(conduit_form::KindSignature {
        kind: conduit_semantic_catalog::BOOL_PRESENTATION_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    conduit_semantic_catalog::install_bool_presentation_catalog(&mut profile)?;
    Ok((startup, profile))
}

pub(crate) fn backs(
    startup: &conduit_form::StartupCatalog,
    profile: &conduit_form::ProfileCatalog,
) -> Result<conduit_form::CanonicalBackCatalog, String> {
    let mut backs = conduit_form::CanonicalBackCatalog::new();
    conduit_text::install_morse_backs(startup, profile, &mut backs)?;
    Ok(backs)
}

pub(crate) fn factory(
    implementation_id: &ImplementationId,
) -> Option<&'static BrowserInstallation> {
    INSTALLATIONS
        .iter()
        .copied()
        .find(|factory| factory.implementation_id == implementation_id.as_str())
}

pub(crate) fn advertisement(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/installed-local@1"),
        resources: vec![resource_offer(
            "browser/presentation",
            PRESENTATION_RESOURCE_CLASS,
            super::MAXIMUM_BROWSER_GEARS as u32,
        )],
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
            limits: PlannerLimits {
                maximum_host_advertisements: 1,
                maximum_gears: super::MAXIMUM_BROWSER_GEARS as u16,
                maximum_connections: super::MAXIMUM_BROWSER_CORDS as u16,
                maximum_authority_grants: 0,
                maximum_protected_resource_grants: 0,
                maximum_line_offers: 0,
            },
        }],
        capabilities: INSTALLATIONS
            .iter()
            .map(|entry| {
                let mut offer = (entry.offer)();
                offer.limits.max_queue_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
                offer
            })
            .collect(),
    }
}

pub(crate) fn local_bases() -> [BaseImplementationId; 1] {
    [BaseImplementationId::from(BOOK_LOCAL_BASE)]
}

pub(super) fn validate_placement(
    placement: &conduit_core::PlannedGear,
    offer: &CapabilityOffer,
) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
    {
        return Err("planned browser Gear does not match its installed capability".into());
    }
    Ok(())
}
