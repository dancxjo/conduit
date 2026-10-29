use crate::hosted_vision::{FiniteHostedVisionBase, HostedVisionFrame};
use crate::{StdHost, StdHostComposition, StdHostConfig};
use conduit_core::{
    authority_grant, process_owned_line_offer_with_limits, BaseImplementationId, BootId,
    CapabilityId, GearId, HostId, KindIdentity, LinkLimits, OfferGeneration, PortTemporal,
    ProtectedResourceAccess, ProtectedResourceCommitPolicy, ProtectedResourceGrant,
    ResourceBindingRoleId, ResourceClassId, ResourceHandleId,
};
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, KindConfigurationField,
    KindConfigurationRule, KindProjection, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::BTreeMap;
mod replacement;

#[test]
fn image_crosses_an_exact_line_into_the_production_vision_back() {
    run_vision_fixture(false);
}

#[test]
fn cancelled_pending_vision_owner_retires_before_fresh_stdhost_play() {
    run_vision_fixture(true);
}

fn run_vision_fixture(retire_pending: bool) {
    let image = conduit_semantic_catalog::deterministic_vision_fixture()
        .unwrap()
        .image;
    let encoded = image.canonical_bytes().unwrap();
    let (_, resource, width, height) =
        crate::hosted_vision::decode_image_resource_for_test(&encoded);
    let vision = FiniteHostedVisionBase::new(
        vec![HostedVisionFrame {
            canonical_image: encoded.clone(),
            resource,
            width,
            height,
            grayscale_pixels: vec![0; usize::from(width) * usize::from(height)],
        }],
        width,
        height,
        4,
        "finite-image-residence/remote-vision",
    )
    .unwrap();
    let mut source_host = host("vision-edge");
    let mut vision_host = StdHost::new_with_finite_vision(
        StdHostConfig {
            host_id: HostId::from("vision-worker"),
            boot_id: BootId::from("vision-worker-boot"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        vision,
    )
    .unwrap();
    let (startup, profile, source_offer, sink_offer) = catalogs(&image);
    source_host
        .advertisement
        .capabilities
        .push(source_offer.clone());
    source_host
        .advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    source_host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(&source_host.advertisement).unwrap();
    vision_host
        .advertisement
        .capabilities
        .push(sink_offer.clone());
    vision_host
        .advertisement
        .capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    vision_host.kernel_resources =
        crate::kernel_preparation::KernelResourceLedger::new(&vision_host.advertisement).unwrap();

    let source = format!(
        "plot remote-vision {{\n source: conduit-test/vision-remote-source(value = \"{}\")\n motion: vision/local-motion\n sink: conduit-test/local-model-result\n source.image >> motion.image\n motion.motions >> sink.value\n}}\n",
        hex(&encoded)
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "remote-vision", &profile).unwrap();
    let source_advertisement = source_host.advertisement().clone();
    let vision_advertisement = vision_host.advertisement().clone();
    let motion_offer = vision_advertisement
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::VISION_MOTION_KIND)
        .unwrap();
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            placement("source", &source_advertisement, &source_offer.capability_id),
            placement("motion", &vision_advertisement, &motion_offer.capability_id),
            placement("sink", &vision_advertisement, &sink_offer.capability_id),
        ]),
    };
    let motion_gear = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::VISION_MOTION_KIND)
        .unwrap();
    let authority = authority_grant(
        "grant/vision/remote-read",
        &motion_offer.authority_requirements[0],
        vision_advertisement.host_id.clone(),
        vision_advertisement.boot_id.clone(),
        motion_offer.capability_id.clone(),
    );
    let resource = ProtectedResourceGrant {
        role_id: ResourceBindingRoleId::from(conduit_std_offers::LOCAL_VISION_RESOURCE_ROLE),
        handle_id: ResourceHandleId::from("handle/finite-image-residence/remote-vision"),
        gear_id: motion_gear.gear_id.clone(),
        host_id: vision_advertisement.host_id.clone(),
        boot_id: vision_advertisement.boot_id.clone(),
        capability_id: motion_offer.capability_id.clone(),
        class_id: ResourceClassId::from(conduit_std_offers::LOCAL_VISION_RESOURCE_CLASS),
        access: ProtectedResourceAccess::ReadExisting,
        maximum_bytes: conduit_semantic_catalog::MAXIMUM_LOCAL_CV_PIXELS as u64,
        commit_policy: ProtectedResourceCommitPolicy::NotApplicable,
    };
    let maximum = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
    let mut line = process_owned_line_offer_with_limits(
        "line/remote-vision-image",
        "binding/remote-vision-image",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "instance/remote-vision-image",
        &source_advertisement,
        &vision_advertisement,
        LinkLimits {
            maximum_in_flight_items: 1,
            maximum_payload_bytes: maximum,
            maximum_buffered_bytes: maximum,
            maximum_frame_bytes: maximum + 8_192,
        },
    );
    line.contract.scope = conduit_core::LineScope::LocalNetwork;
    line.contract.security = conduit_core::LineSecurity::PlaintextNetwork;
    let line_candidates = BTreeMap::from([(
        (
            GearId::from("remote-vision/source"),
            GearId::from("remote-vision/motion"),
        ),
        vec![line.line_id.clone()],
    )]);
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &[source_advertisement.clone(), vision_advertisement.clone()],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: maximum,
            authority_grants: &[authority],
            protected_resource_grants: &[resource],
            line_offers: std::slice::from_ref(&line),
        },
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 2);
    let source_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == source_advertisement.host_id)
        .unwrap();
    let vision_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == vision_advertisement.host_id)
        .unwrap();
    let mut source_runtime = source_host
        .prepare_remote_fragment(source_fragment)
        .unwrap();
    let mut vision_runtime = vision_host
        .prepare_remote_fragment(vision_fragment)
        .unwrap();
    feed_vision(&mut source_runtime, &mut vision_runtime, &encoded);

    let mut retired_play = None;
    let mut replacement_request = None;
    if retire_pending {
        // These descriptors are meaningful only with this trusted mutable owner.
        // They are not capabilities to transplant into a replacement runtime.
        let request = (0..32)
            .find_map(|_| {
                if let Some(request) = vision_runtime.next_host_request() {
                    return Some(request);
                }
                vision_runtime.step().unwrap();
                None
            })
            .expect("A reaches a genuine dispatched Vision request");
        let input = vision_runtime
            .host_request_view(request)
            .unwrap()
            .input
            .to_vec();
        assert_eq!(input, encoded);
        let staged = vision_runtime
            .store_host_value(b"retired-A-uncommitted-output")
            .unwrap();
        let old_play = vision_runtime.identity().clone();
        retired_play = Some(old_play.clone());
        let sibling_play = source_runtime.identity().clone();
        vision_runtime.cancel().unwrap();
        let refused = vision_runtime
            .complete_host_call(
                request,
                conduit_kernel::HostCallOutcome {
                    disposition: conduit_kernel::HostCallDisposition::Completed,
                    output: Some(staged),
                    failure: None,
                },
            )
            .unwrap_err();
        assert!(refused.contains("HostCallCompletionRejected"), "{refused}");
        assert!(vision_runtime.next_host_request().is_none());
        assert!(matches!(
            vision_runtime.step().unwrap(),
            conduit_kernel::scheduler::SchedulerStatus::Cancelled
        ));
        // A's cancellation does not cancel the independently admitted source.
        assert_eq!(source_runtime.identity(), &sibling_play);
        assert!(!matches!(
            source_runtime.step().unwrap(),
            conduit_kernel::scheduler::SchedulerStatus::Cancelled
        ));
        vision_host.release_remote_fragment(vision_runtime).unwrap();
        source_host.release_remote_fragment(source_runtime).unwrap();
        vision_runtime = vision_host
            .prepare_remote_fragment(vision_fragment)
            .unwrap();
        source_runtime = source_host
            .prepare_remote_fragment(source_fragment)
            .unwrap();
        assert_ne!(source_runtime.identity(), &sibling_play);
        assert_eq!(source_runtime.identity().host_id, sibling_play.host_id);
        assert_eq!(source_runtime.identity().boot_id, sibling_play.boot_id);
        assert!(source_runtime.identity().play_sequence > sibling_play.play_sequence);
        assert_ne!(vision_runtime.identity(), &old_play);
        assert_eq!(vision_runtime.identity().host_id, old_play.host_id);
        assert_eq!(vision_runtime.identity().boot_id, old_play.boot_id);
        assert!(vision_runtime.identity().play_sequence > old_play.play_sequence);
        // Fresh B obtains its own ingress and request, then uses normal provider
        // completion below. No raw A descriptor is ever submitted to B.
        feed_vision(&mut source_runtime, &mut vision_runtime, &encoded);
        replacement_request = Some(replacement::pending(
            &mut vision_runtime,
            &mut source_runtime,
            request,
            &encoded,
        ));
    }

    let mut completed_vision_call = false;
    let mut provider_completions = 0;
    if let Some(request) = replacement_request {
        completed_vision_call = vision_host
            .complete_remote_vision_host_call(&mut vision_runtime, request, 71)
            .unwrap();
        provider_completions += usize::from(completed_vision_call);
        assert_eq!(replacement::completion_count(&vision_runtime, request), 1);
    }
    let drained = (0..32).any(|_| {
        if let Some(request) = vision_runtime.next_host_request() {
            completed_vision_call = vision_host
                .complete_remote_vision_host_call(&mut vision_runtime, request, 71)
                .unwrap();
            provider_completions += usize::from(completed_vision_call);
        }
        matches!(
            vision_runtime.step().unwrap(),
            conduit_kernel::scheduler::SchedulerStatus::Drained
        )
    });
    assert!(completed_vision_call);
    assert!(drained);
    assert_eq!(provider_completions, 1);
    if let Some(request) = replacement_request {
        assert_eq!(replacement::completion_count(&vision_runtime, request), 1);
    }
    if retire_pending {
        println!(
            "CONDUIT_STD_HOSTCALL_RETIREMENT {}",
            serde_json::json!({
                "schema": "conduit-std-hostcall-retirement-v1",
                "retired_play": retired_play,
                "replacement_play": vision_runtime.identity(),
                "cancelled_owner_completion": "HostCallCompletionRejected",
                "replacement_provider_completions": provider_completions,
                "replacement_drained": drained,
                "replacement_pending_descriptor_matches_retired": true,
                "replacement_pending_input_and_signs_unchanged": true,
                "replacement_output_before_fresh_completion": "no motion output routing",
                "retired_receiver": "consumed by public release; compile-fail regression",
                "async_completion_class": "unavailable on synchronous Vision owner",
                "sibling_control": {
                    "before_release": "A source sibling remained live until explicitly released",
                    "replacement_pending": "fresh B source sibling identity and Signs unchanged"
                },
                "not_established": ["cross-owner descriptor replay", "asynchronous callback revocation", "Watch", "hostile confinement"]
            })
        );
    }

    let mut failed = source_host
        .prepare_remote_fragment(source_fragment)
        .err()
        .unwrap();
    assert!(failed.contains("combined active-instance limit exceeded"));
    let source_endpoint = source_runtime.sessions().iter().next().unwrap().endpoint;
    source_runtime
        .fail_remote_line(source_endpoint, 73)
        .unwrap();
    failed = source_runtime.next_egress(source_endpoint).unwrap_err();
    assert!(failed.contains("Cancelled"));
}

