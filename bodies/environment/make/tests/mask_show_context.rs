//! Compact Show construction preserves the existing complete representation.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/mask.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_face as f;
#[test]
fn compact_context_drops_the_rich_plan_and_preserves_show_states_and_identity() {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let host = fixture::host();
    let planned = source.plan(&host, 8192, &[]).unwrap();
    let mut face = fixture::face();
    face.basis.body_id = Some(serde_json::from_value(serde_json::json!("b".repeat(64))).unwrap());
    face.basis.wake_id = Some(serde_json::from_value(serde_json::json!("c".repeat(64))).unwrap());
    face.reseal_prepared().unwrap();
    let active = c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1);
    let target = "fixture/\"display\n☃";
    let rich = f::MaskShow::prepared(
        &planned,
        &face,
        active.clone(),
        "environment".into(),
        target.into(),
        "fixture/prepared".into(),
    )
    .unwrap();
    let available = rich
        .transition(
            f::ManifestationLifecycle::Available,
            "fixture/available".into(),
        )
        .unwrap();
    let closed = available
        .transition(f::ManifestationLifecycle::Closed, "fixture/closed".into())
        .unwrap();
    let failed = rich
        .fail(
            f::ManifestationFailure::DeliveryFailed,
            "fixture/failed".into(),
        )
        .unwrap();
    let expected: Vec<_> = [&rich, &available, &closed, &failed]
        .into_iter()
        .map(|show| serde_json::to_vec(show).unwrap())
        .collect();
    let (context, context_memory) =
        allocation_probe::observe(|| f::PreparedMaskShowContext::prepare(&planned, 8192).unwrap());
    assert_eq!(context.plan_wire(), serde_json::to_vec(&planned).unwrap());
    assert_eq!(context.plan_id(), &planned.plan.plan_id);
    drop(rich);
    drop(available);
    drop(closed);
    drop(failed);
    drop(planned);
    drop(source);
    let (mut occurrence, show_memory) = allocation_probe::observe(|| {
        context
            .prepared_show(
                &face,
                active.clone(),
                "environment".into(),
                target.into(),
                "fixture/prepared".into(),
            )
            .unwrap()
    });
    assert!(show_memory.retained_bytes() < 4096);
    occurrence.validate_face(&face).unwrap();
    let mut changed = face.clone();
    changed.revision += 1;
    changed.reseal_prepared().unwrap();
    assert_eq!(
        occurrence.validate_face(&changed),
        Err(f::MaskShowError::StaleFace)
    );
    // Codec states only: a transition is not an effect/delivery admission.
    for (index, expected_wire) in expected.iter().take(3).enumerate() {
        match index {
            1 => occurrence
                .transition(
                    f::ManifestationLifecycle::Available,
                    "fixture/available".into(),
                )
                .unwrap(),
            2 => occurrence
                .transition(f::ManifestationLifecycle::Closed, "fixture/closed".into())
                .unwrap(),
            _ => (),
        }
        let encoder = occurrence.json(8192).unwrap();
        let mut bytes = vec![0; encoder.encoded_bytes()];
        encoder.encode(&mut bytes).unwrap();
        assert_eq!(&bytes, expected_wire);
        let mut direct = vec![0; 8192];
        let (_, direct_memory) = allocation_probe::observe(|| {
            for _ in 0..4 {
                let length = occurrence
                    .encode_json_with(&mut direct, serialize_part)
                    .unwrap();
                assert_eq!(&direct[..length], &expected_wire[..]);
            }
        });
        assert_eq!(
            (direct_memory.allocations, direct_memory.reallocations),
            (0, 0)
        );
        let (_, core_memory) = allocation_probe::observe(|| {
            for _ in 0..4 {
                let length = occurrence
                    .encode_json_with(&mut direct, |part, out| {
                        serde_json_core::to_slice(&part, out).map_err(|_| f::MaskJsonRefusal::Json)
                    })
                    .unwrap();
                assert_eq!(&direct[..length], &expected_wire[..]);
            }
        });
        assert_eq!((core_memory.allocations, core_memory.reallocations), (0, 0));
        // Refusal may leave private bytes, but never yields a publishable length.
        assert_eq!(
            occurrence.encode_json_with(&mut direct[..bytes.len() - 1], serialize_part),
            Err(f::MaskJsonRefusal::Capacity)
        );
        assert_eq!(
            occurrence.encode_json_with(&mut direct, |_, out| Ok(out.len() + 1)),
            Err(f::MaskJsonRefusal::Json)
        );
        let decoded: f::MaskShow = serde_json::from_slice(&bytes).unwrap();
        decoded.validate(&face).unwrap();
    }
    assert!(occurrence
        .transition(f::ManifestationLifecycle::Available, "fixture/extra".into())
        .is_err());
    let mut failing = context
        .prepared_show(
            &face,
            active,
            "environment".into(),
            target.into(),
            "fixture/prepared".into(),
        )
        .unwrap();
    failing
        .fail(
            f::ManifestationFailure::DeliveryFailed,
            "fixture/failed".into(),
        )
        .unwrap();
    let encoder = failing.json(8192).unwrap();
    drop(failing);
    drop(occurrence);
    drop(face);
    // The encoder owns all changing metadata; only immutable context remains.
    let mut output = vec![0; encoder.encoded_bytes()];
    let (_, encoding) = allocation_probe::observe(|| {
        for _ in 0..960 {
            encoder.encode(&mut output).unwrap();
        }
    });
    assert_eq!((encoding.allocations, encoding.reallocations), (0, 0));
    assert_eq!(output, expected[3]);
    println!(
        "Compact Show context retained={} peak={}; occurrence retained={} peak={}; encode peak={}",
        context_memory.retained_bytes(),
        context_memory.peak_bytes,
        show_memory.retained_bytes(),
        show_memory.peak_bytes,
        encoding.peak_bytes
    );
}
#[test]
fn compact_context_refuses_forged_plan_play_and_unknown_subject() {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let host = fixture::host();
    let mut planned = source.plan(&host, 8192, &[]).unwrap();
    let context = f::PreparedMaskShowContext::prepare(&planned, 8192).unwrap();
    let face = fixture::face();
    let mut active = c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1);
    active.boot_id = "wrong".into();
    assert!(matches!(
        context.prepared_show(
            &face,
            active,
            "environment".into(),
            "fixture/display".into(),
            "fixture/prepared".into()
        ),
        Err(f::MaskShowError::InvalidManifestation(
            f::ManifestationError::StaleIdentity
        ))
    ));
    let active = c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1);
    assert!(matches!(
        context.prepared_show(
            &face,
            active,
            "unknown".into(),
            "fixture/display".into(),
            "fixture/prepared".into()
        ),
        Err(f::MaskShowError::InvalidManifestation(
            f::ManifestationError::UnknownFaceSubject
        ))
    ));
    planned.plan.fragments[0].placements[0].implementation_id = "forged".into();
    assert!(matches!(
        f::PreparedMaskShowContext::prepare(&planned, 8192),
        Err(f::MaskJsonRefusal::InvalidPlan)
    ));
}

