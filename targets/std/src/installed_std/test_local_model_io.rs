use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindIdentity, PlannedGear, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_form::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_kernel::{OperationAction, OperationInput, PortId, ValueRef, ValueStorage};

const SOURCE_KIND: &str = "conduit-test/local-model-request";
const SOURCE_REVISION: &str = "conduit-test/local-model-request@1";
pub(crate) const HOUSE_RECOGNIZED_SOURCE_KIND: &str = "conduit-test/house-recognized-source";
const HOUSE_RECOGNIZED_SOURCE_REVISION: &str = "conduit-test/house-recognized-source@1";
pub(crate) const HOUSE_AUDIO_SOURCE_KIND: &str = "conduit-test/house-audio-source";
const HOUSE_AUDIO_SOURCE_REVISION: &str = "conduit-test/house-audio-source@1";
pub(crate) const HOUSE_AUDIO_CLIP_SOURCE_KIND: &str = "conduit-test/house-audio-clip-source";
const HOUSE_AUDIO_CLIP_SOURCE_REVISION: &str = "conduit-test/house-audio-clip-source@1";
#[cfg(feature = "local-model-proof")]
pub(crate) const HOUSE_AUDIO_CLIP_SOURCE_OPERATION: &str = super::PROOF_PCM_CLIP_SOURCE_OPERATION;
pub(crate) const HOUSE_ADDRESSES_SOURCE_KIND: &str = "conduit-test/house-addresses-source";
const HOUSE_ADDRESSES_SOURCE_REVISION: &str = "conduit-test/house-addresses-source@1";
pub(crate) const HOUSE_CONTEXT_SOURCE_KIND: &str = "conduit-test/house-context-source";
const HOUSE_CONTEXT_SOURCE_REVISION: &str = "conduit-test/house-context-source@1";
const SOURCE_IMPLEMENTATION: &str = "conduit-test/local-model-request-kernel@1";
const SINK_KIND: &str = "conduit-test/local-model-result";
const SINK_REVISION: &str = "conduit-test/local-model-result@1";
pub(crate) const HOUSE_TEXT_SINK_KIND: &str = "conduit-test/house-text-sink";
const HOUSE_TEXT_SINK_REVISION: &str = "conduit-test/house-text-sink@1";
pub(crate) const HOUSE_RECOGNITION_SINK_KIND: &str = "conduit-test/house-recognition-sink";
const HOUSE_RECOGNITION_SINK_REVISION: &str = "conduit-test/house-recognition-sink@1";
const SINK_IMPLEMENTATION: &str = "conduit-test/local-model-result-kernel@1";
const PROFILE: &str = "conduit-test/local-model-io-kernel@1";
const ARTIFACT: &str = "conduit-std-host/test-local-model-io@1";
#[cfg(test)]
const NAV_POSE_SOURCE: &str = "conduit-test/navigation-pose-source";
#[cfg(test)]
const NAV_POSE_SOURCE_REVISION: &str = "conduit-test/navigation-pose-source@1";
#[cfg(test)]
const NAV_GOAL_SOURCE: &str = "conduit-test/navigation-goal-source";
#[cfg(test)]
const NAV_GOAL_SOURCE_REVISION: &str = "conduit-test/navigation-goal-source@1";
#[cfg(test)]
const NAV_GRID_SOURCE: &str = "conduit-test/navigation-grid-source";
#[cfg(test)]
const NAV_GRID_SOURCE_REVISION: &str = "conduit-test/navigation-grid-source@1";
#[cfg(test)]
const NAV_TIME_SOURCE: &str = "conduit-test/navigation-time-source";
#[cfg(test)]
const NAV_TIME_SOURCE_REVISION: &str = "conduit-test/navigation-time-source@1";

pub(super) static TEST_LOCAL_MODEL_SOURCE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
};
pub(super) static TEST_LOCAL_MODEL_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct TestLocalModelSourceOperation {
    value: ValueRef,
    emitted: bool,
    hosted: bool,
}

pub(super) struct TestLocalModelSinkOperation {
    complete: bool,
}