fn feed_vision(
    source_runtime: &mut crate::AdmittedRemoteFragment,
    vision_runtime: &mut crate::AdmittedRemoteFragment,
    encoded: &[u8],
) {
    let source_endpoint = source_runtime.sessions().iter().next().unwrap().endpoint;
    let vision_endpoint = vision_runtime.sessions().iter().next().unwrap().endpoint;
    crate::remote_cord_sessions::activate_in_process(
        source_runtime
            .sessions_mut()
            .get_mut(source_endpoint)
            .unwrap(),
        vision_runtime
            .sessions_mut()
            .get_mut(vision_endpoint)
            .unwrap(),
    )
    .unwrap();

    let transfer = (0..16)
        .find_map(|_| {
            if let Some(transfer) = source_runtime.next_egress(source_endpoint).unwrap() {
                return Some(transfer);
            }
            let _ = source_runtime.step().unwrap();
            None
        })
        .expect("authored image reaches the admitted remote Cord");
    assert_eq!(transfer.bytes, encoded);
    source_runtime.accept_egress(&transfer).unwrap();
    assert_eq!(
        vision_runtime
            .admit_ingress(vision_endpoint, transfer.sequence, &transfer.bytes)
            .unwrap(),
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted {
            sequence: transfer.sequence
        }
    );
    source_runtime.deliver_egress(&transfer).unwrap();
    vision_runtime.close_ingress(vision_endpoint).unwrap();
}

