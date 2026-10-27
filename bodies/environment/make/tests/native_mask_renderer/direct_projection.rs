//! Exact native selection parity and borrowed numeric audit; CPU pins only.
use super::*;
use conduit_plan_lowering::selected_operation::*;
fn verified(plan: &c::Plan) -> c::VerifiedPlanMetadata {
    let mut metadata = plan.clone();
    let configurations: std::collections::BTreeMap<_, _> = metadata
        .fragments
        .iter_mut()
        .flat_map(|f| &mut f.placements)
        .map(|g| {
            (
                g.placement_id.clone(),
                core::mem::take(&mut g.configuration),
            )
        })
        .collect();
    c::verify_plan_metadata_with_configurations(metadata, |g| {
        Ok::<_, ()>(configurations[&g.placement_id].clone())
    })
    .unwrap()
}
#[test]
fn direct_invocation_matches_complete_rich_capsule_and_removes_lowering_peak() {
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let planned = planned(&ready);
    let metadata = verified(&planned.plan);
    let fragment = &metadata.preparation_metadata().fragments[0];
    let gear = planned.show_placement();
    let active = c::bind_active_play(
        metadata.plan_id(),
        &ready.identity().host_id,
        &ready.identity().boot_id,
        1,
    );
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let material = conduit_environment_make::SevenSegmentMaskMaterial::prepare(&source).unwrap();
    // SAFETY: same checked Make Source just constructed above, no received bytes.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            material.planning_image(),
        )
    }
    .unwrap();
    let (configuration, source_memory) = allocation_probe::observe(|| {
        RendererSourceConfiguration::prepare(&image, &source.role.plot_identity, &gear.gear_id)
            .unwrap()
    });
    let (table, _, claim) = native::issue(&ready, &active);
    let (rich, rich_memory) = allocation_probe::observe(|| {
        let lowered = conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(
            &metadata,
            &fragment.fragment_id,
        )
        .unwrap();
        ready
            .prepare_verified_invocation(
                &table,
                &claim,
                &gear.configuration,
                SelectedVerifiedForeOperationPlan {
                    fore: &fragment.fore_ports,
                    operation: SelectedVerifiedOperationPlan {
                        plan: &metadata,
                        fragment_id: &fragment.fragment_id,
                        lowered: &lowered,
                        active: &active,
                        placement_id: &gear.placement_id,
                    },
                },
            )
            .unwrap()
    });
    let (direct, direct_memory) = allocation_probe::observe(|| {
        ready
            .prepare_direct_invocation(
                &table,
                &claim,
                &configuration,
                DirectRendererSelection {
                    plan: &metadata,
                    fragment_id: &fragment.fragment_id,
                    active: &active,
                    placement_id: &gear.placement_id,
                    fore: &fragment.fore_ports,
                },
            )
            .unwrap()
    });
    assert_eq!(direct.node(), rich.node());
    assert_eq!(direct.numeric_node(), rich.numeric_node());
    assert_eq!(direct.numeric_call(), rich.numeric_call());
    assert_eq!(direct.configuration(), rich.configuration());
    assert_eq!(direct.fore(), rich.fore());
    assert_eq!(pins.edges.get(), 0);
    println!("renderer direct invocation CPU source peak={} capsule peak={} retained={} rich-lowering+capsule peak={} retained={}",source_memory.peak_bytes,direct_memory.peak_bytes,direct_memory.retained_bytes(),rich_memory.peak_bytes,rich_memory.retained_bytes());
    assert!(direct_memory.peak_bytes+4096<rich_memory.peak_bytes,"must eliminate duplicated whole rich lowering: rich={rich_memory:?} direct={direct_memory:?}");
    assert!(
        direct_memory.peak_bytes < 8192,
        "selected projection must fit existing preparation reserve: {direct_memory:?}"
    );
}
#[test]
fn direct_and_rich_binding_refuse_wrong_fore_scope_configuration_and_actual_possession() {
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let planned = planned(&ready);
    let metadata = verified(&planned.plan);
    let fragment = &metadata.preparation_metadata().fragments[0];
    let gear = planned.show_placement();
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let material = conduit_environment_make::SevenSegmentMaskMaterial::prepare(&source).unwrap();
    // SAFETY: exact checked Make image; wrong variants below deliberately test refusal.
    let image = unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
            material.planning_image(),
        )
    }
    .unwrap();
    let configuration =
        RendererSourceConfiguration::prepare(&image, &source.role.plot_identity, &gear.gear_id)
            .unwrap();
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    f::install_mask_plot_value_aliases(&mut startup).unwrap();
    f::install_mask_mechanism_catalog(&mut startup, &mut profiles).unwrap();
    let text = include_str!("../../../../../plots/seven-segment-mask/main.conduit")
        .replace("Environmental display", "Different display");
    let syntax = conduit_plot::parse_syntax_document(&text);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let alternative =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "seven-segment", &profiles)
            .unwrap();
    let alternative_role = f::MaskPlot::admit(&alternative).unwrap();
    let bytes = conduit_plot::flat_image::indexed::encode(&alternative.expanded).unwrap();
    // SAFETY: complete freshly checked canonical expansion, never received bytes.
    let alternative_image =
        unsafe { conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(&bytes) }
            .unwrap();
    let alternative_gear = alternative
        .expanded
        .gears
        .iter()
        .find(|g| g.kind_id.as_str() == f::RENDERER_KIND)
        .unwrap();
    let wrong_configuration = RendererSourceConfiguration::prepare(
        &alternative_image,
        &alternative_role.plot_identity,
        &alternative_gear.gear_id,
    )
    .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(
        &metadata,
        &fragment.fragment_id,
    )
    .unwrap();
    for failure in [
        "fore",
        "configuration",
        "placement",
        "plan",
        "boot",
        "active-sequence",
        "claim-generation",
        "claim-envelope",
        "claim-size",
        "claim-work",
        "revoked",
    ] {
        let active = c::bind_active_play(
            metadata.plan_id(),
            &ready.identity().host_id,
            &ready.identity().boot_id,
            1,
        );
        let (mut table, handle, mut claim) = native::issue(&ready, &active);
        let mut selected_active = active.clone();
        let mut fore = fragment.fore_ports.clone();
        let mut placement = gear.placement_id.clone();
        match failure {
            "fore" => fore[0].byte_capacity -= 1,
            "placement" => placement = "wrong/placement".into(),
            "plan" => selected_active.plan_id = "wrong/plan".into(),
            "boot" => selected_active.boot_id = "wrong/boot".into(),
            "active-sequence" => selected_active.play_sequence += 1,
            "claim-generation" => claim.base_provider_generation += 1,
            "claim-envelope" => claim.envelope_id = "wrong/envelope".into(),
            "claim-size" => claim.parameter_bytes -= 1,
            "claim-work" => claim.work_units += 1,
            "revoked" => table.revoke(&handle).unwrap(),
            _ => {}
        }
        let direct = ready.prepare_direct_invocation(
            &table,
            &claim,
            if failure == "configuration" {
                &wrong_configuration
            } else {
                &configuration
            },
            DirectRendererSelection {
                plan: &metadata,
                fragment_id: &fragment.fragment_id,
                active: &selected_active,
                placement_id: &placement,
                fore: &fore,
            },
        );
        let rich = ready.prepare_verified_invocation(
            &table,
            &claim,
            if failure == "configuration" {
                &alternative_gear.configuration
            } else {
                &gear.configuration
            },
            SelectedVerifiedForeOperationPlan {
                fore: &fore,
                operation: SelectedVerifiedOperationPlan {
                    plan: &metadata,
                    fragment_id: &fragment.fragment_id,
                    lowered: &lowered,
                    active: &selected_active,
                    placement_id: &placement,
                },
            },
        );
        assert!(direct.is_err(), "direct {failure}");
        assert!(rich.is_err(), "rich {failure}");
        assert_eq!(pins.edges.get(), 0);
    }
    let mut wrong_identity = source.role.plot_identity.clone();
    wrong_identity.expanded_plot_id = "wrong/source".into();
    assert!(RendererSourceConfiguration::prepare(&image, &wrong_identity, &gear.gear_id).is_err());
    let foreign = &source
        .plot
        .expanded
        .gears
        .iter()
        .find(|g| g.kind_id.as_str() != f::RENDERER_KIND)
        .unwrap()
        .gear_id;
    assert!(
        RendererSourceConfiguration::prepare(&image, &source.role.plot_identity, foreign).is_err()
    );
}
