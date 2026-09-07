use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindContractRevision, PlannedGear, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_kernel::{OperationAction, OperationInput, PortId, ValueRef, ValueStorage};

const SOURCE_KIND: &str = "conduit-test/local-model-request";
const SOURCE_REVISION: &str = "conduit-test/local-model-request@1";
pub(crate) const HOUSE_RECOGNIZED_SOURCE_KIND: &str = "conduit-test/house-recognized-source";
const HOUSE_RECOGNIZED_SOURCE_REVISION: &str = "conduit-test/house-recognized-source@1";
pub(crate) const HOUSE_ADDRESSES_SOURCE_KIND: &str = "conduit-test/house-addresses-source";
const HOUSE_ADDRESSES_SOURCE_REVISION: &str = "conduit-test/house-addresses-source@1";
pub(crate) const HOUSE_CONTEXT_SOURCE_KIND: &str = "conduit-test/house-context-source";
const HOUSE_CONTEXT_SOURCE_REVISION: &str = "conduit-test/house-context-source@1";
const SOURCE_IMPLEMENTATION: &str = "conduit-test/local-model-request-kernel@1";
const SINK_KIND: &str = "conduit-test/local-model-result";
const SINK_REVISION: &str = "conduit-test/local-model-result@1";
pub(crate) const HOUSE_TEXT_SINK_KIND: &str = "conduit-test/house-text-sink";
const HOUSE_TEXT_SINK_REVISION: &str = "conduit-test/house-text-sink@1";
const SINK_IMPLEMENTATION: &str = "conduit-test/local-model-result-kernel@1";
const PROFILE: &str = "conduit-test/local-model-io-kernel@1";
const ARTIFACT: &str = "conduit-std-host/test-local-model-io@1";

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
}

pub(super) struct TestLocalModelSinkOperation {
    complete: bool,
}

impl TestLocalModelSourceOperation {
    pub(super) fn emit_or_complete(&self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
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

pub(crate) fn house_source_offers() -> [CapabilityOffer; 3] {
    [
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

pub(crate) fn sink_offer(value_kind: &str) -> CapabilityOffer {
    offer(
        SINK_KIND,
        SINK_REVISION,
        SINK_IMPLEMENTATION,
        value_kind,
        PortDirection::Input,
    )
}

pub(crate) fn house_text_sink_offer() -> CapabilityOffer {
    offer(
        HOUSE_TEXT_SINK_KIND,
        HOUSE_TEXT_SINK_REVISION,
        SINK_IMPLEMENTATION,
        conduit_ai::TEXT_VALUE_KIND,
        PortDirection::Input,
    )
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
        kind_contract_revision: KindContractRevision::from(revision),
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

pub(crate) fn install_house_source_catalog(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
) {
    for offer in house_source_offers() {
        install_offer(startup, catalog, offer);
    }
}

pub(crate) fn install_house_text_sink_catalog(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
) {
    install_offer(startup, catalog, house_text_sink_offer());
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
        .insert(KindDefinition {
            kind_id: offer.kind_id,
            kind_contract_revision: offer.kind_contract_revision,
            inputs: offer.inputs,
            outputs: offer.outputs,
            configuration: Vec::new(),
        })
        .expect("test local-model IO Kind is unique");
}

fn validate(placement: &PlannedGear, direction: PortDirection) -> Result<(), String> {
    let (kind, revision, implementation) = if direction == PortDirection::Output {
        match placement.kind_id.as_str() {
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
            _ => (SOURCE_KIND, SOURCE_REVISION, SOURCE_IMPLEMENTATION),
        }
    } else if placement.kind_id.as_str() == HOUSE_TEXT_SINK_KIND {
        (
            HOUSE_TEXT_SINK_KIND,
            HOUSE_TEXT_SINK_REVISION,
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
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: 4_096,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: 4_096,
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
    let request = if placement.kind_id.as_str() == HOUSE_RECOGNIZED_SOURCE_KIND {
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
            canonical_value: b"21 C, observed 18 seconds ago".to_vec(),
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
    } else {
        b"Conduit bounded local model request".to_vec()
    };
    let value = values
        .store(&request)
        .map_err(|error| format!("store local-model test request: {error:?}"))?;
    Ok(InstalledOperation::TestLocalModelSource(
        TestLocalModelSourceOperation {
            value,
            emitted: false,
        },
    ))
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