impl TestLocalModelSourceOperation {
    pub(super) fn emit_or_complete(&self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else if self.hosted {
            OperationAction::RequestHostOperation {
                request: conduit_kernel::RequestId(0),
                operation: conduit_kernel::HostOperationId(0),
                input: conduit_kernel::BoundedValueRef::new(self.value, 1)
                    .expect("proof clip source marker is one admitted byte"),
            }
        } else {
            OperationAction::Emit {
                port: PortId(0),
                value: self.value,
            }
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.emitted = true;
        OperationAction::Complete
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.hosted && !self.emitted && request == conduit_kernel::RequestId(0) =>
            {
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (conduit_kernel::HostOperationDisposition::Completed, Some(output), None) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (conduit_kernel::HostOperationDisposition::Denied, _, _) => {
                        InstalledOperation::fail(143)
                    }
                    _ => InstalledOperation::fail(144),
                }
            }
            _ => InstalledOperation::fail(145),
        }
    }
}

impl TestLocalModelSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0), ..
            } if !self.complete => {
                self.complete = true;
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(142),
        }
    }
}

pub(crate) fn source_offer(value_kind: &str) -> CapabilityOffer {
    offer(
        SOURCE_KIND,
        SOURCE_REVISION,
        SOURCE_IMPLEMENTATION,
        value_kind,
        PortDirection::Output,
    )
}

#[cfg(feature = "local-model-proof")]
pub(crate) fn house_source_offers() -> [CapabilityOffer; 5] {
    [
        offer(
            HOUSE_AUDIO_SOURCE_KIND,
            HOUSE_AUDIO_SOURCE_REVISION,
            SOURCE_IMPLEMENTATION,
            conduit_audio::AUDIO_PCM_INFO_ID,
            PortDirection::Output,
        ),
        clip_source_offer(),
        offer(
            HOUSE_RECOGNIZED_SOURCE_KIND,
            HOUSE_RECOGNIZED_SOURCE_REVISION,
            SOURCE_IMPLEMENTATION,
            conduit_text::TEXT_VALUE_KIND,
            PortDirection::Output,
        ),
        offer(
            HOUSE_ADDRESSES_SOURCE_KIND,
            HOUSE_ADDRESSES_SOURCE_REVISION,
            SOURCE_IMPLEMENTATION,
            conduit_text::ADDRESS_SET_VALUE_KIND,
            PortDirection::Output,
        ),
        offer(
            HOUSE_CONTEXT_SOURCE_KIND,
            HOUSE_CONTEXT_SOURCE_REVISION,
            SOURCE_IMPLEMENTATION,
            conduit_tongues::WIRED_HOUSE_CONTEXT_VALUE_KIND,
            PortDirection::Output,
        ),
    ]
}

#[cfg(feature = "local-model-proof")]
fn clip_source_offer() -> CapabilityOffer {
    let mut offer = offer(
        HOUSE_AUDIO_CLIP_SOURCE_KIND,
        HOUSE_AUDIO_CLIP_SOURCE_REVISION,
        SOURCE_IMPLEMENTATION,
        conduit_audio::AUDIO_PCM_CLIP_INFO_ID,
        PortDirection::Output,
    );
    offer
        .host_operations
        .push(conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(
                HOUSE_AUDIO_CLIP_SOURCE_OPERATION,
            ),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        });
    offer.limits.max_queue_bytes = conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32;
    offer
}

pub(crate) fn sink_offer(value_kind: &str) -> CapabilityOffer {
    offer(
        SINK_KIND,
        SINK_REVISION,
        SINK_IMPLEMENTATION,
        value_kind,
        PortDirection::Input,
    )
}

#[cfg(feature = "local-model-proof")]
pub(crate) fn house_text_sink_offer() -> CapabilityOffer {
    offer(
        HOUSE_TEXT_SINK_KIND,
        HOUSE_TEXT_SINK_REVISION,
        SINK_IMPLEMENTATION,
        conduit_ai::TEXT_VALUE_KIND,
        PortDirection::Input,
    )
}

#[cfg(feature = "local-model-proof")]
pub(crate) fn house_recognition_sink_offer() -> CapabilityOffer {
    let mut offer = offer(
        HOUSE_RECOGNITION_SINK_KIND,
        HOUSE_RECOGNITION_SINK_REVISION,
        SINK_IMPLEMENTATION,
        conduit_tongues::SPEECH_RECOGNITION_RESULT_KIND,
        PortDirection::Input,
    );
    offer.limits.max_queue_bytes = conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32;
    offer
}

