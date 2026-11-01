//! Verified preparation preserves the complete existing Mask Show protocol.
use super::*;
use conduit_core::{ConfigurationEntry, ConfigurationValue, PlacementId, VerifiedPlanMetadata};
use conduit_face::{MaskJsonRefusal, PreparedMaskShowContext};
use std::collections::BTreeMap;
fn planned(program_bytes: usize) -> PlannedMaskPlot {
    let mut planned = plan_mask("plot browser (\n >> face: Face\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.face\n output.show >> show\n input.interaction >> interaction\n}\n", "browser");
    for (index, gear) in planned.plan.fragments[0].placements.iter_mut().enumerate() {
        gear.configuration = vec![ConfigurationEntry {
            key: "program".into(),
            value: ConfigurationValue::Text(
                format!("{index} é\0\\\"🦀{}", "p".repeat(program_bytes)).into(),
            ),
        }];
    }
    planned.plan = conduit_core::seal_plan_with_realization_backs_and_completion(
        planned.mask.plot_identity.clone(),
        planned.plan.completion_policy,
        planned.plan.realization_backs,
        planned.plan.fragments,
    );
    planned.validate().unwrap();
    planned
}
fn separate(
    planned: &PlannedMaskPlot,
) -> (
    VerifiedPlanMetadata,
    BTreeMap<PlacementId, Vec<ConfigurationEntry>>,
) {
    let mut metadata = planned.plan.clone();
    let entries: BTreeMap<_, _> = metadata
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
        .map(|gear| {
            (
                gear.placement_id.clone(),
                core::mem::take(&mut gear.configuration),
            )
        })
        .collect();
    let verified = conduit_core::verify_plan_metadata_with_configurations(metadata, |gear| {
        Ok::<_, ()>(entries[&gear.placement_id].clone())
    })
    .unwrap();
    (verified, entries)
}
fn face(planned: &PlannedMaskPlot) -> Face {
    Face::new(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some(planned.mask.plot_identity.source_document_id.clone()),
            checked_plot_id: Some(planned.mask.plot_identity.checked_plot_id.clone()),
            expanded_plot_id: Some(planned.mask.plot_identity.expanded_plot_id.clone()),
            plan_id: Some("plan/face-source".into()),
            active_play_id: None,
            sign_ids: vec!["sign/face".into()],
        },
        vec![FaceSubject {
            identity: "mask/plot".into(),
            role: FaceRole::Plot,
            name: "Face source".into(),
        }],
        vec![],
        vec![],
        vec![FaceText {
            subject: "mask/plot".into(),
            text: "Current semantic value".into(),
        }],
    )
    .unwrap()
}
#[test]
fn verified_context_preserves_exact_lossless_wire_show_identity_and_lifecycle() {
    let planned = planned(64);
    let face = face(&planned);
    let (verified, entries) = separate(&planned);
    let rich = PreparedMaskShowContext::prepare(&planned, 64 * 1024).unwrap();
    let streamed = PreparedMaskShowContext::prepare_verified(
        &planned.mask,
        &verified,
        |gear| Ok::<_, ()>(entries[&gear.placement_id].clone()),
        64 * 1024,
    )
    .unwrap();
    assert_eq!(streamed.plan_wire(), rich.plan_wire());
    assert_eq!(
        serde_json::from_slice::<PlannedMaskPlot>(streamed.plan_wire()).unwrap(),
        planned
    );
    let presenter = planned.show_placement();
    let active = bind_active_play(
        &planned.plan.plan_id,
        &presenter.host_id,
        &presenter.boot_id,
        1,
    );
    let rich_show = MaskShow::prepared(
        &planned,
        &face,
        active.clone(),
        "mask/plot".into(),
        "target/display".into(),
        "sign/prepared".into(),
    )
    .unwrap();
    let mut occurrence = streamed
        .prepared_show(
            &face,
            active.clone(),
            "mask/plot".into(),
            "target/display".into(),
            "sign/prepared".into(),
        )
        .unwrap();
    let encode = |occurrence: &conduit_face::PreparedMaskOccurrence<'_>| {
        let encoded = occurrence.json(128 * 1024).unwrap();
        let mut output = vec![0; encoded.encoded_bytes()];
        encoded.encode(&mut output).unwrap();
        output
    };
    assert_eq!(encode(&occurrence), serde_json::to_vec(&rich_show).unwrap());
    occurrence
        .transition(ManifestationLifecycle::Available, "sign/available".into())
        .unwrap();
    let available = rich_show
        .transition(ManifestationLifecycle::Available, "sign/available".into())
        .unwrap();
    assert_eq!(encode(&occurrence), serde_json::to_vec(&available).unwrap());
    occurrence
        .transition(ManifestationLifecycle::Closed, "sign/closed".into())
        .unwrap();
    assert_eq!(
        encode(&occurrence),
        serde_json::to_vec(
            &available
                .transition(ManifestationLifecycle::Closed, "sign/closed".into())
                .unwrap()
        )
        .unwrap()
    );
    let mut failed = streamed
        .prepared_show(
            &face,
            active.clone(),
            "mask/plot".into(),
            "target/display".into(),
            "sign/prepared".into(),
        )
        .unwrap();
    failed
        .fail(
            conduit_face::ManifestationFailure::DeliveryFailed,
            "sign/failed".into(),
        )
        .unwrap();
    assert_eq!(
        encode(&failed),
        serde_json::to_vec(
            &rich_show
                .fail(
                    conduit_face::ManifestationFailure::DeliveryFailed,
                    "sign/failed".into()
                )
                .unwrap()
        )
        .unwrap()
    );
    let mut wrong_active = active;
    wrong_active.boot_id = "foreign/boot".into();
    assert!(streamed
        .prepared_show(
            &face,
            wrong_active,
            "mask/plot".into(),
            "target/display".into(),
            "sign/x".into()
        )
        .is_err());
    let mut stale = face.clone();
    stale.revision += 1;
    stale.reseal_prepared().unwrap();
    assert!(occurrence.validate_face(&stale).is_err());
}
#[test]
fn verified_context_refuses_wrong_role_source_configuration_and_capacity() {
    let planned = planned(32);
    let (verified, entries) = separate(&planned);
    for failure in ["source", "role", "configuration", "unavailable", "capacity"] {
        let mut mask = planned.mask.clone();
        if failure == "source" {
            mask.plot_identity.source_document_id = "foreign/source".into();
        }
        if failure == "role" {
            mask.show_output.gear_port_id = "foreign/show".into();
        }
        let result = PreparedMaskShowContext::prepare_verified(
            &mask,
            &verified,
            |gear| {
                if failure == "unavailable" {
                    return Err(());
                }
                if failure == "configuration" {
                    return Ok(vec![]);
                }
                Ok(entries[&gear.placement_id].clone())
            },
            if failure == "capacity" { 1 } else { 64 * 1024 },
        );
        assert!(result.is_err(), "{failure}");
    }
    let mut supplied = false;
    let result = PreparedMaskShowContext::prepare_verified(
        &planned.mask,
        &verified,
        |_| {
            supplied = true;
            Ok::<_, ()>(vec![])
        },
        0,
    );
    assert!(matches!(result, Err(MaskJsonRefusal::Capacity)));
    assert!(!supplied);
}
#[test]
fn verified_context_avoids_reconstructing_the_whole_rich_mask() {
    let planned = planned(16 * 1024);
    let (verified, entries) = separate(&planned);
    let (rich, rich_memory) = allocation_probe::observe(|| {
        let reconstructed = planned.clone();
        PreparedMaskShowContext::prepare(&reconstructed, 64 * 1024).unwrap()
    });
    let (streamed, streamed_memory) = allocation_probe::observe(|| {
        PreparedMaskShowContext::prepare_verified(
            &planned.mask,
            &verified,
            |gear| Ok::<_, ()>(entries[&gear.placement_id].clone()),
            64 * 1024,
        )
        .unwrap()
    });
    assert_eq!(streamed.plan_wire(), rich.plan_wire());
    assert!(
        streamed_memory.peak_bytes < rich_memory.peak_bytes,
        "streamed={streamed_memory:?}, reconstructed={rich_memory:?}"
    );
    eprintln!(
        "verified_mask_context_peak={} reconstructed_rich_context_peak={} retained_wire={}",
        streamed_memory.peak_bytes,
        rich_memory.peak_bytes,
        streamed.plan_wire().len()
    );
}

