//! Exact ordinary planning for USB keyboard input through portable text semantics.

use alloc::{collections::BTreeMap, format, vec};

use conduit_core::{
    ActivePlayIdentity, ArtifactId, BaseImplementationId, CapabilityId, ExecutionProfileId,
    HostAdvertisement, ImplementationId, Plan, bind_active_play, resource_requirement,
};
use conduit_planner::{
    PlanningOptions, default_expanded_placements, plan_expanded_canonical_with_options,
};

use crate::{
    identity::BootIdentities,
    keyboard_offer::{KEYBOARD_IMPLEMENTATION, OPERATION_RESOURCE},
    offer::HostOffer,
    ordinary_plan::{PreparationError, advertisement},
};

pub const FORM_SOURCE: &str = "form conduitos-keyboard-upper {\n    keyboard: input/keyboard\n    keymap: input/keymap\n    upper: text/upper\n    show: presentation/text\n    keyboard.key > keymap.key\n    keymap.text > upper.text\n    upper.text > show.text\n}\n";
pub const KEYMAP_IMPLEMENTATION: &str = "conduitos/kernel-keymap@1";
pub const KEYMAP_EXECUTION_PROFILE: &str = "conduitos/portable-input-cooperative@1";
const PLACEMENTS: usize = 4;
const CONNECTIONS: usize = 3;

pub struct PreparedKeyboardTextPlay {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub expanded_form_id: conduit_core::ExpandedFormId,
    pub active_play: ActivePlayIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardTextSeedIdentity {
    pub source_document_id: conduit_core::SourceDocumentId,
    pub checked_form_id: conduit_core::CheckedFormId,
    pub expanded_form_id: conduit_core::ExpandedFormId,
}

/// Check the IMAGE-embedded platform-neutral Seed without selecting a Host,
/// producing a Plan, or admitting any runtime effect.
pub fn checked_seed_identity() -> Result<KeyboardTextSeedIdentity, PreparationError> {
    let form = checked_expanded_form()?;
    Ok(KeyboardTextSeedIdentity {
        source_document_id: form.source_document_id,
        checked_form_id: form.checked_form_id,
        expanded_form_id: form.expanded_form_id,
    })
}

pub fn prepare(
    identities: &BootIdentities,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<PreparedKeyboardTextPlay, PreparationError> {
    if offer.keyboard.is_none() {
        return Err(PreparationError::PlacementRejected);
    }
    let mut advertisement = advertisement(identities, offer, build_id)?;
    append_keymap_offer(&mut advertisement, build_id);
    let form = checked_expanded_form()?;
    let hosts = [advertisement.clone()];
    let placements = default_expanded_placements(&form, &hosts)
        .map_err(|_| PreparationError::PlacementRejected)?;
    let plan = plan_expanded_canonical_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::KEYBOARD_MAX_QUEUE_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| PreparationError::PlanRejected)?;
    validate(&plan, &advertisement, offer, build_id)?;
    let fragment = &plan.fragments[0];
    let active_play = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 2);
    Ok(PreparedKeyboardTextPlay {
        advertisement,
        source_document_id: plan.source_document_id.clone(),
        checked_form_id: plan.checked_form_id.clone(),
        expanded_form_id: plan.expanded_form_id.clone(),
        plan,
        active_play,
    })
}

