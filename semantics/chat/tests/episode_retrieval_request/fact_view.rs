use super::*;

#[test]
fn fact_view_keeps_the_exact_native_pending_packet_and_allocates_only_at_preparation() {
    let f = fixture();
    let capture = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = capture.capture(f.input()).unwrap();
    let context = context(&snapshot, &f.intent);
    let mut original =
        PreparedEpisodeRetrievalRequest::prepare("Use facts only", empty_current).unwrap();
    original.compose(&snapshot, &context).unwrap();
    let original_data: serde_json::Value = serde_json::from_str(
        decode_generation_context(core::str::from_utf8(original.output()).unwrap())
            .unwrap()
            .unwrap()
            .data(),
    )
    .unwrap();
    let calls = Cell::new(0);
    let mut request = PreparedEpisodeRetrievalRequest::prepare_fact_view(
        "Use facts only",
        |packet: &[u8], output: &mut [u8]| {
            calls.set(calls.get() + 1);
            assert_eq!(packet, f.experience.as_slice());
            empty_current(packet, output)
        },
    )
    .unwrap();
    let (result, allocation) =
        allocation_probe::observe(|| request.compose(&snapshot, &context).map(|_| ()));
    result.unwrap();
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(calls.get(), 1);
    assert!(request.output().len() <= 4096);
    assert_eq!(request.pending(), original.pending());
    let context = decode_generation_context(core::str::from_utf8(request.output()).unwrap())
        .unwrap()
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(context.data()).unwrap();
    assert_eq!(
        context.question().as_deref(),
        original_data["observation"]["content"].as_str()
    );
    assert!(data["provenance"]["observation"].get("content").is_none());
    assert_eq!(data["current_facts"], serde_json::json!([]));
    for (key, value) in original_data.as_object().unwrap() {
        if !["schema", "current_experience", "recalled", "observation"].contains(&key.as_str()) {
            assert_eq!(&data["provenance"][key], value, "lost {key}");
        }
    }
    for (i, episode) in original_data["recalled"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(
            data["remembered_facts"][i]["actual_outcome"],
            episode["actual_outcome"]
        );
        let old = &episode["historical_evidence"]["evidence"][0];
        let mut metadata =
            data["provenance"]["recalled"][i]["historical_evidence"]["evidence"][0].clone();
        let fields = metadata.as_object_mut().unwrap();
        assert_eq!(
            fields.remove("content_ref").unwrap(),
            format!("/remembered_facts/{i}/reported/0")
        );
        assert_eq!(fields.remove("content_encoding").unwrap(), "utf8");
        let mut old_metadata = old.clone();
        old_metadata.as_object_mut().unwrap().remove("utf8_content");
        assert_eq!(metadata, old_metadata);
        assert_eq!(
            data["remembered_facts"][i]["reported"][0],
            old["utf8_content"]
        );
    }
}

#[test]
fn fact_view_overflow_is_atomic_and_the_same_prepared_instance_can_continue() {
    let mut f = fixture();
    f.memory = conduit_chat::initial_episode_memory();
    f.source = source(&f.memory);
    let capture = PreparedEpisodeRetrievalSnapshot::prepare();
    let snapshot = capture.capture(f.input()).unwrap();
    let context = context(&snapshot, &f.intent);
    // Codec-size fence, not a sensory observation or an integration proof.
    let big = serde_json::json!({"evidence":[{"identity":"fenced/size","utf8_content":"x".repeat(2700)}]}).to_string();
    let overflow = Cell::new(true);
    let mut request = PreparedEpisodeRetrievalRequest::prepare_fact_view(
        &"i".repeat(1024),
        |packet: &[u8], output: &mut [u8]| {
            let empty = empty_current(packet, output)?;
            if !overflow.get() {
                return Ok(empty);
            }
            output[..big.len()].copy_from_slice(big.as_bytes());
            Ok(big.len())
        },
    )
    .unwrap();
    let (result, allocation) =
        allocation_probe::observe(|| request.compose(&snapshot, &context).map(|_| ()));
    assert_eq!(result, Err(EpisodeRetrievalRequestRefusal::ByteBound));
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert!(request.output().is_empty());
    assert!(request.pending().is_empty());
    overflow.set(false);
    let (result, allocation) =
        allocation_probe::observe(|| request.compose(&snapshot, &context).map(|_| ()));
    result.unwrap();
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let context = decode_generation_context(core::str::from_utf8(request.output()).unwrap())
        .unwrap()
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(context.data()).unwrap();
    assert_eq!(data["current_facts"], serde_json::json!([]));
    assert!(!request.pending().is_empty());
    assert!(request.output().len() <= 4096);
}