fn offer(
    kind: &str,
    revision: &str,
    implementation: &str,
    value_kind: &str,
    direction: PortDirection,
) -> CapabilityOffer {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    };
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("{kind}/{value_kind}")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: if direction == PortDirection::Input {
            vec![port.clone()]
        } else {
            Vec::new()
        },
        outputs: if direction == PortDirection::Output {
            vec![port]
        } else {
            Vec::new()
        },
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: 16_384,
        },
    }
}

pub(crate) fn install_catalog(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
    request_kind: &str,
    result_kind: &str,
) {
    for offer in [source_offer(request_kind), sink_offer(result_kind)] {
        install_offer(startup, catalog, offer);
    }
}

#[cfg(test)]
pub(crate) fn install_navigation_catalog(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
) {
    for offer in navigation_source_offers() {
        install_offer(startup, catalog, offer);
    }
    let request_kind = conduit_robotics::robotics_motion_request_type();
    install_offer(
        startup,
        catalog,
        sink_offer(request_kind.profile().unwrap().value_kind().as_str()),
    );
}

#[cfg(test)]
pub(crate) fn navigation_source_offers() -> [CapabilityOffer; 4] {
    let pose = conduit_semantic_catalog::navigation_pose_type();
    let goal = conduit_semantic_catalog::navigation_goal_type();
    let grid = conduit_semantic_catalog::navigation_traversability_type();
    let time = conduit_semantic_catalog::navigation_time_type();
    [
        source_offer_for(
            NAV_POSE_SOURCE,
            pose.profile().unwrap().value_kind().as_str(),
        ),
        source_offer_for(
            NAV_GOAL_SOURCE,
            goal.profile().unwrap().value_kind().as_str(),
        ),
        source_offer_for(
            NAV_GRID_SOURCE,
            grid.profile().unwrap().value_kind().as_str(),
        ),
        source_offer_for(
            NAV_TIME_SOURCE,
            time.profile().unwrap().value_kind().as_str(),
        ),
    ]
}

#[cfg(test)]
fn source_offer_for(kind: &str, value_kind: &str) -> CapabilityOffer {
    let revision = match kind {
        NAV_POSE_SOURCE => NAV_POSE_SOURCE_REVISION,
        NAV_GOAL_SOURCE => NAV_GOAL_SOURCE_REVISION,
        NAV_GRID_SOURCE => NAV_GRID_SOURCE_REVISION,
        NAV_TIME_SOURCE => NAV_TIME_SOURCE_REVISION,
        _ => unreachable!("navigation source inventory is closed"),
    };
    offer(
        kind,
        revision,
        SOURCE_IMPLEMENTATION,
        value_kind,
        PortDirection::Output,
    )
}

#[cfg(feature = "local-model-proof")]
pub(crate) fn install_house_source_catalog(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
) {
    for offer in house_source_offers() {
        install_offer(startup, catalog, offer);
    }
}

#[cfg(feature = "local-model-proof")]
pub(crate) fn install_house_text_sink_catalog(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
) {
    install_offer(startup, catalog, house_text_sink_offer());
}

#[cfg(feature = "local-model-proof")]
pub(crate) fn install_house_recognition_sink_catalog(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
) {
    install_offer(startup, catalog, house_recognition_sink_offer());
}

fn install_offer(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
    offer: CapabilityOffer,
) {
    startup
        .insert(KindSignature {
            kind: offer.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })
        .expect("test local-model IO startup Kind is unique");
    catalog
        .insert(KindProjection {
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            inputs: offer.inputs,
            outputs: offer.outputs,
            configuration: Default::default(),
        })
        .expect("test local-model IO Kind is unique");
}