pub fn validate(
    plan: &Plan,
    advertisement: &HostAdvertisement,
    offer: &HostOffer<'_>,
    build_id: &str,
) -> Result<(), PreparationError> {
    let Some(keyboard) = offer.keyboard else {
        return Err(PreparationError::OfferMismatch);
    };
    keyboard
        .validate(build_id)
        .map_err(|_| PreparationError::OfferMismatch)?;
    if !conduit_core::verify_plan(plan)
        || plan.fragments.len() != 1
        || advertisement.host_id.as_str() != crate::identity::hex(&offer.host_id)
        || advertisement.boot_id.as_str() != crate::identity::hex(&offer.boot_id)
        || advertisement.offer_generation.0 != offer.generation
    {
        return Err(PreparationError::PlanRejected);
    }
    let fragment = &plan.fragments[0];
    if fragment.host_id != advertisement.host_id
        || fragment.boot_id != advertisement.boot_id
        || fragment.offer_generation != advertisement.offer_generation
        || fragment.placements.len() != PLACEMENTS
        || fragment.connections.len() != CONNECTIONS
    {
        return Err(PreparationError::PlanRejected);
    }
    for (kind, implementation) in [
        (
            conduit_semantic_catalog::KEYBOARD_KIND,
            KEYBOARD_IMPLEMENTATION,
        ),
        (conduit_semantic_catalog::KEYMAP_KIND, KEYMAP_IMPLEMENTATION),
        (
            conduit_text::TEXT_UPPER_KIND,
            crate::offer::TEXT_UPPER_IMPLEMENTATION,
        ),
        (
            conduit_semantic_catalog::TEXT_PRESENTATION_KIND,
            crate::offer::TEXT_PRESENTATION_IMPLEMENTATION,
        ),
    ] {
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .ok_or(PreparationError::PlanRejected)?;
        if placement.implementation_id.as_str() != implementation
            || placement.artifact_id.as_str() != format!("conduitos-build/{build_id}")
            || placement.host_id != advertisement.host_id
            || placement.boot_id != advertisement.boot_id
        {
            return Err(PreparationError::PlanRejected);
        }
    }
    let keymap = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::KEYMAP_KIND)
        .ok_or(PreparationError::PlanRejected)?;
    if keymap.configuration.len() != 1
        || keymap.configuration[0].key.as_str() != "layout"
        || keymap.configuration[0].value
            != conduit_core::ConfigurationValue::Text(conduit_core::CONDUIT_INTL_LAYOUT.into())
        || keymap.host_operations.len() != 1
        || keymap.host_operations[0].contract_id.as_str()
            != crate::functional_offers::KEYMAP_HOST_OPERATION
    {
        return Err(PreparationError::PlanRejected);
    }
    let expected = [
        (
            "keyboard",
            "key",
            "keymap",
            "key",
            conduit_core::KEY_EVENT_INFO_ID,
        ),
        (
            "keymap",
            "text",
            "upper",
            "text",
            conduit_semantic_catalog::TEXT_PRESENTATION_VALUE_KIND,
        ),
        (
            "upper",
            "text",
            "show",
            "text",
            conduit_semantic_catalog::TEXT_PRESENTATION_VALUE_KIND,
        ),
    ];
    for (source_gear, source_port, sink_gear, sink_port, value_kind) in expected {
        let source_placement = fragment
            .placements
            .iter()
            .find(|placement| placement.gear_id.as_str().rsplit('/').next() == Some(source_gear))
            .ok_or(PreparationError::PlanRejected)?;
        let sink_placement = fragment
            .placements
            .iter()
            .find(|placement| placement.gear_id.as_str().rsplit('/').next() == Some(sink_gear))
            .ok_or(PreparationError::PlanRejected)?;
        if !fragment.connections.iter().any(|connection| {
            connection.source_placement_id == source_placement.placement_id
                && connection.source_port_id.as_str() == source_port
                && connection.sink_placement_id == sink_placement.placement_id
                && connection.sink_port_id.as_str() == sink_port
                && connection.value_kind.as_str() == value_kind
                && connection.item_capacity == 1
                && connection.byte_capacity == conduit_semantic_catalog::KEYBOARD_MAX_QUEUE_BYTES
        }) {
            return Err(PreparationError::PlanRejected);
        }
    }
    Ok(())
}

fn append_keymap_offer(advertisement: &mut HostAdvertisement, build_id: &str) {
    let mut keymap = crate::functional_offers::keymap_offer();
    keymap.capability_id = CapabilityId::from("conduitos/input-keymap@1");
    keymap.implementation.execution_profile_id = ExecutionProfileId::from(KEYMAP_EXECUTION_PROFILE);
    keymap.implementation.implementation_id = ImplementationId::from(KEYMAP_IMPLEMENTATION);
    keymap.implementation.artifact_id = ArtifactId::from(format!("conduitos-build/{build_id}"));
    keymap.resource_requirements = vec![
        resource_requirement("conduit.resource/runtime-memory@1", 4_096),
        resource_requirement(OPERATION_RESOURCE, 1),
    ];
    advertisement.capabilities.push(keymap);
}