#[test]
fn received_wire_is_bounded_and_fully_validated_without_recompression() {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let host = fixture::host();
    let planned = source.plan(&host, 8192, &[]).unwrap();
    let face = fixture::face();
    let active = c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1);
    let rich = f::MaskShow::prepared(
        &planned,
        &face,
        active.clone(),
        "environment".into(),
        "fixture/display".into(),
        "fixture/prepared".into(),
    )
    .unwrap();
    let expected = serde_json::to_vec(&rich).unwrap();
    let wire = serde_json::to_vec(&planned).unwrap();
    let (context, memory) = allocation_probe::observe(|| {
        f::PreparedMaskShowContext::from_wire(wire.clone().into_boxed_slice(), 8192, 32768).unwrap()
    });
    assert_eq!(context.plan_wire(), wire);
    assert!(memory.peak_bytes < 64 * 1024);
    drop(planned);
    drop(rich);
    drop(source);
    let occurrence = context
        .prepared_show(
            &face,
            active,
            "environment".into(),
            "fixture/display".into(),
            "fixture/prepared".into(),
        )
        .unwrap();
    let encoder = occurrence.json(8192).unwrap();
    drop(occurrence);
    let mut bytes = vec![0; encoder.encoded_bytes()];
    encoder.encode(&mut bytes).unwrap();
    assert_eq!(bytes, expected);
    let decoded: f::MaskShow = serde_json::from_slice(&bytes).unwrap();
    decoded.validate(&face).unwrap();
    let mut malformed: serde_json::Value = serde_json::from_slice(&wire).unwrap();
    malformed["plan"]["expanded_bytes"] = 32769.into();
    malformed["plan"]["zlib_base64"] = "not base64".into();
    let oversized = serde_json::to_vec(&malformed).unwrap();
    let (result, refusal_memory) = allocation_probe::observe(|| {
        f::PreparedMaskShowContext::from_wire(oversized.clone().into_boxed_slice(), 8192, 32768)
    });
    assert!(matches!(result, Err(f::MaskJsonRefusal::Capacity)));
    assert!(refusal_memory.peak_bytes < oversized.len() + 2048);
    let mut malformed: serde_json::Value = serde_json::from_slice(&wire).unwrap();
    malformed["mask"]["show_output"]["gear_id"] = "missing".into();
    assert!(matches!(
        f::PreparedMaskShowContext::from_wire(
            serde_json::to_vec(&malformed).unwrap().into_boxed_slice(),
            8192,
            32768
        ),
        Err(f::MaskJsonRefusal::InvalidPlan)
    ));
    assert!(matches!(
        f::PreparedMaskShowContext::from_wire(wire.into_boxed_slice(), 1, 32768),
        Err(f::MaskJsonRefusal::Capacity)
    ));
    println!(
        "Received Show context retained={} preparation peak={} oversized refusal peak={}",
        memory.retained_bytes(),
        memory.peak_bytes,
        refusal_memory.peak_bytes
    );
}

fn serialize_part(
    part: f::MaskJsonPart<'_>,
    output: &mut [u8],
) -> Result<usize, f::MaskJsonRefusal> {
    let total = output.len();
    let mut writer = output;
    serde_json::to_writer(&mut writer, &part).map_err(|_| f::MaskJsonRefusal::Capacity)?;
    Ok(total - writer.len())
}

#[path = "mask_show_context/verified_native.rs"]
mod verified_native;
