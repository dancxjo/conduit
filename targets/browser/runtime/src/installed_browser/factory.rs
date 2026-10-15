//! Browser-owned offer, factory, and Host Call installation catalog.

mod installations;
mod queue_limits;

use super::{input, quantity_output};
use conduit_core::{
    resource_offer, BaseImplementationId, BootId, CapabilityOffer, HostAdvertisement, HostId,
    HostProfileId, ImplementationId, OfferGeneration, PlannerCapabilityOffer, PlannerLimits,
    PlannerProfileId, FACE_RESOURCE_CLASS, PROTOCOL_VERSION, TIMER_RESOURCE_CLASS,
};
use conduit_planner::BROWSER_PLANNER_PROFILE;
use installations::INSTALLATIONS;

pub(crate) const TOUR_LOCAL_BASE: &str = "conduit.base/local@1";
pub(crate) struct BrowserManifestation {
    pub kind_id: &'static str,
    pub canonical_value: Vec<u8>,
    pub data_label: Option<String>,
}

pub(super) fn manifestation_label(
    placement: &conduit_core::PlannedGear,
) -> Result<Option<String>, String> {
    if conduit_semantic_catalog::face_data_label_required(placement.kind_id.as_str()) {
        conduit_semantic_catalog::face_data_label(&placement.configuration)
            .map(|label| Some(label.to_owned()))
    } else {
        Ok(None)
    }
}

pub(crate) struct BrowserHostResult {
    pub output: Option<Vec<u8>>,
    pub manifestation: Option<BrowserManifestation>,
}

pub(crate) type BrowserHostCall =
    fn(&conduit_core::PlannedGear, &[u8]) -> Result<BrowserHostResult, String>;

pub(crate) struct BrowserInstallation {
    pub implementation_id: &'static str,
    pub offer: fn() -> CapabilityOffer,
    pub prepare: fn(
        &conduit_core::PlannedGear,
        &mut conduit_kernel::HostedValueStore,
    ) -> Result<super::BrowserBack, String>,
    pub perform: Option<BrowserHostCall>,
}

pub(crate) const AUDIO_MAKE_ID: &str = "browser/audio-cue@1";
pub(crate) const FACE_MAKE_ID: &str = "browser/dom-face@1";
pub(crate) const KEYBOARD_MAKE_ID: &str = "browser/keyboard-events@1";
pub(crate) const POINTER_MAKE_ID: &str = "browser/pointer-events@1";

/// The finite human-facing machinery admitted into one browser IMAGE.
///
/// This is deliberately expressed in make identities. The Web API
/// surface is merely how the host realizes these selections after Boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BrowserMachinery {
    face: bool,
    keyboard: bool,
    pointer: bool,
    audio: bool,
}

impl BrowserMachinery {
    #[cfg(test)]
    pub(crate) const FACE_ONLY: Self = Self {
        face: true,
        keyboard: false,
        pointer: false,
        audio: false,
    };

    pub(crate) fn from_selected(selected: &[&str]) -> Result<Self, String> {
        let has = |identity| selected.contains(&identity);
        for identity in selected {
            if ![
                FACE_MAKE_ID,
                KEYBOARD_MAKE_ID,
                POINTER_MAKE_ID,
                AUDIO_MAKE_ID,
            ]
            .contains(identity)
            {
                return Err(format!(
                    "unknown browser human-facing make identity {identity}"
                ));
            }
        }
        Ok(Self {
            face: has(FACE_MAKE_ID),
            keyboard: has(KEYBOARD_MAKE_ID),
            pointer: has(POINTER_MAKE_ID),
            audio: has(AUDIO_MAKE_ID),
        })
    }

    fn admits(self, installation: &BrowserInstallation) -> bool {
        if installation.implementation_id == input::KEYBOARD_IMPLEMENTATION {
            return self.keyboard;
        }
        if installation.implementation_id == input::BUTTON_IMPLEMENTATION {
            return self.pointer;
        }
        if installation.implementation_id == super::startup_chime::IMPLEMENTATION {
            return self.audio;
        }
        if installation.implementation_id == super::audio_io::CAPTURE_IMPLEMENTATION
            || installation.implementation_id == super::audio_io::PLAY_IMPLEMENTATION
            || installation.implementation_id == super::audio_io::CONTINUOUS_PLAY_IMPLEMENTATION
        {
            return self.audio;
        }
        if installation.implementation_id == super::pitch_tone::IMPLEMENTATION {
            return self.audio;
        }
        let offer = (installation.offer)();
        if offer
            .resource_requirements
            .iter()
            .any(|requirement| requirement.class_id.as_str() == FACE_RESOURCE_CLASS)
        {
            return self.face;
        }
        true
    }

