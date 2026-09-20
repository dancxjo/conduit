use super::RecordingTimer;
use crate::hosted_vision::{FiniteHostedVisionBase, HostedVisionFrame};
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_core::{
    authority_grant, BaseImplementationId, BootId, CapabilityId, HostId, KindContractRevision,
    OfferGeneration, PortTemporal, ProtectedResourceAccess, ProtectedResourceCommitPolicy,
    ProtectedResourceGrant, ResourceBindingRoleId, ResourceClassId, ResourceHandleId,
    TerminalDisposition,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

#[test]
fn authored_motion_runs_through_protected_finite_base_and_production_kernel() {
    let image = conduit_semantic_catalog::deterministic_vision_fixture()
        .unwrap()
        .image;
    let encoded = image.canonical_bytes().unwrap();
    let (_, resource, width, height) =
        crate::hosted_vision::decode_image_resource_for_test(&encoded);
    let base = FiniteHostedVisionBase::new(
        vec![HostedVisionFrame {
            resource,
            width,
            height,
            grayscale_pixels: vec![0; usize::from(width) * usize::from(height)],
        }],
        width,
        height,
        4,
        "finite-image-residence/plan-play-1",
    )
    .unwrap();
    let mut host = StdHost::new_with_finite_vision(
        StdHostConfig {
            host_id: HostId::from("vision-host"),
            boot_id: BootId::from("vision-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        base,
    )
    .unwrap();

    let (startup, profile, source_offer, sink_offer) = catalogs(&image);
    host.advertisement
        .capabilities
        .extend([source_offer, sink_offer]);
    host.advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement).unwrap();
    let source = format!(
        "form proof {{\n source: conduit-test/vision-image-source(value = \"{}\")\n motion: vision/local-motion\n sink: conduit-test/local-model-result\n source.image > motion.image\n motion.motions > sink.value\n}}\n",
        hex(&encoded)
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "proof", &profile).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let motion_offer = hosts[0]
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::VISION_MOTION_KIND)
        .unwrap();
    let motion_gear = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::VISION_MOTION_KIND)
        .unwrap();
    let authority = authority_grant(
        "grant/vision/read-1",
        &motion_offer.authority_requirements[0],
        hosts[0].host_id.clone(),
        hosts[0].boot_id.clone(),
        motion_offer.capability_id.clone(),
    );
    let resource = ProtectedResourceGrant {
        role_id: ResourceBindingRoleId::from(conduit_std_offers::LOCAL_VISION_RESOURCE_ROLE),
        handle_id: ResourceHandleId::from("handle/finite-image-residence/1"),
        gear_id: motion_gear.gear_id.clone(),
        host_id: hosts[0].host_id.clone(),
        boot_id: hosts[0].boot_id.clone(),
        capability_id: motion_offer.capability_id.clone(),
        class_id: ResourceClassId::from(conduit_std_offers::LOCAL_VISION_RESOURCE_CLASS),
        access: ProtectedResourceAccess::ReadExisting,
        maximum_bytes: conduit_semantic_catalog::MAXIMUM_LOCAL_CV_PIXELS as u64,
        commit_policy: ProtectedResourceCommitPolicy::NotApplicable,
    };
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[authority],
            protected_resource_grants: &[resource],
            line_offers: &[],
        },
    )
    .unwrap();
    let motion = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::VISION_MOTION_KIND
        })
        .unwrap();
    assert_eq!(motion.resources.len(), 1);
    assert!(motion.resources[0].protected.is_some());
    assert_eq!(motion.authority.len(), 1);
    assert_eq!(
        motion.host_operations[0].contract_id.as_str(),
        conduit_std_offers::LOCAL_VISION_MOTION_OPERATION
    );

    let report = host
        .run_fragment_to(
            plan.fragments[0].clone(),
            &mut Vec::with_capacity(2_048),
            &mut RecordingTimer { waits: Vec::new() },
        )
        .unwrap();
    assert!(matches!(
        report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    assert!(report.kernel.is_some());
}

fn catalogs(
    image: &conduit_core::StructuredInfoValue,
) -> (
    StartupCatalog,
    ProfileCatalog,
    conduit_core::CapabilityOffer,
    conduit_core::CapabilityOffer,
) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile).unwrap();
    let kind = "conduit-test/vision-image-source";
    let mut offer = crate::installed_std::test_structured_selector::offer_named(
        image.value_type(),
        conduit_core::PortDirection::Output,
        kind,
        "unused",
    );
    offer.outputs[0].port_id = conduit_core::port_id("image");
    offer.outputs[0].temporal = PortTemporal::Current;
    offer.capability_id = CapabilityId::from(kind);
    let motions = conduit_semantic_catalog::vision_motions_type();
    let mut sink_offer = crate::installed_std::test_local_model_io::sink_offer(
        motions.profile().unwrap().value_kind().as_str(),
    );
    sink_offer.inputs[0].temporal = PortTemporal::Current;
    sink_offer.limits.max_queue_bytes = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
    startup
        .insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "Text".into(),
                default: Some(String::new()),
            }],
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: conduit_core::kind_id(kind),
            kind_contract_revision: KindContractRevision::from("conduit-test/structured-source@1"),
            inputs: Vec::new(),
            outputs: offer.outputs.clone(),
            configuration: vec![ConfigurationField {
                key: "value".into(),
                default_value: conduit_core::ConfigurationValue::Text(String::new()),
                validation: ConfigurationRule::TextBytes {
                    maximum: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 2,
                },
            }],
        })
        .unwrap();
    startup
        .insert(KindSignature {
            kind: sink_offer.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    profile
        .insert(KindDefinition {
            kind_id: sink_offer.kind_id.clone(),
            kind_contract_revision: sink_offer.kind_contract_revision.clone(),
            inputs: sink_offer.inputs.clone(),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .unwrap();
    (startup, profile, offer, sink_offer)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
