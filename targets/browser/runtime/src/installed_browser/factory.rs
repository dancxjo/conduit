//! Browser-owned offer, factory, and host-operation installation catalog.

use super::{
    button_indicator, delay, input, layout, linguistics, logic, math, morse, morse_composition,
    presentation, quantity, quantity_output, state_time, text, values,
};
use conduit_core::{
    resource_offer, BaseImplementationId, BootId, CapabilityOffer, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, OfferGeneration, PlannerCapabilityOffer, PlannerLimits,
    PlannerProfileId, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION, TIMER_RESOURCE_CLASS,
};
use conduit_planner::BROWSER_PLANNER_PROFILE;

pub(crate) const TOUR_LOCAL_BASE: &str = "conduit.base/local@1";
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
    &super::pattern_comparison::INSTALLATION,
    &super::button_attempt::INSTALLATION,
    &super::timing::INTERVALS,
    &super::timing::NORMALIZE,
    &super::json::ENCODE,
    &super::json::DECODE,
    &super::json::COLLECTION,
    &super::json::SUMMARY,
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
    &quantity::MAP,
    &super::normalized_quantity::NORMALIZE,
    &super::pointer::POINTER,
    &super::pointer_selector::POSITION,
    &super::pointer_selector::X,
    &quantity_output::WRAP,
    &logic::COMPARE,
    &logic::NOT,
    &logic::SELECT,
    &morse::DIRECT,
    &morse_composition::TEXT_CHARACTERS,
    &morse_composition::LOOKUP,
    &morse_composition::INTERSPERSE,
    &morse_composition::FLATTEN,
    &morse_composition::SYMBOLS_TO_PATTERN,
    &morse_composition::PATTERN_TO_SYMBOLS,
    &morse_composition::SYMBOLS_TO_TEXT,
    &state_time::TIME_EVERY,
    &delay::TIME_DELAY,
    &state_time::STATE_COUNT,
    &state_time::COUNT_PRESENTATION,
    &super::tick::INSTALLATION,
    &presentation::INDICATOR,
    &presentation::BOOL,
    &presentation::PATCHBAY,
    &layout::VIEWPORT,
    &input::KEYBOARD,
    &input::BUTTON,
    &button_indicator::MAPPER,
    &button_indicator::INDICATOR,
];

pub(crate) const PRESENTATION_FABRICATION_ID: &str = "browser/dom-presentation@1";
pub(crate) const KEYBOARD_FABRICATION_ID: &str = "browser/keyboard-events@1";
pub(crate) const POINTER_FABRICATION_ID: &str = "browser/pointer-events@1";

/// The finite human-facing machinery admitted into one browser IMAGE.
///
/// This is deliberately expressed in fabrication identities. The Web API
/// surface is merely how the Host realizes these selections after Boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BrowserMachinery {
    presentation: bool,
    keyboard: bool,
    pointer: bool,
}

impl BrowserMachinery {
    #[cfg(test)]
    pub(crate) const PRESENTATION_ONLY: Self = Self {
        presentation: true,
        keyboard: false,
        pointer: false,
    };

    pub(crate) fn from_selected(selected: &[&str]) -> Result<Self, String> {
        let has = |identity| selected.contains(&identity);
        for identity in selected {
            if ![
                PRESENTATION_FABRICATION_ID,
                KEYBOARD_FABRICATION_ID,
                POINTER_FABRICATION_ID,
            ]
            .contains(identity)
            {
                return Err(format!(
                    "unknown browser human-facing fabrication identity {identity}"
                ));
            }
        }
        Ok(Self {
            presentation: has(PRESENTATION_FABRICATION_ID),
            keyboard: has(KEYBOARD_FABRICATION_ID),
            pointer: has(POINTER_FABRICATION_ID),
        })
    }

    fn admits(self, installation: &BrowserInstallation) -> bool {
        if installation.implementation_id == input::KEYBOARD_IMPLEMENTATION {
            return self.keyboard;
        }
        if installation.implementation_id == input::BUTTON_IMPLEMENTATION {
            return self.pointer;
        }
        let offer = (installation.offer)();
        if offer
            .resource_requirements
            .iter()
            .any(|requirement| requirement.class_id.as_str() == PRESENTATION_RESOURCE_CLASS)
        {
            return self.presentation;
        }
        true
    }

    pub(crate) fn selected_fabrication_ids(self) -> Vec<&'static str> {
        [
            (self.presentation, PRESENTATION_FABRICATION_ID),
            (self.keyboard, KEYBOARD_FABRICATION_ID),
            (self.pointer, POINTER_FABRICATION_ID),
        ]
        .into_iter()
        .filter_map(|(selected, identity)| selected.then_some(identity))
        .collect()
    }
}