    pub(crate) fn selected_make_ids(self) -> Vec<&'static str> {
        [
            (self.face, FACE_MAKE_ID),
            (self.keyboard, KEYBOARD_MAKE_ID),
            (self.pointer, POINTER_MAKE_ID),
            (self.audio, AUDIO_MAKE_ID),
        ]
        .into_iter()
        .filter_map(|(selected, identity)| selected.then_some(identity))
        .collect()
    }
}

pub(crate) fn selected_human_machinery() -> Vec<&'static str> {
    BrowserMachinery::from_selected(&[
        FACE_MAKE_ID,
        KEYBOARD_MAKE_ID,
        POINTER_MAKE_ID,
        AUDIO_MAKE_ID,
    ])
    .expect("ordinary browser profile contains reviewed machinery")
    .selected_make_ids()
}

/// Exact installed capabilities supported by the local plot/Body executor.
/// Membership may additionally advertise implementations for other entrances.
pub(crate) fn execution_capability_ids() -> Vec<conduit_core::CapabilityId> {
    let machinery = BrowserMachinery::from_selected(&selected_human_machinery())
        .expect("ordinary browser profile contains reviewed machinery");
    INSTALLATIONS
        .iter()
        .filter(|entry| machinery.admits(entry))
        .map(|entry| (entry.offer)().capability_id)
        .collect()
}

pub(crate) use super::catalogs::{backs, catalogs, catalogs_for_face};
mod lookup;
pub(crate) use lookup::factory;

pub(crate) fn advertisement(host_id: HostId, boot_id: BootId) -> HostAdvertisement {
    advertisement_for_machinery(
        host_id,
        boot_id,
        BrowserMachinery::from_selected(&selected_human_machinery())
            .expect("ordinary browser profile contains reviewed machinery"),
    )
}

pub(crate) fn advertisement_for_face(
    host_id: HostId,
    boot_id: BootId,
    profile: super::FaceProfile,
) -> HostAdvertisement {
    let mut host = advertisement(host_id, boot_id);
    if profile == super::FaceProfile::Annotation {
        for offer in &mut host.capabilities {
            if matches!(
                offer.kind_id.as_str(),
                conduit_language::TOKENIZE_FOUR_KIND
                    | conduit_language::ANNOTATE_FOUR_KIND
                    | conduit_semantic_catalog::STRUCTURED_FACE_KIND
            ) {
                offer.limits.max_queue_bytes = super::MAXIMUM_BROWSER_STORED_VALUE_BYTES as u32;
            }
        }
        return host;
    }
    host.capabilities
        .retain(|offer| offer.kind_id.as_str() != conduit_semantic_catalog::STRUCTURED_FACE_KIND);
    let mut presenter = match profile {
        super::FaceProfile::Quantity => quantity_output::direct_face_offer(),
        super::FaceProfile::PatternComparison => super::comparison_face::offer(),
        super::FaceProfile::NormalizedDurations => super::normalized_face::offer(),
        super::FaceProfile::Annotation => unreachable!(),
    };
    presenter.limits.max_queue_bytes = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;
    if !host
        .capabilities
        .iter()
        .any(|offer| offer.capability_id == presenter.capability_id)
    {
        host.capabilities.push(presenter);
    }
    host.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    host
}