fn validate(placement: &PlannedGear, direction: PortDirection) -> Result<(), String> {
    let (kind, revision, implementation) = if direction == PortDirection::Output {
        match placement.kind_id.as_str() {
            HOUSE_AUDIO_SOURCE_KIND => (
                HOUSE_AUDIO_SOURCE_KIND,
                HOUSE_AUDIO_SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
            ),
            HOUSE_AUDIO_CLIP_SOURCE_KIND => (
                HOUSE_AUDIO_CLIP_SOURCE_KIND,
                HOUSE_AUDIO_CLIP_SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
            ),
            HOUSE_RECOGNIZED_SOURCE_KIND => (
                HOUSE_RECOGNIZED_SOURCE_KIND,
                HOUSE_RECOGNIZED_SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
            ),
            HOUSE_ADDRESSES_SOURCE_KIND => (
                HOUSE_ADDRESSES_SOURCE_KIND,
                HOUSE_ADDRESSES_SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
            ),
            HOUSE_CONTEXT_SOURCE_KIND => (
                HOUSE_CONTEXT_SOURCE_KIND,
                HOUSE_CONTEXT_SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
            ),
            #[cfg(test)]
            NAV_POSE_SOURCE => (
                NAV_POSE_SOURCE,
                NAV_POSE_SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
            ),
            #[cfg(test)]
            NAV_GOAL_SOURCE => (
                NAV_GOAL_SOURCE,
                NAV_GOAL_SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
            ),
            #[cfg(test)]
            NAV_GRID_SOURCE => (
                NAV_GRID_SOURCE,
                NAV_GRID_SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
            ),
            #[cfg(test)]
            NAV_TIME_SOURCE => (
                NAV_TIME_SOURCE,
                NAV_TIME_SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
            ),
            _ => (SOURCE_KIND, SOURCE_REVISION, SOURCE_IMPLEMENTATION),
        }
    } else if placement.kind_id.as_str() == HOUSE_TEXT_SINK_KIND {
        (
            HOUSE_TEXT_SINK_KIND,
            HOUSE_TEXT_SINK_REVISION,
            SINK_IMPLEMENTATION,
        )
    } else if placement.kind_id.as_str() == HOUSE_RECOGNITION_SINK_KIND {
        (
            HOUSE_RECOGNITION_SINK_KIND,
            HOUSE_RECOGNITION_SINK_REVISION,
            SINK_IMPLEMENTATION,
        )
    } else {
        (SINK_KIND, SINK_REVISION, SINK_IMPLEMENTATION)
    };
    if placement.kind_id.as_str() != kind
        || placement.kind_contract_revision.as_str() != revision
        || placement.execution_profile_id.as_str() != PROFILE
        || placement.implementation_id.as_str() != implementation
        || placement.artifact_id.as_str() != ARTIFACT
    {
        return Err("planned local-model test endpoint identity mismatch".to_string());
    }
    Ok(())
}

fn source_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, PortDirection::Output)?;
    let hosted_clip = placement.kind_id.as_str() == HOUSE_AUDIO_CLIP_SOURCE_KIND;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: if hosted_clip {
            conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
        } else {
            4_096
        },
        host_requests: usize::from(hosted_clip),
        sign_items: 16,
        maximum_value_bytes: if hosted_clip {
            conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
        } else {
            4_096
        },
    })
}

fn sink_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, PortDirection::Input)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: 4_096,
    })
}

fn prepare_source(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, PortDirection::Output)?;
    #[cfg(test)]
    let request = if placement.kind_id.as_str() == HOUSE_AUDIO_CLIP_SOURCE_KIND {
        vec![0]
    } else if placement.kind_id.as_str() == NAV_POSE_SOURCE {
        conduit_semantic_catalog::encode_navigation_pose(&navigation_pose(), 0, 0)
            .map_err(|error| format!("encode navigation pose: {error:?}"))?
    } else if placement.kind_id.as_str() == NAV_GOAL_SOURCE {
        conduit_semantic_catalog::encode_navigation_goal(&navigation_goal())
            .map_err(|error| format!("encode navigation goal: {error:?}"))?
    } else if placement.kind_id.as_str() == NAV_GRID_SOURCE {
        conduit_semantic_catalog::encode_navigation_traversability(&navigation_grid())
            .map_err(|error| format!("encode navigation grid: {error:?}"))?
    } else if placement.kind_id.as_str() == NAV_TIME_SOURCE {
        conduit_semantic_catalog::encode_navigation_time(&navigation_time())
            .map_err(|error| format!("encode navigation time: {error:?}"))?
    } else {
        source_request(placement)?
    };
    #[cfg(not(test))]
    let request = if placement.kind_id.as_str() == HOUSE_AUDIO_CLIP_SOURCE_KIND {
        vec![0]
    } else {
        source_request(placement)?
    };
    let value = values
        .store(&request)
        .map_err(|error| format!("store local-model test request: {error:?}"))?;
    Ok(InstalledOperation::TestLocalModelSource(
        TestLocalModelSourceOperation {
            value,
            emitted: false,
            hosted: placement.kind_id.as_str() == HOUSE_AUDIO_CLIP_SOURCE_KIND,
        },
    ))
}