#[test]
fn small_verified_context_has_finite_cold_peak_and_rechecks_second_pass_configuration() {
    let planned = planned(32);
    let (verified, entries) = separate(&planned);
    let mut calls = 0;
    let (context, memory) = allocation_probe::observe(|| {
        PreparedMaskShowContext::prepare_verified(
            &planned.mask,
            &verified,
            |gear| {
                calls += 1;
                Ok::<_, ()>(entries[&gear.placement_id].clone())
            },
            8192,
        )
        .unwrap()
    });
    assert_eq!(calls, 2 * entries.len());
    for maximum in [context.plan_wire().len(), context.plan_wire().len() - 1] {
        let bounded = PreparedMaskShowContext::prepare_verified(
            &planned.mask,
            &verified,
            |gear| Ok::<_, ()>(entries[&gear.placement_id].clone()),
            maximum,
        );
        if maximum == context.plan_wire().len() {
            assert_eq!(bounded.unwrap().plan_wire(), context.plan_wire());
        } else {
            assert!(matches!(bounded, Err(MaskJsonRefusal::Capacity)));
        }
    }

    assert_eq!(
        serde_json::from_slice::<PlannedMaskPlot>(context.plan_wire()).unwrap(),
        planned
    );
    assert!(memory.peak_bytes < 40 * 1024, "{memory:?}");
    assert!(PreparedMaskShowContext::codec_workspace_bytes() < 30 * 1024);
    eprintln!(
        "small_verified_context_peak={} workspace_bound={} retained={}",
        memory.peak_bytes,
        PreparedMaskShowContext::codec_workspace_bytes(),
        context.plan_wire().len()
    );
    let mut calls = 0;
    let changed = PreparedMaskShowContext::prepare_verified(
        &planned.mask,
        &verified,
        |gear| {
            calls += 1;
            Ok::<_, ()>(if calls > entries.len() {
                vec![]
            } else {
                entries[&gear.placement_id].clone()
            })
        },
        8192,
    );
    assert!(matches!(changed, Err(MaskJsonRefusal::InvalidPlan)));
    assert!(calls > entries.len());
}