pub(crate) fn selected_human_machinery() -> Vec<&'static str> {
    BrowserMachinery::from_selected(&[
        PRESENTATION_FABRICATION_ID,
        KEYBOARD_FABRICATION_ID,
        POINTER_FABRICATION_ID,
    ])
    .expect("ordinary browser profile contains reviewed machinery")
    .selected_fabrication_ids()
}

pub(crate) use super::catalogs::{backs, catalogs, catalogs_for_presentation};

pub(crate) fn factory(
    implementation_id: &ImplementationId,
) -> Option<&'static BrowserInstallation> {
    if let Some(resource) = super::resource::factory(implementation_id.as_str()) {
        return Some(resource);
    }
    #[cfg(test)]
    if let Some(fixture) = super::test_json::factory(implementation_id.as_str()) {
        return Some(fixture);
    }
    #[cfg(test)]
    if implementation_id.as_str() == super::test_timing_sink::KIND {
        return Some(&super::test_timing_sink::SINK);
    }
    if implementation_id.as_str() == super::comparison_presentation::IMPLEMENTATION {
        return Some(&super::comparison_presentation::PRESENTATION);
    }
    if implementation_id.as_str() == super::normalized_presentation::IMPLEMENTATION {
        return Some(&super::normalized_presentation::PRESENTATION);
    }
    if implementation_id.as_str() == quantity_output::PRESENTATION_IMPLEMENTATION {
        return Some(&quantity_output::PRESENTATION);
    }
    INSTALLATIONS
        .iter()
        .copied()
        .find(|factory| factory.implementation_id == implementation_id.as_str())
}

pub(crate) fn advertisement(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    advertisement_for_machinery(
        host_id,
        boot_id,
        BrowserMachinery::from_selected(&selected_human_machinery())
            .expect("ordinary browser profile contains reviewed machinery"),
    )
}

pub(crate) fn advertisement_for_presentation(
    host_id: HostId,
    boot_id: BootId,
    profile: super::PresentationProfile,
) -> HostAdvertisement {
    let mut host = advertisement(host_id, boot_id);
    if profile == super::PresentationProfile::Annotation {
        return host;
    }
    host.capabilities.retain(|offer| {
        offer.kind_id.as_str() != conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND
    });
    let mut presenter = match profile {
        super::PresentationProfile::Quantity => quantity_output::presentation_offer(),
        super::PresentationProfile::PatternComparison => super::comparison_presentation::offer(),
        super::PresentationProfile::NormalizedDurations => super::normalized_presentation::offer(),
        super::PresentationProfile::Annotation => unreachable!(),
    };
    presenter.limits.max_queue_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
    host.capabilities.push(presenter);
    host.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    host
}

/// The bounded admission profile used by the ordinary browser membership
/// entrance. It combines the live webchat machinery with the installed text
/// literal needed by small canonical browser Forms.
pub(crate) fn membership_advertisement(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    let mut advertisement = crate::webchat::admission_advertisement(host_id, boot_id);
    advertisement.capabilities.push((text::LITERAL.offer)());
    if !advertisement
        .resources
        .iter()
        .any(|offer| offer.class_id.as_str() == PRESENTATION_RESOURCE_CLASS)
    {
        advertisement.resources.push(resource_offer(
            "browser/presentation",
            PRESENTATION_RESOURCE_CLASS,
            super::MAXIMUM_BROWSER_GEARS as u32,
        ));
        advertisement
            .resources
            .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    }
    advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    advertisement
}

pub(crate) fn advertisement_for_machinery(
    host_id: HostId,
    boot_id: BootId,
    machinery: BrowserMachinery,
) -> HostAdvertisement {
    let mut resources = Vec::new();
    if machinery.presentation {
        resources.push(resource_offer(
            "browser/presentation",
            PRESENTATION_RESOURCE_CLASS,
            super::MAXIMUM_BROWSER_GEARS as u32,
        ));
    }
    resources.push(resource_offer("browser/timer", TIMER_RESOURCE_CLASS, 1));
    super::button_attempt::admit_clock_resource(&mut resources);
    if machinery.keyboard || machinery.pointer {
        resources.push(resource_offer(
            "browser/window-input",
            input::WINDOW_INPUT_RESOURCE_CLASS,
            1,
        ));
    }
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/installed-local@1"),
        resources,
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
            .filter(|entry| machinery.admits(entry))
            .map(|entry| {
                let mut offer = (entry.offer)();
                offer.limits.max_queue_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
                offer
            })
            .collect(),
    }
}

