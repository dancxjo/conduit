use super::*;

#[test]
fn unchanged_spoken_house_form_plans_across_three_exact_hosts_and_lines() {
    let model = local_offer();
    let host = StdHost::new_with_composition(
        StdHostConfig {
            host_id: HostId::from("host/house-template"),
            boot_id: BootId::from("boot/house-template"),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::minimal(),
    );
    let mut template = host.advertisement().clone();
    template
        .resources
        .extend(crate::hosted_local_model::resource_offers(&model.limits));
    template.resources.extend([
        conduit_core::resource_offer(
            "std/microphone/fixture",
            conduit_std_offers::MICROPHONE_CAPTURE_RESOURCE_CLASS,
            1,
        ),
        conduit_core::resource_offer(
            "std/whisper-process",
            conduit_std_offers::WHISPER_PROCESS_RESOURCE_CLASS,
            1,
        ),
        crate::hosted_speech::process_resource_offer(),
        conduit_core::resource_offer(
            "std/audio/alsa/fixture/card-FIXTURE/device-0",
            conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS,
            1,
        ),
    ]);
    template
        .capabilities
        .extend(model.capability_offers().unwrap());
    template.capabilities.extend([
        conduit_std_offers::house_prompt_std_offer(),
        conduit_std_offers::model_result_to_text_std_offer(),
        conduit_std_offers::address_detect_offer(),
        conduit_std_offers::recognition_to_text_std_offer(),
        conduit_std_offers::text_literal_offer(),
        conduit_std_offers::microphone_clip_offer(),
        conduit_std_offers::whisper_clip_speech_offer(),
        conduit_std_offers::piper_speech_offer(),
        conduit_std_offers::audio_convert_pcm_profile_offer(),
        conduit_std_offers::audio_play_alsa_hw_offer(),
    ]);
    template
        .capabilities
        .extend(crate::installed_std::test_local_model_io::house_source_offers());
    template.capabilities.retain(|offer| {
        offer.implementation.implementation_id.as_str()
            != conduit_std_offers::DETERMINISTIC_SPEECH_IMPLEMENTATION
    });
    template.resources.sort();
    template.capabilities.sort_by(|left, right| {
        left.capability_id
            .as_str()
            .cmp(right.capability_id.as_str())
    });

    let exact = crate::distributed_house_plan::exact_distributed_spoken_house_plan(&template)
        .expect("unchanged House Form plans across exact Hosts");
    assert_eq!(exact.plan.fragments.len(), 3);
    assert_eq!(exact.lines.len(), 2);
    assert_eq!(exact.plan.checked_form_id.as_str(), exact.checked_form_id);
    for fragment in &exact.plan.fragments {
        assert!(exact.hosts.iter().any(|host| {
            host.host_id == fragment.host_id
                && host.boot_id == fragment.boot_id
                && host.offer_generation == fragment.offer_generation
        }));
    }
    let remote_connections = exact
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| fragment.connections.iter())
        .filter(|connection| connection.selected_line.is_some())
        .collect::<Vec<_>>();
    assert_eq!(remote_connections.len(), exact.lines.len() * 2);
    for connection in remote_connections {
        let selected = connection.selected_line.as_ref().unwrap();
        assert_eq!(
            selected.binding.base.as_str(),
            crate::distributed_house_plan::REMOTE_BASE
        );
        assert_eq!(
            selected.binding.limits.maximum_payload_bytes,
            connection.byte_capacity
        );
        assert_eq!(
            selected.binding.limits.maximum_in_flight_items,
            connection.item_capacity
        );
    }

    let mut line_endpoints = Vec::new();
    for line in &exact.lines {
        let source = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == line.binding.source.host_id)
            .unwrap();
        let sink = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == line.binding.sink.host_id)
            .unwrap();
        let connection = source
            .connections
            .iter()
            .find(|connection| {
                connection
                    .selected_line
                    .as_ref()
                    .is_some_and(|selected| selected.line_id == line.line_id)
            })
            .unwrap();
        conduit_wire::SessionBinding::from_planned_connection(
            exact.plan.plan_id.clone(),
            source.fragment_id.clone(),
            sink.fragment_id.clone(),
            connection,
        )
        .expect("every distributed House Line admits its exact session contract and limits");
    }

    let mut runtimes = exact
        .hosts
        .iter()
        .map(|host| {
            let fragment = exact
                .plan
                .fragments
                .iter()
                .find(|fragment| fragment.host_id == host.host_id)
                .unwrap();
            crate::InstalledRemoteFragment::prepare(host, fragment, 1)
                .expect("every distributed House fragment prepares in the one std kernel")
        })
        .collect::<Vec<_>>();
    for line in &exact.lines {
        let source_index = exact
            .hosts
            .iter()
            .position(|host| host.host_id == line.binding.source.host_id)
            .unwrap();
        let sink_index = exact
            .hosts
            .iter()
            .position(|host| host.host_id == line.binding.sink.host_id)
            .unwrap();
        assert!(source_index < sink_index);
        let (sources, sinks) = runtimes.split_at_mut(sink_index);
        let source = &mut sources[source_index];
        let sink = &mut sinks[0];
        let source_endpoint = source
            .sessions()
            .iter()
            .find(|session| session.binding().attachment.line_id == line.line_id)
            .unwrap()
            .endpoint;
        let sink_endpoint = sink
            .sessions()
            .iter()
            .find(|session| session.binding().attachment.line_id == line.line_id)
            .unwrap()
            .endpoint;
        crate::remote_cord_sessions::activate_in_process(
            source.sessions_mut().get_mut(source_endpoint).unwrap(),
            sink.sessions_mut().get_mut(sink_endpoint).unwrap(),
        )
        .expect("every exact distributed House Line reaches Ready");
        line_endpoints.push((source_index, source_endpoint, sink_index, sink_endpoint));
    }

    let capture_clip = crate::installed_std::test_local_model_io::recorded_house_audio_clip()
        .expect("recorded House PCM clip fixture is canonical");
    let capture_egress = line_endpoints
        .iter()
        .find(|(source, _, sink, _)| *source == 0 && *sink == 1)
        .copied()
        .unwrap();
    let capture_transfer = (0..32)
        .find_map(|_| {
            if let Some(transfer) = runtimes[0].next_egress(capture_egress.1).unwrap() {
                return Some(transfer);
            }
            if let Some(request) = runtimes[0].next_host_request() {
                let work = runtimes[0].describe_host_request(request).unwrap();
                assert_eq!(
                    work.contract_id.as_str(),
                    conduit_std_offers::MICROPHONE_CLIP_OPERATION
                );
                assert_eq!(work.input, b"capture");
                complete_remote_work(&mut runtimes[0], work, Some(capture_clip.clone()));
            }
            let _ = runtimes[0].step().unwrap();
            None
        })
        .expect("capture fragment emits its exact bounded clip");
    assert_eq!(capture_transfer.bytes, capture_clip);
    runtimes[0].accept_egress(&capture_transfer).unwrap();
    assert!(matches!(
        runtimes[1]
            .admit_ingress(
                capture_egress.3,
                capture_transfer.sequence,
                &capture_transfer.bytes
            )
            .unwrap(),
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
    ));
    assert_eq!(
        runtimes[1]
            .admit_ingress(
                capture_egress.3,
                capture_transfer.sequence + 1,
                &capture_transfer.bytes,
            )
            .unwrap(),
        conduit_kernel::scheduler::RemoteIngressOutcome::Full {
            sequence: capture_transfer.sequence + 1
        }
    );
    runtimes[0].deliver_egress(&capture_transfer).unwrap();

    let response = b"The upstairs temperature is 21 C.".to_vec();
    let mut recognized = None::<String>;
    let mut addresses = None::<conduit_text::AddressSet>;
    let mut detection = None::<conduit_text::AddressDetection>;
    let mut context = None::<Vec<conduit_ai::WiredHouseContextItem>>;
    let cognition_egress = line_endpoints
        .iter()
        .find(|(source, _, sink, _)| *source == 1 && *sink == 2)
        .copied()
        .unwrap();
    let response_transfer = (0..128)
        .find_map(|_| {
            if let Some(transfer) = runtimes[1].next_egress(cognition_egress.1).unwrap() {
                return Some(transfer);
            }
            if let Some(request) = runtimes[1].next_host_request() {
                let work = runtimes[1].describe_host_request(request).unwrap();
                let output = match work.contract_id.as_str() {
                    conduit_std_offers::WHISPER_CLIP_SPEECH_OPERATION => {
                        assert_eq!(work.input, capture_clip);
                        Some(
                            conduit_tongues::encode_speech_recognition_result(
                                &conduit_tongues::SpeechRecognitionResult {
                                    disposition:
                                        conduit_tongues::SpeechRecognitionDisposition::Recognized,
                                    text: Some(
                                        "Rosehip House, what is the temperature upstairs?".into(),
                                    ),
                                    audio_sha256: sha2::Sha256::digest(&work.input).into(),
                                },
                            )
                            .unwrap(),
                        )
                    }
                    conduit_std_offers::RECOGNITION_TO_TEXT_OPERATION => {
                        Some(conduit_tongues::project_recognized_text(&work.input).unwrap())
                    }
                    conduit_std_offers::ADDRESS_DETECT_RECOGNIZED_OPERATION => {
                        recognized = Some(String::from_utf8(work.input.clone()).unwrap());
                        match (&recognized, &addresses) {
                            (Some(recognized), Some(addresses)) => Some(
                                conduit_text::encode_address_detection(
                                    &addresses.detect(recognized).unwrap(),
                                )
                                .unwrap(),
                            ),
                            _ => None,
                        }
                    }
                    conduit_std_offers::ADDRESS_DETECT_ADDRESSES_OPERATION => {
                        addresses = Some(conduit_text::decode_address_set(&work.input).unwrap());
                        match (&recognized, &addresses) {
                            (Some(recognized), Some(addresses)) => Some(
                                conduit_text::encode_address_detection(
                                    &addresses.detect(recognized).unwrap(),
                                )
                                .unwrap(),
                            ),
                            _ => None,
                        }
                    }
                    conduit_std_offers::HOUSE_PROMPT_DETECTION_OPERATION => {
                        detection =
                            Some(conduit_tongues::decode_address_detection(&work.input).unwrap());
                        match (&detection, &context) {
                            (Some(detection), Some(context)) => Some(
                                conduit_tongues::prepare_house_generation_request(
                                    detection, context, 2_048,
                                )
                                .unwrap()
                                .encoded_request
                                .into_bytes(),
                            ),
                            _ => None,
                        }
                    }
                    conduit_std_offers::HOUSE_PROMPT_CONTEXT_OPERATION => {
                        context =
                            Some(conduit_tongues::decode_wired_house_context(&work.input).unwrap());
                        match (&detection, &context) {
                            (Some(detection), Some(context)) => Some(
                                conduit_tongues::prepare_house_generation_request(
                                    detection, context, 2_048,
                                )
                                .unwrap()
                                .encoded_request
                                .into_bytes(),
                            ),
                            _ => None,
                        }
                    }
                    conduit_ai::LOCAL_MODEL_OPERATION => Some(
                        serde_json::to_vec(&conduit_ai::ModelDerivedResult {
                            provenance: conduit_ai::ModelResultProvenance::ModelDerived,
                            payload_kind: conduit_ai::GENERATED_RESULT_VALUE_KIND.into(),
                            payload: response.clone(),
                            implementation_identity: "fixture/model".into(),
                            request_identity: "request/distributed-house".into(),
                            run_identity: "run/distributed-house".into(),
                            confidence: None,
                            disposition: conduit_ai::ModelResultDisposition::Produced,
                            determinism: LlmDeterminismProfile::ProviderNondeterministic,
                            accounting: conduit_ai::ModelWorkAccounting {
                                input_bytes: work.input.len() as u64,
                                context_items: 1,
                                output_bytes: response.len() as u64,
                                work_units: 1,
                                history_items: 0,
                            },
                        })
                        .unwrap(),
                    ),
                    conduit_std_offers::MODEL_RESULT_TO_TEXT_OPERATION => {
                        Some(conduit_ai::project_generated_text(&work.input).unwrap())
                    }
                    other => panic!("unexpected distributed cognition host operation: {other}"),
                };
                complete_remote_work(&mut runtimes[1], work, output);
            }
            let _ = runtimes[1].step().unwrap();
            None
        })
        .expect("cognition fragment emits the exact projected model response");
    assert_eq!(response_transfer.bytes, response);
    runtimes[1].accept_egress(&response_transfer).unwrap();
    assert!(matches!(
        runtimes[2]
            .admit_ingress(
                cognition_egress.3,
                response_transfer.sequence,
                &response_transfer.bytes,
            )
            .unwrap(),
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
    ));
    runtimes[1].deliver_egress(&response_transfer).unwrap();
    let speech_work = (0..32)
        .find_map(|_| {
            if let Some(request) = runtimes[2].next_host_request() {
                return Some(runtimes[2].describe_host_request(request).unwrap());
            }
            let _ = runtimes[2].step().unwrap();
            None
        })
        .expect("output fragment requests speech synthesis");
    assert_eq!(
        speech_work.contract_id.as_str(),
        conduit_std_offers::PIPER_SPEECH_OPERATION
    );
    assert_eq!(speech_work.input, response);

    let capture_fragment = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == exact.hosts[0].host_id)
        .unwrap();
    let mut stale_capture = exact.hosts[0].clone();
    stale_capture.boot_id = BootId::from("boot/house-capture-stale");
    assert_eq!(
        crate::InstalledRemoteFragment::prepare(&stale_capture, capture_fragment, 1)
            .err()
            .unwrap(),
        "remote fragment preparation requires its exact Host and Boot"
    );
    let capture_endpoint = runtimes[0].sessions().iter().next().unwrap().endpoint;
    runtimes[0]
        .fail_remote_line(capture_endpoint, 71)
        .expect("exact capture Line loss is retained before cancellation");
    assert!(runtimes[0]
        .next_egress(capture_endpoint)
        .unwrap_err()
        .contains("Cancelled"));
    runtimes[2]
        .cancel()
        .expect("playback fragment cancellation stays fragment-local");

    let mut startup = conduit_form::StartupCatalog::new();
    let mut profiles = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles).unwrap();
    let source = include_str!("../../../forms/greet/main.conduit");
    let checked =
        conduit_form::check_syntax_document(&conduit_form::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, "welcome", &profiles).unwrap();
    let mut unrelated_host = StdHost::new();
    let unrelated_plan = unrelated_host.plan_expanded_local(&expanded).unwrap();
    let unrelated_report = unrelated_host
        .run_fragment_to(
            unrelated_plan.fragments[0].clone(),
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .expect("unrelated admitted Form remains runnable after House machinery loss");
    assert!(matches!(
        unrelated_report
            .observations
            .last()
            .map(|observation| &observation.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));

    let accepted_plan = exact.plan.clone();
    for lost in [
        crate::distributed_house_plan::DistributedHouseRole::Cognition,
        crate::distributed_house_plan::DistributedHouseRole::Output,
    ] {
        let refusal = crate::distributed_house_plan::replan_distributed_spoken_house_after_loss(
            &template, lost,
        )
        .unwrap_err();
        assert!(refusal.contains("UnknownHost"), "{refusal}");
        assert_eq!(exact.plan, accepted_plan);
    }

    let replacement =
        crate::distributed_house_plan::replan_distributed_spoken_house_after_replacement(
            &template,
            crate::distributed_house_plan::DistributedHouseRole::Capture,
            BootId::from("boot/house-capture-replacement"),
            OfferGeneration(2),
        )
        .expect("fresh capture Host truth produces a new exact Plan");
    let prior_capture = &exact.hosts[0];
    let replacement_capture = &replacement.hosts[0];
    assert_eq!(replacement_capture.host_id, prior_capture.host_id);
    assert_ne!(replacement_capture.boot_id, prior_capture.boot_id);
    assert!(replacement_capture.offer_generation > prior_capture.offer_generation);
    let prior_pools = prior_capture
        .resources
        .iter()
        .map(|resource| resource.pool_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let replacement_pools = replacement_capture
        .resources
        .iter()
        .map(|resource| resource.pool_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(prior_pools.is_disjoint(&replacement_pools));
    let bound_pools = replacement.plan.fragments[0]
        .placements
        .iter()
        .flat_map(|placement| &placement.resources)
        .map(|resource| resource.pool_id.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(!bound_pools.is_empty());
    assert!(bound_pools.is_subset(&replacement_pools));
    assert_eq!(replacement.plan.checked_form_id, exact.plan.checked_form_id);
    assert_ne!(replacement.plan.plan_id, exact.plan.plan_id);
    assert_eq!(exact.plan, accepted_plan);
    let stale_fragment_refusal = match crate::InstalledRemoteFragment::prepare(
        replacement_capture,
        &exact.plan.fragments[0],
        2,
    ) {
        Ok(_) => panic!("old fragment must refuse the replacement Boot"),
        Err(error) => error,
    };
    assert_eq!(
        stale_fragment_refusal,
        "remote fragment preparation requires its exact Host and Boot"
    );
}