fn source_request(placement: &PlannedGear) -> Result<Vec<u8>, String> {
    let request = if placement.kind_id.as_str() == HOUSE_AUDIO_SOURCE_KIND {
        recorded_house_audio()?
    } else if placement.kind_id.as_str() == HOUSE_AUDIO_CLIP_SOURCE_KIND {
        recorded_house_audio_clip()?
    } else if placement.kind_id.as_str() == HOUSE_RECOGNIZED_SOURCE_KIND {
        b"Rosehip House, what is the temperature upstairs?".to_vec()
    } else if placement.kind_id.as_str() == HOUSE_ADDRESSES_SOURCE_KIND {
        conduit_text::encode_address_set(
            &conduit_text::AddressSet::new(&["Rosehip House", "Rosehip"])
                .map_err(|error| format!("build House address set: {error:?}"))?,
        )
        .map_err(|error| format!("encode House address set: {error:?}"))?
    } else if placement.outputs[0].value_kind.as_str()
        == conduit_tongues::WIRED_HOUSE_CONTEXT_VALUE_KIND
    {
        conduit_tongues::encode_wired_house_context(&[conduit_ai::WiredHouseContextItem {
            item_identity: "context/upstairs-temperature".into(),
            value_kind: "temperature/summary@1".into(),
            canonical_value: b"21 degrees Celsius, observed 18 seconds ago".to_vec(),
            provenance: conduit_ai::HouseContextProvenanceClass::ObservedSign,
            source_identity: "sign/temperature/42".into(),
        }])
        .map_err(|error| format!("encode House context: {error:?}"))?
    } else if placement.outputs[0].value_kind.as_str() == conduit_ai::SIMILARITY_QUERY_VALUE_KIND {
        serde_json::to_vec(&conduit_ai::SimilarityQuery {
            embedding: conduit_ai::Embedding {
                profile: conduit_ai::EmbeddingProfile {
                    identity: "embedding/vector-play-fixture".into(),
                    semantic_space_identity: "space/vector-play-fixture".into(),
                    model_identity: "model/vector-play-fixture".into(),
                    provider_identity: "provider/vector-play-fixture".into(),
                    dimensions: 3,
                    normalization: conduit_ai::EmbeddingNormalization::None,
                    compatible_metrics: conduit_ai::CompatibleMetrics {
                        cosine_similarity: true,
                        dot_product_similarity: true,
                        squared_euclidean_distance: true,
                    },
                },
                values: vec![1.0, 0.0, 0.0],
            },
            metric: conduit_ai::SimilarityMetric::CosineSimilarity,
            top_k: 2,
            threshold: None,
            filters: Vec::new(),
            temporal_intent: None,
        })
        .map_err(|error| format!("encode vector-search request: {error}"))?
    } else if placement.outputs[0].value_kind.as_str() == "llm/interpretation-request@1" {
        serde_json::to_vec(&conduit_ai::InterpretationRequest {
            evidence: vec![
                conduit_ai::InterpretationEvidence {
                    sign_id: conduit_core::SignId::from("sign/line/carrier-lost/7"),
                    observation: "carrier lost".into(),
                },
                conduit_ai::InterpretationEvidence {
                    sign_id: conduit_core::SignId::from("sign/peer/unreachable/8"),
                    observation: "peer unreachable".into(),
                },
                conduit_ai::InterpretationEvidence {
                    sign_id: conduit_core::SignId::from("sign/host/offer-fresh/9"),
                    observation: "fresh Host offer remains available".into(),
                },
            ],
            context: "explain the likely operational boundary without taking action".into(),
            temporal_reference: conduit_ai::TemporalReference {
                reference_at: 1_723_456_789_000,
                clock_basis: conduit_ai::ClockBasis::UnixEpochMilliseconds,
            },
            temporal_intent: Some(conduit_ai::TemporalRetrievalIntent::LatestEvidence),
        })
        .map_err(|error| format!("encode local-model interpretation request: {error}"))?
    } else if placement.outputs[0].value_kind.as_str()
        == conduit_presentation::GENERATIVE_PRESENTER_INPUT_KIND
    {
        serde_json::to_vec(&crate::hosted_local_model::ollama_present::proof_request()?)
            .map_err(|error| format!("encode generative Presenter request: {error}"))?
    } else {
        b"Conduit bounded local model request".to_vec()
    };
    Ok(request)
}