pub(crate) fn local_bases() -> [BaseImplementationId; 1] {
    [BaseImplementationId::from(TOUR_LOCAL_BASE)]
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

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[test]
    fn selected_fabrication_ids_gate_real_planning_offers_and_resources() {
        let viewer = advertisement_for_machinery(
            HostId::from("browser/viewer"),
            BootId::from("browser/viewer-boot"),
            BrowserMachinery::PRESENTATION_ONLY,
        );
        assert!(viewer.capabilities.iter().any(|offer| {
            offer
                .resource_requirements
                .iter()
                .any(|item| item.class_id.as_str() == PRESENTATION_RESOURCE_CLASS)
        }));
        assert!(!viewer.capabilities.iter().any(|offer| {
            offer.implementation.implementation_id.as_str() == input::KEYBOARD_IMPLEMENTATION
        }));
        assert!(!viewer
            .resources
            .iter()
            .any(|resource| { resource.class_id.as_str() == input::WINDOW_INPUT_RESOURCE_CLASS }));

        let control = advertisement_for_machinery(
            HostId::from("browser/control"),
            BootId::from("browser/control-boot"),
            BrowserMachinery::from_selected(&[
                PRESENTATION_FABRICATION_ID,
                KEYBOARD_FABRICATION_ID,
                POINTER_FABRICATION_ID,
            ])
            .unwrap(),
        );
        assert!(control.capabilities.iter().any(|offer| {
            offer.implementation.implementation_id.as_str() == input::KEYBOARD_IMPLEMENTATION
        }));
        assert!(control
            .resources
            .iter()
            .any(|resource| { resource.class_id.as_str() == input::WINDOW_INPUT_RESOURCE_CLASS }));
        assert!(BrowserMachinery::from_selected(&["browser/touch-events@1"]).is_err());
    }

    #[test]
    fn fabrication_metadata_matches_installed_offer_identities() {
        let advertised = advertisement(
            HostId::from("browser/metadata-test"),
            BootId::from("browser/metadata-test-boot"),
        );
        for binding in conduit_host_browser_fabrication::BROWSER_HUMAN_PRESENTATION_REALIZATIONS {
            let Some(installation) = INSTALLATIONS
                .iter()
                .find(|entry| entry.implementation_id == binding.runtime_implementation_id)
            else {
                assert_eq!(
                    binding.runtime_implementation_id,
                    "browser/dom-pointer-source@1"
                );
                let pointer = crate::browser_pointer::advertisement()
                    .capabilities
                    .into_iter()
                    .find(|offer| {
                        offer.implementation.implementation_id.as_str()
                            == binding.runtime_implementation_id
                    })
                    .expect("reviewed pointer vertical advertises its exact realization");
                assert_eq!(pointer.kind_id.as_str(), binding.portable_kind);
                assert_eq!(
                    pointer.implementation.artifact_id.as_str(),
                    binding.runtime_artifact_id
                );
                assert_eq!(
                    pointer.limits.max_queue_items as u32,
                    binding.maximum_queue_items
                );
                assert_eq!(pointer.limits.max_queue_bytes, binding.maximum_queue_bytes);
                assert!(pointer.host_operations.iter().any(|operation| {
                    operation.contract_id.as_str() == binding.host_operation
                        && operation.maximum_in_flight == binding.maximum_in_flight
                }));
                continue;
            };
            let offer = advertised
                .capabilities
                .iter()
                .find(|offer| {
                    offer.implementation.implementation_id.as_str()
                        == installation.implementation_id
                })
                .expect("selected installation is advertised");
            assert_eq!(offer.kind_id.as_str(), binding.portable_kind);
            assert_eq!(
                offer.implementation.artifact_id.as_str(),
                binding.runtime_artifact_id
            );
            assert_eq!(
                offer.limits.max_queue_items as u32, binding.maximum_queue_items,
                "queue item binding drifted for {}",
                binding.runtime_implementation_id
            );
            assert_eq!(
                offer.limits.max_queue_bytes, binding.maximum_queue_bytes,
                "queue byte binding drifted for {}",
                binding.runtime_implementation_id
            );
            assert!(
                offer.host_operations.iter().any(|operation| {
                    operation.contract_id.as_str() == binding.host_operation
                        && operation.maximum_in_flight == binding.maximum_in_flight
                }),
                "fabrication operation binding drifted for {}: expected {}",
                binding.runtime_implementation_id,
                binding.host_operation
            );
        }
    }
}