pub(crate) fn advertisement_for_machinery(
    host_id: HostId,
    boot_id: BootId,
    machinery: BrowserMachinery,
) -> HostAdvertisement {
    let mut resources = vec![resource_offer(
        "browser/runtime-memory",
        conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
        super::measurement_limits::MEMORY_POOL_BYTES,
    )];
    if machinery.audio {
        resources.push(resource_offer(
            super::startup_chime::POOL,
            super::startup_chime::RESOURCE,
            super::MAXIMUM_BROWSER_GEARS as u32,
        ));
        resources.push(resource_offer(
            super::audio_io::CAPTURE_POOL,
            super::audio_io::CAPTURE_RESOURCE,
            1,
        ));
        resources.push(resource_offer(
            super::audio_io::PLAY_POOL,
            super::audio_io::PLAY_RESOURCE,
            1,
        ));
    }
    if machinery.face {
        resources.push(resource_offer(
            "browser/face",
            FACE_RESOURCE_CLASS,
            super::MAXIMUM_BROWSER_GEARS as u32,
        ));
    }
    // A browser page can keep one timer operation pending for each admitted
    // Gear. The page-side adapter acquires the same finite number of timeout
    // owners before Play, so this is an executable offer rather than a
    // planning-only capacity.
    resources.push(resource_offer(
        "browser/timer",
        TIMER_RESOURCE_CLASS,
        super::MAXIMUM_BROWSER_GEARS as u32,
    ));
    super::button_attempt::admit_clock_resource(&mut resources);
    resources.push(resource_offer(
        "browser/named-pattern-storage",
        super::template_storage::RESOURCE_CLASS,
        1,
    ));
    if machinery.keyboard || machinery.pointer {
        resources.push(resource_offer(
            "browser/window-input",
            input::WINDOW_INPUT_RESOURCE_CLASS,
            super::MAXIMUM_BROWSER_GEARS as u32,
        ));
    }
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/installed-local@1"),
        bases: vec![],
        resources,
        planner_capabilities: vec![PlannerCapabilityOffer {
            profile_id: PlannerProfileId::from(BROWSER_PLANNER_PROFILE),
            limits: PlannerLimits {
                maximum_host_advertisements: 1,
                maximum_gears: super::MAXIMUM_BROWSER_GEARS as u16,
                maximum_connections: super::MAXIMUM_BROWSER_CORDS as u16,
                maximum_authority_grants: 2,
                maximum_protected_resource_grants: 0,
                maximum_line_offers: 0,
            },
        }],
        capabilities: INSTALLATIONS
            .iter()
            .filter(|entry| machinery.admits(entry))
            .map(|entry| {
                let mut offer = (entry.offer)();
                queue_limits::apply(&mut offer, entry.implementation_id);
                offer
            })
            .collect(),
    }
}

pub(crate) fn local_bases() -> [BaseImplementationId; 1] {
    [BaseImplementationId::from(TOUR_LOCAL_BASE)]
}

mod placement_validation;
pub(super) use placement_validation::validate_placement;

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[test]
    fn selected_make_ids_gate_real_planning_offers_and_resources() {
        let viewer = advertisement_for_machinery(
            HostId::from("browser/viewer"),
            BootId::from("browser/viewer-boot"),
            BrowserMachinery::FACE_ONLY,
        );
        assert!(viewer.capabilities.iter().any(|offer| {
            offer
                .resource_requirements
                .iter()
                .any(|item| item.class_id.as_str() == FACE_RESOURCE_CLASS)
        }));
        assert!(!viewer.capabilities.iter().any(|offer| {
            offer.implementation.implementation_id.as_str() == input::KEYBOARD_IMPLEMENTATION
        }));
        assert!(!viewer
            .resources
            .iter()
            .any(|resource| { resource.class_id.as_str() == input::WINDOW_INPUT_RESOURCE_CLASS }));

        assert!(!viewer.capabilities.iter().any(|offer| {
            offer.implementation.implementation_id.as_str()
                == super::super::audio_io::CONTINUOUS_PLAY_IMPLEMENTATION
        }));
        let audio_host = advertisement_for_machinery(
            "browser/audio".into(),
            "browser/audio-boot".into(),
            BrowserMachinery::from_selected(&[AUDIO_MAKE_ID]).unwrap(),
        );
        assert!(audio_host.capabilities.iter().any(|offer| {
            offer.implementation.implementation_id.as_str()
                == super::super::audio_io::CONTINUOUS_PLAY_IMPLEMENTATION
        }));

        let control = advertisement_for_machinery(
            HostId::from("browser/control"),
            BootId::from("browser/control-boot"),
            BrowserMachinery::from_selected(&[FACE_MAKE_ID, KEYBOARD_MAKE_ID, POINTER_MAKE_ID])
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
    fn make_metadata_matches_installed_offer_identities() {
        let advertised = advertisement(
            HostId::from("browser/metadata-test"),
            BootId::from("browser/metadata-test-boot"),
        );
        for binding in conduit_host_browser_make::BROWSER_HUMAN_FACE_REALIZATIONS {
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
                assert!(pointer.host_calls.iter().any(|operation| {
                    operation.contract_id.as_str() == binding.host_call
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
                offer.host_calls.iter().any(|operation| {
                    operation.contract_id.as_str() == binding.host_call
                        && operation.maximum_in_flight == binding.maximum_in_flight
                }),
                "make operation binding drifted for {}: expected {}",
                binding.runtime_implementation_id,
                binding.host_call
            );
        }
    }
}

#[cfg(test)]
mod data_label_tests;
