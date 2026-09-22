use super::TimerAdapter;
use crate::hosted_vision::{FiniteHostedVisionBase, HostedVisionFrame};
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_core::{
    authority_grant, BaseImplementationId, BootId, CapabilityId, HostId, KindIdentity,
    OfferGeneration, PortTemporal, ProtectedResourceAccess, ProtectedResourceCommitPolicy,
    ProtectedResourceGrant, ResourceBindingRoleId, ResourceClassId, ResourceHandleId,
    TerminalDisposition,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindConfigurationField,
    KindConfigurationRule, KindProjection, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;

struct VisionTimer;

impl TimerAdapter for VisionTimer {
    fn wait(&mut self, _: std::time::Duration) {}
    fn monotonic_now_micros(&mut self) -> Option<u64> {
        Some(41)
    }
}

struct FixtureVisualModel;

impl crate::hosted_local_model::HostedVisualModelAdapter for FixtureVisualModel {
    fn describe(
        &mut self,
        image: &[u8],
        _: &str,
    ) -> Result<crate::hosted_local_model::VisualModelOutput, String> {
        if !image.starts_with(b"P5\n") {
            return Err("fixture expected one exact graymap".into());
        }
        Ok(crate::hosted_local_model::VisualModelOutput {
            text: "One exact finite scene.".into(),
            model_id: "fixture-vision-model".into(),
            provider_instance_id: "fixture-provider/1".into(),
            artifact_id: "fixture-artifact/sha256-1".into(),
            prompt_contract_revision: "conduit.prompt/visual-description@1",
            truncated: false,
            work_units: 4,
        })
    }
}

#[test]
fn authored_description_joins_four_typed_inputs_through_one_production_play() {
    let image = conduit_semantic_catalog::deterministic_vision_fixture()
        .unwrap()
        .image;
    let encoded_image = image.canonical_bytes().unwrap();
    let (_, resource, width, height) =
        crate::hosted_vision::decode_image_resource_for_test(&encoded_image);
    let profile = resource.content_profile.clone();
    let values = [
        image,
        conduit_semantic_catalog::object_observations_value(&[], &profile).unwrap(),
        conduit_semantic_catalog::visible_text_observations_value(&[], &profile).unwrap(),
        conduit_semantic_catalog::track_observations_value(&[], &profile).unwrap(),
    ];
    let base = FiniteHostedVisionBase::new(
        vec![HostedVisionFrame {
            canonical_image: encoded_image,
            resource,
            width,
            height,
            grayscale_pixels: vec![0; usize::from(width) * usize::from(height)],
        }],
        width,
        height,
        4,
        "finite-image-residence/describe-play-1",
    )
    .unwrap()
    .with_visual_model(FixtureVisualModel);
    let mut host = StdHost::new_with_finite_vision(
        StdHostConfig {
            host_id: HostId::from("vision-describe-host"),
            boot_id: BootId::from("vision-describe-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        base,
    )
    .unwrap();
    let (startup, profile_catalog, source_offers, sink_offer) = describe_catalogs(&values);
    host.advertisement.capabilities.extend(source_offers);
    host.advertisement.capabilities.push(sink_offer);
    host.advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement).unwrap();
    let source = format!(
        "form proof {{\n image: conduit-test/vision-image(value = \"{}\")\n objects: conduit-test/vision-objects(value = \"{}\")\n texts: conduit-test/vision-texts(value = \"{}\")\n tracks: conduit-test/vision-tracks(value = \"{}\")\n describe: vision/model-describe\n sink: conduit-test/local-model-result\n image.image > describe.image\n objects.detections > describe.detections\n texts.texts > describe.texts\n tracks.tracks > describe.tracks\n describe.impression > sink.value\n}}\n",
        hex(&values[0].canonical_bytes().unwrap()),
        hex(&values[1].canonical_bytes().unwrap()),
        hex(&values[2].canonical_bytes().unwrap()),
        hex(&values[3].canonical_bytes().unwrap()),
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "proof", &profile_catalog).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let offer = hosts[0]
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::VISION_DESCRIBE_KIND)
        .unwrap();
    let gear = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::VISION_DESCRIBE_KIND)
        .unwrap();
    let authority = authority_grant(
        "grant/vision/describe-read-1",
        &offer.authority_requirements[0],
        hosts[0].host_id.clone(),
        hosts[0].boot_id.clone(),
        offer.capability_id.clone(),
    );
    let resource = ProtectedResourceGrant {
        role_id: ResourceBindingRoleId::from(conduit_std_offers::LOCAL_VISION_RESOURCE_ROLE),
        handle_id: ResourceHandleId::from("handle/finite-image-residence/describe-1"),
        gear_id: gear.gear_id.clone(),
        host_id: hosts[0].host_id.clone(),
        boot_id: hosts[0].boot_id.clone(),
        capability_id: offer.capability_id.clone(),
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
    let report = host
        .run_fragment_to(
            plan.fragments[0].clone(),
            &mut Vec::with_capacity(2_048),
            &mut VisionTimer,
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|value| &value.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
}

fn describe_catalogs(
    values: &[conduit_core::StructuredInfoValue; 4],
) -> (
    StartupCatalog,
    ProfileCatalog,
    Vec<conduit_core::CapabilityOffer>,
    conduit_core::CapabilityOffer,
) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profile).unwrap();
    let sources = [
        ("conduit-test/vision-image", "image"),
        ("conduit-test/vision-objects", "detections"),
        ("conduit-test/vision-texts", "texts"),
        ("conduit-test/vision-tracks", "tracks"),
    ];
    let mut offers = Vec::with_capacity(4);
    for ((kind, port), value) in sources.into_iter().zip(values) {
        let mut offer = crate::installed_std::test_structured_selector::offer_named(
            value.value_type(),
            conduit_core::PortDirection::Output,
            kind,
            "unused",
        );
        offer.outputs[0].port_id = conduit_core::port_id(port);
        offer.outputs[0].temporal = PortTemporal::Current;
        offer.capability_id = CapabilityId::from(kind);
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
            .insert(KindProjection {
                kind_id: conduit_core::kind_id(kind),
                kind_contract_revision: KindIdentity::from("conduit-test/structured-source@1"),
                inputs: Vec::new(),
                outputs: offer.outputs.clone(),
                configuration: vec![KindConfigurationField {
                    key: "value".into(),
                    default_value: conduit_core::ConfigurationValue::Text(String::new()),
                    rule: KindConfigurationRule::TextBytes {
                        maximum: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 2,
                    },
                }],
            })
            .unwrap();
        offers.push(offer);
    }
    let mut sink_offer = crate::installed_std::test_local_model_io::sink_offer(
        conduit_semantic_catalog::visual_impression_type()
            .profile()
            .unwrap()
            .value_kind()
            .as_str(),
    );
    sink_offer.inputs[0].temporal = PortTemporal::Current;
    sink_offer.limits.max_queue_bytes = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
    startup
        .insert(KindSignature {
            kind: sink_offer.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: sink_offer.kind_id.clone(),
            kind_contract_revision: sink_offer.kind_contract_revision.clone(),
            inputs: sink_offer.inputs.clone(),
            outputs: Vec::new(),
            configuration: Default::default(),
        })
        .unwrap();
    (startup, profile, offers, sink_offer)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
