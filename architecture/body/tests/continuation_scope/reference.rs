//! Canonical six-Plot CPU reference; no IO provider or global Playing assertion.
use conduit_body::*;
use conduit_core as c;
use conduit_embedded_build as e;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
#[path = "admission.rs"]
mod admission;
#[path = "../../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "candidate_storage.rs"]
mod candidate_storage;
#[path = "native_operations.rs"]
mod native_operations;
#[path = "../prepared_fragment/support.rs"]
mod support;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn maxima() -> c::AssignedPlanMaxima {
    c::AssignedPlanMaxima {
        encoded_bytes: 4096,
        runtime_state_bytes: 4096,
        counts: [255; 13],
    }
}
fn limits() -> e::BodyFragmentProjectionLimits {
    e::BodyFragmentProjectionLimits {
        image: e::EmbeddedImageBounds::HOST_TOOLING,
        computations: e::EmbeddedComputationBounds {
            maximum_program_bytes: 1024,
            maximum_total_program_bytes: 1024,
        },
        assigned: maxima(),
        maximum_fore_ports: 0,
        maximum_document_bytes: 8192,
    }
}
fn seal(plan: &c::Plan, boot: &str) -> c::Plan {
    let mut f = plan.fragments[0].clone();
    f.boot_id = boot.into();
    f.offer_generation = c::OfferGeneration(9);
    for p in &mut f.placements {
        p.boot_id = boot.into();
        p.offer_generation = c::OfferGeneration(9);
        native_operations::refresh(p, boot);
        for resource in &mut p.resources {
            if let Some(content) = &mut resource.content {
                content.owner_boot = boot.into();
            }
        }
    }
    c::seal_plan(
        c::PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        vec![f],
    )
}
fn bindings<'a>(
    recipe: &c::FragmentSealRecipe,
    f: &'a c::PlanFragment,
) -> Vec<c::FragmentSealBinding<'a>> {
    use c::{FragmentScopeRole as R, FragmentSealValue as V};
    let mut result = Vec::new();
    for piece in recipe.pieces() {
        let c::FragmentSealPiece::Field(field, _) = piece else {
            continue;
        };
        if result
            .iter()
            .any(|b: &c::FragmentSealBinding<'_>| b.field == field)
        {
            continue;
        }
        let value = if let Some(index) = field.placement {
            let p = &f.placements[usize::from(index)];
            match field.role {
                R::Host => V::Text(p.host_id.as_str()),
                R::Boot => V::Text(p.boot_id.as_str()),
                R::OfferGeneration => V::U64(p.offer_generation.0),
                R::PlacementId => V::Text(p.placement_id.as_str()),
                R::ResourcePool => V::Text(p.resources[usize::from(field.entry)].pool_id.as_str()),
                R::ContentOwnerHost => V::Text(
                    p.resources[usize::from(field.entry)]
                        .content
                        .as_ref()
                        .unwrap()
                        .owner_host
                        .as_str(),
                ),
                R::ContentOwnerBoot => V::Text(
                    p.resources[usize::from(field.entry)]
                        .content
                        .as_ref()
                        .unwrap()
                        .owner_boot
                        .as_str(),
                ),
                R::ProviderInstance => {
                    V::Text(p.base.as_ref().unwrap().provider_instance_id.as_str())
                }
                R::ProviderGeneration => V::U64(p.base.as_ref().unwrap().provider_generation),
                R::AuthorityGrant => {
                    V::Text(p.authority[usize::from(field.entry)].grant_id.as_str())
                }
                R::AuthorityHost => V::Text(p.authority[usize::from(field.entry)].host_id.as_str()),
                R::AuthorityBoot => V::Text(p.authority[usize::from(field.entry)].boot_id.as_str()),
                _ => panic!("fixture has no other dynamic fields"),
            }
        } else {
            match field.role {
                R::Host => V::Text(f.host_id.as_str()),
                R::Boot => V::Text(f.boot_id.as_str()),
                R::OfferGeneration => V::U64(f.offer_generation.0),
                _ => panic!("header"),
            }
        };
        result.push(c::FragmentSealBinding { field, value });
    }
    result
}
pub fn with_typed_reference(
    check: impl for<'a> FnOnce(
        &FreshContinuationScope<'a>,
        &VerifiedContinuationIntent<'a>,
        &c::HostAdvertisement,
    ),
) {
    with_reference_kind(true, 0, |s, v, a, _, _, _| check(s, v, a));
}
fn content_contract() -> c::ResourceContentRequirement {
    c::ResourceContentRequirement {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        version: c::ResourceVersionIdentity::from_digest([2; 32]),
        content_profile: "recording/canonical@1".into(),
        maximum_bytes: 4096,
        maximum_items: 64,
        retention: c::ResourceRetention::ExternalDurable,
        sharing: c::ResourceSharing::SingleWriterPublished,
        access: c::ResourceAccessMode::WriteCandidatePublish,
        generation_slots: 1,
        reader_leases: 1,
        publication_slots: 1,
        sensitive: false,
    }
}
pub fn with_full_reference(
    check: impl for<'a> FnOnce(
        &FreshContinuationScope<'a>,
        &VerifiedContinuationIntent<'a>,
        &c::HostAdvertisement,
        &ContinuationFullScope<'_>,
        &[ContinuationBundleEntry<'_>],
        &[u8],
    ),
) {
    with_reference_kind(false, 0, check);
}
pub fn with_native_reference(operations: usize) {
    with_reference_kind(false, operations, |scope, _, _, _, _, _| {
        assert_eq!(scope.fragment_count(), 3)
    });
}
fn with_reference_kind(
    content: bool,
    operations: usize,
    check: impl for<'a> FnOnce(
        &FreshContinuationScope<'a>,
        &VerifiedContinuationIntent<'a>,
        &c::HostAdvertisement,
        &ContinuationFullScope<'_>,
        &[ContinuationBundleEntry<'_>],
        &[u8],
    ),
) {
    let partitions: Vec<_> = ["a", "b", "c", "d", "e", "f"]
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let mut plan = support::plan(label, if i < 3 { "host/local" } else { "host/remote" });
            if operations != 0 && i == 0 {
                native_operations::configure(&mut plan, operations);
            }
            if content && i == 0 {
                let f = &mut plan.fragments[0];
                f.placements[0].resources[0].pool_id = "host/local/recording-pool".into();
                f.placements[0].resources[0].content = Some(c::ResourceContentOffer {
                    contract: content_contract(),
                    owner_host: f.host_id.clone(),
                    owner_boot: f.boot_id.clone(),
                    base_id: "flash/base".into(),
                    residence_profile: "flash/recording@1".into(),
                });
                plan = c::seal_plan(
                    c::PlotIdentity {
                        source_document_id: plan.source_document_id.clone(),
                        checked_plot_id: plan.checked_plot_id.clone(),
                        expanded_plot_id: plan.expanded_plot_id.clone(),
                    },
                    plan.fragments,
                );
            }
            BodyPlotPlan {
                plot: ResidentPlot::new(
                    plan.source_document_id.clone(),
                    plan.checked_plot_id.clone(),
                ),
                plan,
            }
        })
        .collect();
    let body = Body::born_with_plots(
        BodyWorkset::from_plots(partitions.iter().map(|p| p.plot.clone())).unwrap(),
        1,
        "born".into(),
    )
    .unwrap();
    let (_, wake) = body.wake(91, "woke".into()).unwrap();
    let base = BodyPlan::seal(&wake, partitions).unwrap();
    let current = BodyPlan::seal(
        &wake,
        base.plots
            .iter()
            .enumerate()
            .map(|(i, p)| BodyPlotPlan {
                plot: p.plot.clone(),
                plan: if i < 3 {
                    seal(&p.plan, "local/fresh")
                } else {
                    p.plan.clone()
                },
            })
            .collect(),
    )
    .unwrap();
    let play = BodyPlayIdentity::bind(&current, 1);
    assert_ne!(base.plan_id, current.plan_id);
    let fullpins: Vec<_> = base
        .plots
        .iter()
        .enumerate()
        .map(|(i, p)| ContinuationPlotPin {
            source_document: p.plot.source_document_id.as_str(),
            checked_plot: p.plot.checked_plot_id.as_str(),
            plan_id: p.plan.plan_id.as_str(),
            local: i < 3,
        })
        .collect();
    let full = ContinuationFullScope {
        wake_id: wake.wake_id.as_str(),
        wake_sequence: 91,
        workload_revision: wake.workload_revision,
        base_body_plan: base.plan_id.as_str(),
        plots: &fullpins,
    };
    let recipes: Vec<_> = base.plots[..3]
        .iter()
        .map(|p| {
            let f = &p.plan.fragments[0];
            c::FragmentSealRecipe::capture(f, c::FragmentSealRecipe::required_limits(f).unwrap())
                .unwrap()
        })
        .collect();
    let pieces: Vec<Vec<_>> = recipes.iter().map(|r| r.pieces().collect()).collect();
    let templates: Vec<_> = base.plots[..3]
        .iter()
        .map(|p| {
            e::generate_fore_native_plan(
                &p.plan,
                &p.plan.fragments[0].fragment_id,
                limits().image,
                limits().computations,
                0,
            )
            .unwrap()
            .encode(maxima())
            .unwrap()
        })
        .collect();
    let placements: Vec<Vec<_>> = base.plots[..3]
        .iter()
        .map(|p| {
            p.plan.fragments[0]
                .placements
                .iter()
                .map(|p| p.placement_id.as_str())
                .collect()
        })
        .collect();
    let pins: Vec<_> = base.plots[..3]
        .iter()
        .enumerate()
        .map(|(i, p)| ContinuationBundleEntry {
            name: p.plan.expanded_plot_id.as_str(),
            source_document: p.plot.source_document_id.as_str(),
            checked_plot: p.plot.checked_plot_id.as_str(),
            expanded_plot: p.plan.expanded_plot_id.as_str(),
            projection_digest: c::assigned_local_projection_template_digest(
                &templates[i],
                maxima(),
            )
            .unwrap(),
            recipe_digest: c::fragment_seal_recipe_identity(pieces[i].iter().copied()),
            placement_ids: &placements[i],
            placement_nodes: &[0],
        })
        .collect();
    let documents: Vec<_> = current.plots[..3]
        .iter()
        .map(|p| {
            let scope = PreparedBodyFragment::prepare(&wake, &current, &play, &p.plan.fragments[0])
                .unwrap();
            e::generate_prepared_body_fragment(scope, &[], limits()).unwrap()
        })
        .collect();
    let docs: Vec<_> = documents.iter().map(|d| d.bytes()).collect();
    let substitutions: Vec<_> = current.plots[..3]
        .iter()
        .enumerate()
        .map(|(i, p)| bindings(&recipes[i], &p.plan.fragments[0]))
        .collect();
    let mut actual = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/local".into(),
        boot_id: "local/fresh".into(),
        offer_generation: c::OfferGeneration(9),
        profile: "config".into(),
        bases: vec![],
        resources: vec![c::ResourceOffer {
            pool_id: "host/local/timer-pool".into(),
            class_id: "test/timer".into(),
            capacity_units: 3,
            compute: None,
            content: None,
        }],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    if content {
        actual.resources.push(c::ResourceOffer {
            pool_id: "host/local/recording-pool".into(),
            class_id: "test/timer".into(),
            capacity_units: 1,
            compute: None,
            content: Some(c::ResourceContentOffer {
                contract: content_contract(),
                owner_host: "host/local".into(),
                owner_boot: "local/fresh".into(),
                base_id: "flash/base".into(),
                residence_profile: "flash/recording@1".into(),
            }),
        });
    }
    let native_rows: Vec<_> = docs.iter().map(|d| native_operations::rows(d)).collect();
    let native_calls: Vec<Vec<_>> = native_rows
        .iter()
        .map(|rows| rows.iter().map(|n| n.contract).collect())
        .collect();
    let entries: Vec<_> = (0..3)
        .map(|i| ContinuationLocalEntry {
            recipe: &pieces[i],
            bindings: &substitutions[i],
            template: &templates[i],
            native: &native_rows[i],
            fore: &[],
            profile: AssignedBodyPreparationProfile {
                maxima: maxima(),
                requirements: c::AssignedPlanRequirements {
                    host: c::AssignedIdentity::from_text("host/local"),
                    boot: c::AssignedIdentity::from_text("local/fresh"),
                    host_calls: &native_calls[i],
                    resources: &[0],
                    remote_bindings: &[],
                },
                computations: c::AssignedComputationProfile {
                    maximum_text_bytes: 0,
                    maximum_total_text_bytes: 0,
                    requirements: &[],
                },
                fore: None,
            },
        })
        .collect();
    let source = b"CPU canonical six-Plot fixture";
    let source_digest = Sha256::digest(source).into();
    let key = SigningKey::from_bytes(&[7; 32]);
    let ceilings = ContinuationCeilings {
        assignment_bytes: 24576,
        runtime_bytes: 24576,
        durable_bytes: 4096,
        operation_slots: 8,
        active_worksets: 1,
    };
    let intent = ContinuationIntent {
        schema: CONTINUATION_INTENT_SCHEMA,
        body_id: body.body_id.as_str(),
        part_id: "part",
        host_id: "host/local",
        credential_id: "credential",
        membership_generation: 1,
        intent_generation: 1,
        issuer_host: "owner",
        issuer_key: key.verifying_key().to_bytes(),
        source_digest,
        workset_digest: continuation_bundle_digest(source_digest, &pins).unwrap(),
        authority: ContinuationAuthority::LocalWorksetReplacement,
        scope_digest: continuation_scope_digest(body.body_id.as_str(), &full).unwrap(),
        image_id: "image",
        configuration_id: "config",
        ceilings,
    };
    let sig = key.sign(&intent.signing_transcript()).to_bytes();
    let verified = verify_continuation_intent(
        intent,
        &sig,
        Some(ContinuationPolicy {
            issuer_host: "owner",
            issuer_key: key.verifying_key().to_bytes(),
            maximum: ceilings,
        }),
        TrustedInstalledContinuationFacts {
            body_id: body.body_id.as_str(),
            part_id: "part",
            host_id: "host/local",
            credential_id: "credential",
            membership_generation: 1,
            intent_generation: 1,
            source_digest,
            workset_digest: intent.workset_digest,
            image_id: "image",
            configuration_id: "config",
            actual_boot: actual.boot_id.as_str(),
        },
    )
    .unwrap();
    let checked = check_continuation_bundle(&verified, source, &pins).unwrap();
    let mut scratch = vec![0; 8192];
    let (accepted, cost) = allocation_probe::observe(|| {
        FreshContinuationScope::check_local_replacement::<0>(
            &verified,
            &checked,
            &full,
            &actual,
            &pins,
            &entries,
            &docs,
            &mut scratch,
        )
        .unwrap()
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    if operations != 0 {
        native_operations::refusals(&native_rows[0], &pieces[0], |rows| {
            let mut changed: Vec<_> = entries
                .iter()
                .map(|e| ContinuationLocalEntry {
                    recipe: e.recipe,
                    bindings: e.bindings,
                    template: e.template,
                    native: e.native,
                    fore: e.fore,
                    profile: AssignedBodyPreparationProfile {
                        maxima: e.profile.maxima,
                        requirements: e.profile.requirements,
                        computations: e.profile.computations,
                        fore: e.profile.fore,
                    },
                })
                .collect();
            changed[0].native = rows;
            FreshContinuationScope::check_local_replacement::<0>(
                &verified,
                &checked,
                &full,
                &actual,
                &pins,
                &changed,
                &docs,
                &mut scratch,
            )
            .is_err()
        });
    }

    candidate_storage::check(
        &verified,
        &checked,
        &full,
        &actual,
        &pins,
        &entries,
        &docs,
        &mut scratch,
        ceilings.runtime_bytes as usize,
    );
    // Actual bounded assembler must reproduce the full rich canonical encoder,
    // not merely mint IDs which happen to satisfy its own reader.
    if content {
        let rows: Vec<Vec<_>> = docs
            .iter()
            .map(|document| {
                let (metadata, _) = decode_body_fragment_document(document).unwrap();
                FragmentResources::new(metadata.resource_bindings)
                    .map(Result::unwrap)
                    .collect()
            })
            .collect();
        let resources: Vec<_> = rows.iter().map(Vec::as_slice).collect();
        let candidate = build_continuation_candidate_with_resources::<0>(
            &verified,
            &checked,
            &full,
            &actual,
            &pins,
            &entries,
            &resources,
            ceilings.runtime_bytes as usize,
        )
        .unwrap();
        assert_eq!(candidate.len(), docs.len());
        for (i, original) in docs.iter().enumerate() {
            assert_eq!(
                candidate.document(i),
                Some(*original),
                "exact canonical fresh document"
            );
        }
        let (refused, pressure) = allocation_probe::observe(|| {
            build_continuation_candidate_with_resources::<0>(
                &verified, &checked, &full, &actual, &pins, &entries, &resources, 1,
            )
        });
        assert!(matches!(refused, Err(ContinuationScopeRefusal::Bounds)));
        assert_eq!(
            pressure.allocations, 0,
            "workspace shortage refuses before any reserve"
        );
        let mut foreign_boot = actual.clone();
        foreign_boot.boot_id = "other/fresh".into();
        assert!(build_continuation_candidate_with_resources::<0>(
            &verified,
            &checked,
            &full,
            &foreign_boot,
            &pins,
            &entries,
            &resources,
            ceilings.runtime_bytes as usize,
        )
        .is_err());
    } else {
        let rows: Vec<Vec<_>> = docs
            .iter()
            .map(|document| {
                let (metadata, _) = decode_body_fragment_document(document).unwrap();
                PlainFragmentResources::new(metadata.resource_bindings)
                    .map(Result::unwrap)
                    .collect()
            })
            .collect();
        let resources: Vec<_> = rows.iter().map(Vec::as_slice).collect();
        let candidate = build_continuation_candidate::<0>(
            &verified,
            &checked,
            &full,
            &actual,
            &pins,
            &entries,
            &resources,
            ceilings.runtime_bytes as usize,
        )
        .unwrap();
        assert_eq!(candidate.len(), docs.len());
        for (i, original) in docs.iter().enumerate() {
            assert_eq!(
                candidate.document(i),
                Some(*original),
                "exact canonical fresh document"
            );
        }
        let (refused, pressure) = allocation_probe::observe(|| {
            build_continuation_candidate::<0>(
                &verified, &checked, &full, &actual, &pins, &entries, &resources, 1,
            )
        });
        assert!(matches!(refused, Err(ContinuationScopeRefusal::Bounds)));
        assert_eq!(
            pressure.allocations, 0,
            "workspace shortage refuses before any reserve"
        );
        let mut foreign_boot = actual.clone();
        foreign_boot.boot_id = "other/fresh".into();
        assert!(build_continuation_candidate::<0>(
            &verified,
            &checked,
            &full,
            &foreign_boot,
            &pins,
            &entries,
            &resources,
            ceilings.runtime_bytes as usize,
        )
        .is_err());
    }
    if content {
        for mutation in 0..5 {
            let mut changed_actual = actual.clone();
            let held = changed_actual.resources[1].content.as_mut().unwrap();
            match mutation {
                0 => held.contract.maximum_items += 1,
                1 => held.base_id = "other/base".into(),
                2 => held.residence_profile = "other/profile".into(),
                3 => held.owner_boot = "stale/boot".into(),
                _ => held.contract.version = c::ResourceVersionIdentity::from_digest([9; 32]),
            }
            assert!(FreshContinuationScope::check_local_replacement::<0>(
                &verified,
                &checked,
                &full,
                &changed_actual,
                &pins,
                &entries,
                &docs,
                &mut scratch
            )
            .is_err());
        }
    }
    assert_eq!(accepted.fragment_count(), 3);
    assert_eq!(
        decode_body_fragment_document(accepted.document(0).unwrap())
            .unwrap()
            .0
            .body_plan,
        current.plan_id.as_str()
    );
    assert!(accepted.check_actual(&verified, &actual).is_ok());
    admission::check(&accepted, &verified, &actual);
    check(&accepted, &verified, &actual, &full, &pins, source);
    assert_eq!(
        wake.lifecycle,
        WakeLifecycle::AwaitingPlan,
        "local admission is NOT global Playing"
    );
    let mut foreign = actual.clone();
    foreign.host_id = "host/remote".into();
    assert!(FreshContinuationScope::check_local_replacement::<0>(
        &verified,
        &checked,
        &full,
        &foreign,
        &pins,
        &entries,
        &docs,
        &mut scratch
    )
    .is_err());
    let mut wrongboot = actual.clone();
    wrongboot.boot_id = "old/boot".into();
    assert!(FreshContinuationScope::check_local_replacement::<0>(
        &verified,
        &checked,
        &full,
        &wrongboot,
        &pins,
        &entries,
        &docs,
        &mut scratch
    )
    .is_err());
    let missing = ContinuationFullScope {
        plots: &fullpins[..3],
        ..full
    };
    assert!(FreshContinuationScope::check_local_replacement::<0>(
        &verified,
        &checked,
        &missing,
        &actual,
        &pins,
        &entries,
        &docs,
        &mut scratch
    )
    .is_err());
    let changed = ContinuationFullScope {
        workload_revision: full.workload_revision + 1,
        ..full
    };
    assert!(FreshContinuationScope::check_local_replacement::<0>(
        &verified,
        &checked,
        &changed,
        &actual,
        &pins,
        &entries,
        &docs,
        &mut scratch
    )
    .is_err());
    assert!(FreshContinuationScope::check_local_replacement::<0>(
        &verified,
        &checked,
        &full,
        &actual,
        &pins,
        &entries,
        &docs,
        &mut scratch[..1]
    )
    .is_err());
    let mut badpins = pins.clone();
    badpins[0].recipe_digest = [9; 32];
    assert!(FreshContinuationScope::check_local_replacement::<0>(
        &verified,
        &checked,
        &full,
        &actual,
        &badpins,
        &entries,
        &docs,
        &mut scratch
    )
    .is_err());
    let altered = BodyPlan::seal(
        &wake,
        current
            .plots
            .iter()
            .enumerate()
            .map(|(i, p)| BodyPlotPlan {
                plot: p.plot.clone(),
                plan: if i < 3 {
                    p.plan.clone()
                } else {
                    seal(&p.plan, "unauthorized/remote")
                },
            })
            .collect(),
    )
    .unwrap();
    let altered_play = BodyPlayIdentity::bind(&altered, 1);
    let altered_documents: Vec<_> = altered.plots[..3]
        .iter()
        .map(|p| {
            e::generate_prepared_body_fragment(
                PreparedBodyFragment::prepare(&wake, &altered, &altered_play, &p.plan.fragments[0])
                    .unwrap(),
                &[],
                limits(),
            )
            .unwrap()
        })
        .collect();
    let altered_refs: Vec<_> = altered_documents.iter().map(|d| d.bytes()).collect();
    assert!(
        FreshContinuationScope::check_local_replacement::<0>(
            &verified,
            &checked,
            &full,
            &actual,
            &pins,
            &entries,
            &altered_refs,
            &mut scratch
        )
        .is_err(),
        "changed nonlocal commitment cannot ride a local replacement"
    );
    assert_eq!(
        base.plots[3..],
        current.plots[3..],
        "nonlocal plans are byte-exact unchanged, never remotely Started"
    );
}
