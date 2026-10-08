use super::*;
fn request() -> ModelWorkRequest {
    ModelWorkRequest {
        request_identity: [1; 32],
        session_identity: [2; 32],
        expected_generation: 3,
        operation: ModelWorkOperation::Export,
    }
}
#[test]
fn canonical_command_round_trip_and_identity_bounds() {
    let value = request();
    let bytes = value.encode().unwrap();
    assert_eq!(ModelWorkRequest::decode(&bytes).unwrap(), value);
    let mut zero = value;
    zero.session_identity = [0; 32];
    assert_eq!(zero.encode(), Err(ModelWorkCodecRefusal::Identity));
    zero.session_identity = [2; 32];
    zero.operation = ModelWorkOperation::Resume {
        checkpoint_identity: [0; 32],
    };
    assert_eq!(zero.encode(), Err(ModelWorkCodecRefusal::Identity));
}
#[test]
fn oversized_noncanonical_and_unknown_payloads_refuse() {
    assert_eq!(
        ModelWorkRequest::decode(&vec![0; MODEL_WORK_MAXIMUM_INPUT_BYTES as usize + 1]),
        Err(ModelWorkCodecRefusal::Bounds)
    );
    let mut bytes = request().encode().unwrap();
    bytes.push(b' ');
    assert_eq!(
        ModelWorkRequest::decode(&bytes),
        Err(ModelWorkCodecRefusal::Malformed)
    );
    let bytes = request().encode().unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("unreviewed".into(), serde_json::Value::Bool(true));
    assert_eq!(
        ModelWorkRequest::decode(&serde_json::to_vec(&value).unwrap()),
        Err(ModelWorkCodecRefusal::Malformed)
    );
}
#[test]
fn full_state_reply_round_trips() {
    let value = ModelWorkReply {
        request_identity: [1; 32],
        session_identity: [2; 32],
        generation: 4,
        result: ModelWorkResult::Resumed(TrainingState {
            session_identity: [2; 32],
            model: crate::MutableModelState {
                base_artifact_identity: [3; 32],
                state_identity: "model/test".into(),
                state_schema_version: 1,
                generation: 4,
            },
            initial_generation: 0,
            completed_steps: 4,
            consumed_work_units: 400,
        }),
    };
    assert_eq!(
        ModelWorkReply::decode(&value.encode().unwrap()).unwrap(),
        value
    );
}
#[test]
fn maximal_mutating_receipt_fits_prepared_wire_bound() {
    let text = "\0".repeat(128);
    let metric = TrainingMetric::new(
        crate::TrainingObjectiveIdentity::new("~".repeat(128)).unwrap(),
        i64::MIN,
    )
    .unwrap();
    let realization = crate::HostTrainingRealization {
        implementation_identity: text.clone(),
        runtime_name: text.clone(),
        runtime_version: text.clone(),
        runtime_build_identity: text.clone(),
        device_profile: text.clone(),
        format_profile: text.clone(),
        precision_profile: text.clone(),
        deterministic_profile: text.clone(),
    };
    let checkpoint = ModelCheckpoint {
        base_artifact_identity: [255; 32],
        architecture_profile: text.clone(),
        state_schema_version: u32::MAX,
        generation: u64::MAX,
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([255; 32]),
            content_profile: kind_id(&"~".repeat(128)),
            access_class: ResourceClassId::from("~".repeat(128)),
            extent: ResourceExtent {
                bytes: MAXIMUM_REFERENCED_BYTES,
                items: Some(u64::MAX),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([255; 32]),
                expires_at: None,
            },
        },
    };
    let reply = ModelWorkReply {
        request_identity: [255; 32],
        session_identity: [255; 32],
        generation: u64::MAX,
        result: ModelWorkResult::Checkpointed(Box::new(TrainingCheckpointReceipt {
            session_identity: [255; 32],
            session_descriptor_identity: [255; 32],
            base_artifact_identity: [255; 32],
            dataset_manifest_identity: [255; 32],
            split_membership_identity: [255; 32],
            objective_profile: text.clone(),
            randomness: crate::RandomnessProfile::provider_chosen(text, u64::MAX).unwrap(),
            realization,
            completed_steps: u64::MAX,
            consumed_work_units: u64::MAX,
            metric_summaries: vec![metric; crate::MAXIMUM_METRICS],
            checkpoint,
        })),
    };
    let bytes = reply.encode().unwrap();
    let bound = model_work_mutating_reply_bytes_bound().unwrap();
    assert!(bytes.len() <= bound);
    assert!(bound <= MODEL_WORK_MAXIMUM_OUTPUT_BYTES as usize);
    assert_eq!(ModelWorkReply::decode(&bytes).unwrap(), reply);
}

#[cfg(feature = "plot-catalog")]
#[test]
fn ordinary_plot_checks_expands_and_plans_from_portable_contract() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_model_work_catalog(&mut startup, &mut profile).unwrap();
    let syntax = conduit_plot::parse_syntax_document("plot learning {\n work: model/work\n}\n");
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_plot::expand_canonical_plot(&checked, "learning", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("test/host"),
        boot_id: BootId::from("test/boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/host@1"),
        bases: vec![],
        resources: vec![resource_offer("test/model", MODEL_WORK_RESOURCE_CLASS, 1)],
        capabilities: vec![model_work_offer("test").unwrap()],
        planner_capabilities: vec![],
    };
    let hosts = [host];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    assert_eq!(
        plan.fragments[0].placements[0].host_calls[0]
            .contract_id
            .as_str(),
        MODEL_WORK_OPERATION
    );
}