fn checked_expanded_form() -> Result<conduit_form::ExpandedCanonicalForm, PreparationError> {
    let syntax = conduit_form::parse_syntax_document(FORM_SOURCE);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
        .map_err(|_| PreparationError::FormRejected)?;
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|_| PreparationError::FormRejected)?;
    conduit_form::expand_canonical_form(&checked, "conduitos-keyboard-upper", &profile)
        .map_err(|_| PreparationError::FormRejected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        keyboard_offer::KeyboardRealization,
        offer::{CpuFeatures, HostOffer},
    };

    fn fixture() -> (BootIdentities, HostOffer<'static>) {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            1_048_576,
        )
        .with_keyboard(
            KeyboardRealization {
                controller_id: [3; 32],
                device_id: [4; 32],
                interface_id: [5; 32],
                endpoint_id: [6; 32],
                report_buffers: 2,
                transition_slots: 8,
                operation_slots: 2,
            },
            "build",
        )
        .unwrap();
        (identities, offer)
    }

    #[test]
    fn unchanged_form_defaults_keymap_and_seals_exact_four_gear_path() {
        let (identities, offer) = fixture();
        let prepared = prepare(&identities, &offer, "build").unwrap();
        assert!(!FORM_SOURCE.contains("conduit-intl"));
        assert!(!FORM_SOURCE.contains("usb"));
        assert_eq!(prepared.plan.fragments[0].placements.len(), PLACEMENTS);
        assert_eq!(prepared.plan.fragments[0].connections.len(), CONNECTIONS);
    }

    #[test]
    fn absent_keyboard_stale_plan_and_unsupported_layout_refuse() {
        let (identities, offer) = fixture();
        let absent = HostOffer::new(
            &identities,
            "build",
            offer.cpu_features,
            offer.runtime_arena_bytes,
        );
        assert_eq!(
            prepare(&identities, &absent, "build").err(),
            Some(PreparationError::PlacementRejected)
        );
        let prepared = prepare(&identities, &offer, "build").unwrap();
        let mut stale = fixture().1;
        stale.boot_id = [9; 32];
        assert!(validate(&prepared.plan, &prepared.advertisement, &stale, "build").is_err());
        let unsupported = FORM_SOURCE.replace(
            "keymap: input/keymap",
            "keymap: input/keymap(layout = \"host-locale\")",
        );
        let syntax = conduit_form::parse_syntax_document(&unsupported);
        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile).unwrap();
        conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)
            .unwrap();
        conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)
            .unwrap();
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        assert!(
            conduit_form::expand_canonical_form(&checked, "conduitos-keyboard-upper", &profile,)
                .is_err()
        );
    }

    #[test]
    fn presentation_base_loss_and_cord_underprovision_refuse_before_play() {
        let (identities, mut offer) = fixture();
        offer
            .bases
            .iter_mut()
            .find(|base| base.kind == crate::machine::BaseKind::Serial)
            .unwrap()
            .capacity = 0;
        assert!(prepare(&identities, &offer, "build").is_err());

        let (identities, offer) = fixture();
        let mut advertisement = advertisement(&identities, &offer, "build").unwrap();
        append_keymap_offer(&mut advertisement, "build");
        let form = checked_expanded_form().unwrap();
        let hosts = [advertisement];
        let placements = default_expanded_placements(&form, &hosts).unwrap();
        let underprovisioned = plan_expanded_canonical_with_options(
            &form,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 2,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .unwrap();
        assert!(validate(&underprovisioned, &hosts[0], &offer, "build").is_err());
    }

    #[test]
    fn authored_form_contains_only_portable_meaning() {
        for forbidden in [
            "xhci",
            "pci",
            "usb",
            "hid",
            "qemu",
            "x86",
            "endpoint",
            "interrupt",
            "base",
            "host",
            "boot",
            "device",
            "socket",
            "address",
            "dom",
            "gpio",
            "stdout",
            "credential",
            "resource",
            "layout",
            "locale",
        ] {
            assert!(!FORM_SOURCE.to_ascii_lowercase().contains(forbidden));
        }
    }
}