#[test]
fn disjoint_borrowed_scratch_preserves_wire_and_slot_canaries_with_lower_heap_peak() {
    let planned = planned(32);
    let (verified, entries) = separate(&planned);
    let (normal, normal_memory) = allocation_probe::observe(|| {
        PreparedMaskShowContext::prepare_verified(
            &planned.mask,
            &verified,
            |gear| Ok::<_, ()>(entries[&gear.placement_id].clone()),
            8192,
        )
        .unwrap()
    });
    let mut first = vec![0xa5u8; 8192 + 16];
    let mut second = vec![0x5au8; 8192 + 16];
    let (borrowed, memory) = {
        let mut scratch = [&mut first[8..8200], &mut second[8..8200]];
        let result = allocation_probe::observe(|| {
            PreparedMaskShowContext::prepare_verified_with_workspace(
                &planned.mask,
                &verified,
                |gear| Ok::<_, ()>(entries[&gear.placement_id].clone()),
                8192,
                &mut scratch,
            )
            .unwrap()
        });
        assert_eq!((scratch[0].len(), scratch[1].len()), (8192, 8192));
        result
    };
    assert_eq!(borrowed.plan_wire(), normal.plan_wire());
    assert!(first[..8]
        .iter()
        .chain(&first[8200..])
        .all(|&byte| byte == 0xa5));
    assert!(second[..8]
        .iter()
        .chain(&second[8200..])
        .all(|&byte| byte == 0x5a));
    assert!(
        memory.peak_bytes + 15 * 1024 < normal_memory.peak_bytes,
        "borrowed={memory:?}; normal={normal_memory:?}"
    );
    assert!(memory.peak_bytes < 22 * 1024, "{memory:?}");
    eprintln!(
        "borrowed_scratch_context_peak={} normal_peak={} retained={}",
        memory.peak_bytes,
        normal_memory.peak_bytes,
        borrowed.plan_wire().len()
    );
}

#[test]
#[cfg(target_has_atomic = "ptr")]
fn retained_occurrence_requires_the_exact_context_and_outlives_borrowed_inputs() {
    use std::sync::Arc;
    let planned_mask = planned(24);
    let face = face(&planned_mask);
    let context = Arc::new(PreparedMaskShowContext::prepare(&planned_mask, 64 * 1024).unwrap());
    let same_identity =
        Arc::new(PreparedMaskShowContext::prepare(&planned_mask, 64 * 1024).unwrap());
    assert_eq!(context.plan_id(), same_identity.plan_id());
    let different = planned(25);
    let foreign = Arc::new(PreparedMaskShowContext::prepare(&different, 64 * 1024).unwrap());
    let presenter = planned_mask.show_placement();
    let active = bind_active_play(
        &planned_mask.plan.plan_id,
        &presenter.host_id,
        &presenter.boot_id,
        1,
    );
    let mut occurrence = context
        .prepared_show(
            &face,
            active,
            "mask/plot".into(),
            "target/display".into(),
            "sign/prepared".into(),
        )
        .unwrap();
    occurrence
        .transition(ManifestationLifecycle::Available, "sign/available".into())
        .unwrap();
    let encode = |value: &conduit_face::PreparedMaskOccurrence<'_>| {
        let prepared = value.json(128 * 1024).unwrap();
        let mut bytes = vec![0; prepared.encoded_bytes()];
        prepared.encode(&mut bytes).unwrap();
        bytes
    };
    let expected = encode(&occurrence);
    for wrong in [same_identity, foreign] {
        let pointer = Arc::as_ptr(&wrong);
        occurrence = match occurrence.retain_context(wrong) {
            Err((returned, context)) => {
                assert_eq!(Arc::as_ptr(&context), pointer);
                assert_eq!(encode(&returned), expected);
                returned
            }
            Ok(_) => panic!("foreign context accepted"),
        };
    }
    let (retained, memory) =
        allocation_probe::observe(|| occurrence.retain_context(Arc::clone(&context)));
    let mut retained = retained.unwrap_or_else(|_| panic!("exact context refused"));
    assert_eq!(memory.allocations, 0);
    assert_eq!(memory.reallocations, 0);
    drop(face);
    drop(planned_mask);
    drop(context);
    assert_eq!(encode(&retained), expected);
    retained
        .transition(ManifestationLifecycle::Closed, "sign/closed".into())
        .unwrap();
    assert_eq!(retained.show().lifecycle, ManifestationLifecycle::Closed);
}