#[cfg(test)]
fn navigation_time() -> conduit_semantic_catalog::NavigationTime {
    conduit_semantic_catalog::NavigationTime {
        clock_identity: "clock/fixture".into(),
        now_ms: 500,
    }
}

#[cfg(test)]
fn navigation_pose() -> conduit_semantic_catalog::NavigationPose {
    conduit_semantic_catalog::NavigationPose {
        source_identity: "pose/fixture".into(),
        sample_sequence: 7,
        clock_identity: "clock/fixture".into(),
        frame: "map/local".into(),
        x_mm: 50,
        y_mm: 50,
        heading_microdegrees: 0,
        validity: conduit_semantic_catalog::Validity {
            observed_at_ms: 400,
            valid_until_ms: 600,
        },
    }
}

#[cfg(test)]
fn navigation_goal() -> conduit_semantic_catalog::NavigationGoal {
    conduit_semantic_catalog::NavigationGoal {
        identity: "goal/B".into(),
        clock_identity: "clock/fixture".into(),
        valid_until_ms: 1_000,
        target: conduit_semantic_catalog::GoalTarget::Reach {
            frame: "map/local".into(),
            x_mm: 250,
            y_mm: 250,
            heading_microdegrees: 0,
            position_tolerance_mm: 5,
            heading_tolerance_microdegrees: 2_000_000,
        },
    }
}

#[cfg(test)]
fn navigation_grid() -> conduit_semantic_catalog::Traversability4x4 {
    let mut cells = [conduit_semantic_catalog::TraversabilityCell::Free; 16];
    cells[1] = conduit_semantic_catalog::TraversabilityCell::Blocked;
    conduit_semantic_catalog::Traversability4x4 {
        source_identity: "grid/fixture".into(),
        sample_sequence: 11,
        clock_identity: "clock/fixture".into(),
        frame: "map/local".into(),
        origin_x_mm: 0,
        origin_y_mm: 0,
        cell_width_mm: 100,
        cell_height_mm: 100,
        validity: conduit_semantic_catalog::Validity {
            observed_at_ms: 400,
            valid_until_ms: 600,
        },
        cells,
    }
}

pub(crate) fn recorded_house_audio() -> Result<Vec<u8>, String> {
    let samples = [12_i16, -8, 24, -16, 20, -12, 8, -4];
    let payload = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect::<Vec<_>>();
    conduit_audio::PcmFrameHeader::new(
        conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
        16_000,
        conduit_audio::PcmChannelLayout::Mono,
        samples.len() as u16,
        1,
        0,
        false,
    )
    .map_err(|error| format!("build recorded House PCM header: {error:?}"))?
    .encode_frame(&payload)
    .map_err(|error| format!("encode recorded House PCM: {error:?}"))
}

pub(crate) fn recorded_house_audio_clip() -> Result<Vec<u8>, String> {
    let first = recorded_house_audio_block(0, &[12, -8, 24, -16])?;
    let second = recorded_house_audio_block(4, &[20, -12, 8, -4])?;
    conduit_audio::encode_pcm_clip(&[&first, &second])
        .map_err(|error| format!("encode recorded House PCM clip: {error:?}"))
}

fn recorded_house_audio_block(start_frame: u64, samples: &[i16]) -> Result<Vec<u8>, String> {
    let payload = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect::<Vec<_>>();
    conduit_audio::PcmFrameHeader::new(
        conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
        16_000,
        conduit_audio::PcmChannelLayout::Mono,
        samples.len() as u16,
        1,
        start_frame,
        false,
    )
    .map_err(|error| format!("build recorded House PCM clip frame: {error:?}"))?
    .encode_frame(&payload)
    .map_err(|error| format!("encode recorded House PCM clip frame: {error:?}"))
}

fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, PortDirection::Input)?;
    Ok(InstalledOperation::TestLocalModelSink(
        TestLocalModelSinkOperation { complete: false },
    ))
}