fn host(id: &str) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(id),
        boot_id: BootId::from(format!("{id}-boot")),
        offer_generation: OfferGeneration(1),
    })
}

fn placement(
    gear: &str,
    host: &conduit_core::HostAdvertisement,
    capability_id: &CapabilityId,
) -> (GearId, PlacementChoice) {
    (
        GearId::from(format!("remote-vision/{gear}")),
        PlacementChoice {
            host_id: host.host_id.clone(),
            capability_id: capability_id.clone(),
        },
    )
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
    let source_kind = "conduit-test/vision-remote-source";
    let mut source = crate::installed_std::test_structured_selector::offer_named(
        image.value_type(),
        conduit_core::PortDirection::Output,
        source_kind,
        "unused",
    );
    source.outputs[0].port_id = conduit_core::port_id("image");
    source.outputs[0].temporal = PortTemporal::Current;
    source.capability_id = CapabilityId::from(source_kind);
    crate::installed_std::test_structured_selector::bind_text_configuration(
        &mut source,
        "value",
        String::new(),
        conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 2,
    );
    let mut sink = crate::installed_std::test_local_model_io::sink_offer(
        conduit_semantic_catalog::vision_motions_type()
            .profile()
            .unwrap()
            .value_kind()
            .as_str(),
    );
    sink.inputs[0].temporal = PortTemporal::Current;
    sink.limits.max_queue_bytes = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
    startup
        .insert(KindSignature {
            kind: source.kind_id.as_str().into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: "Text".into(),
                default: Some(String::new()),
            }],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: source.kind_id.clone(),
            kind_contract_revision: KindIdentity::from("conduit-test/structured-source@1"),
            inputs: source.inputs.clone(),
            outputs: source.outputs.clone(),
            configuration: vec![KindConfigurationField {
                key: "value".into(),
                default_value: conduit_core::ConfigurationValue::Text(String::new().into()),
                rule: KindConfigurationRule::TextBytes {
                    maximum: conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32 * 2,
                },
            }],
        })
        .unwrap();
    startup
        .insert(KindSignature {
            kind: sink.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: sink.kind_id.clone(),
            kind_contract_revision: sink.kind_contract_revision.clone(),
            inputs: sink.inputs.clone(),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .unwrap();
    (startup, profile, source, sink)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
