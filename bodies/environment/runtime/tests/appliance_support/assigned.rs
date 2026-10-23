//! Make-side CPU recipe construction. Firmware must retain this independently
//! of received assignment bytes; this is not a native provider or an issuer.
use super::*;
use conduit_environment_runtime::appliance::assigned::{ResidentAppliance, ResidentRoles};
pub struct Recipe {
    records: Vec<c::AssignedRecordRequirement>,
    source: conduit_environment_runtime::appliance::assigned::ResidentSourceIdentity,
    cadence: conduit_time::TickConfiguration,
    texts: Vec<(u16, c::AssignedIdentity, String, Vec<u8>)>,
    counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS],
    roles: ResidentRoles,
}
impl Recipe {
    pub fn prepare(
        plan: &c::Plan,
        lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
        image: &conduit_embedded_build::GeneratedEmbeddedPlan,
        projection: &assignment::NativeProjection,
    ) -> Self {
        let find = |kind: &str| {
            let gear = plan.fragments[0]
                .placements
                .iter()
                .find(|g| g.kind_id.as_str() == kind)
                .unwrap();
            lowered
                .identity
                .node_for_placement(&gear.placement_id)
                .unwrap()
        };
        let roles = ResidentRoles {
            pulse: find(conduit_pulse_capture::PULSE_READ_KIND),
            tick: find(conduit_time::TICK_KIND),
            trigger: find(conduit_time::TICK_TRIGGER_KIND),
            observation: find(conduit_environment_runtime::observation::KIND),
            publications: Channel::ALL.map(|channel| find(channel.kind_id())),
        };
        let mut records = conduit_embedded_build::assigned_native_layout_requirements(
            image,
            assignment::NativeProjection::maxima(),
        )
        .unwrap();
        records.extend(
            c::AssignedPlanRecords::new(&projection.bytes)
                .unwrap()
                .map(Result::unwrap)
                .filter(|r| r.tag == c::ASSIGNED_CONFIGURATION)
                .map(c::AssignedRecordRequirement::from_record),
        );
        let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
        counts.copy_from_slice(&projection.bytes[80..80 + c::ASSIGNED_PLAN_COUNT_KINDS]);
        Self {
            cadence: conduit_time::parse_tick_configuration(&plan.fragments[0].placements.iter().find(|g|g.kind_id.as_str()==conduit_time::TICK_KIND).unwrap().configuration).unwrap(),
            source: conduit_environment_runtime::appliance::assigned::ResidentSourceIdentity::from_expanded(&plan.fragments[0].expanded_plot_id),
            records,
            texts: image
                .configuration
                .iter()
                .filter_map(|entry| {
                    if !matches!(entry.key.as_str(), "program" | "selector") {
                        return None;
                    }
                    let conduit_embedded_build::GeneratedConfigurationValue::Text(text) =
                        &entry.value
                    else {
                        return None;
                    };
                    Some((
                        entry.node,
                        c::AssignedIdentity::from_text(&entry.key),
                        text.clone(),
                        text.as_bytes()
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|pair| {
                                u8::from_str_radix(
                                    core::str::from_utf8(pair).expect("checked hex"),
                                    16,
                                )
                                .expect("checked hex")
                            })
                            .collect(),
                    ))
                })
                .collect(),
            counts,
            roles,
        }
    }
    pub fn programs(
        &self,
    ) -> Vec<conduit_environment_runtime::appliance::assigned::ResidentComputation<'_>> {
        self.texts
            .iter()
            .map(|(node, key, text, bytes)| {
                conduit_environment_runtime::appliance::assigned::ResidentComputation {
                    node: *node,
                    key: *key,
                    text,
                    canonical_bytes: Some(bytes),
                }
            })
            .collect()
    }
    pub fn resident<'a>(
        &'a self,
        projection: &'a assignment::NativeProjection,
    ) -> ResidentAppliance<'a> {
        ResidentAppliance {
            cadence: self.cadence.clone(),
            source: self.source,
            layout: conduit_environment_runtime::assigned::ResidentLayout {
                records: &self.records,
                counts: self.counts,
            },
            computations: projection.profile(),
            roles: self.roles,
        }
    }
}

#[test]
fn assigned_preparation_refuses_wrong_root_scope_roles_and_storage_before_effects() {
    use conduit_environment_runtime::appliance::assigned::{
        NativePreparationRefusal as E, PreparedAssignedAppliance,
    };
    let source = compile(1, 0).unwrap();
    let (ready, effects) = pulse::ready(0);
    let mut software = software(&source);
    let host = appliance_support::advertisement(&source, &mut software, &ready);
    let plan = source
        .plan_local(&host, &["conduit.base/local@1".into()], &[pulse::grant()])
        .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    let image = appliance_support::native_image(&plan, &lowered);
    let projection = assignment::NativeProjection::prepare(&image);
    let recipe = Recipe::prepare(&plan, &lowered, &image, &projection);
    for mode in 0..6 {
        let mut resident = recipe.resident(&projection);
        let mut required = projection.required();
        match mode {
            0 => resident.roles.pulse = resident.roles.tick,
            1 => resident.roles.publications.swap(0, 1),
            2 => resident.roles.observation = k::NodeId(15),
            3 => required.host = c::AssignedIdentity::from_text("foreign/host"),
            4 => required.boot = c::AssignedIdentity::from_text("stale/boot"),
            _ => {}
        }
        // Deliberately inadequate wait storage in mode 5. Other refusals precede
        // the first software allocation and leave native Ready ownership intact.
        let values = k::FixedValueStore::<1, 1>::new(1).unwrap();
        let (result, memory) = allocation_probe::observe(|| {
            PreparedAssignedAppliance::prepare(
                &projection.bytes,
                assignment::NativeProjection::maxima(),
                required,
                &resident,
                values,
            )
        });
        match mode {
            0..=2 => assert!(matches!(result, Err(E::Roles))),
            3..=4 => assert!(matches!(result, Err(E::Tables(_)))),
            _ => assert!(matches!(result, Err(E::Storage(_)))),
        }
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(effects.get(), 0);
    }
    // A changed program remains an untrusted assignment, not a new resident
    // inventory. Independent pinned hashes must refuse it before initialization.
    let mut altered = projection.bytes.clone();
    let text = projection.computations[0].text_bytes as usize;
    let mut cursor = c::ASSIGNED_PLAN_HEADER_BYTES;
    while cursor < altered.len() {
        let length = usize::from(u16::from_le_bytes([
            altered[cursor + 1],
            altered[cursor + 2],
        ]));
        if altered[cursor] == c::ASSIGNED_CONFIGURATION
            && length == 19 + text
            && altered[cursor + 3 + 18] == 3
        {
            altered[cursor + 3 + 19] = if altered[cursor + 3 + 19] == b'0' {
                b'1'
            } else {
                b'0'
            };
            break;
        }
        cursor += 3 + length;
    }
    assert!(cursor < altered.len());
    let (result, memory) = allocation_probe::observe(|| {
        PreparedAssignedAppliance::prepare(
            &altered,
            assignment::NativeProjection::maxima(),
            projection.required(),
            &recipe.resident(&projection),
            k::FixedValueStore::<1, 8>::new(8).unwrap(),
        )
    });
    assert!(matches!(result, Err(E::Tables(_))));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(effects.get(), 0);
}

#[test]
fn resident_initialization_refuses_missing_duplicate_or_changed_text_before_allocating() {
    use conduit_environment_runtime::appliance::assigned::{
        NativePreparationRefusal as E, ValidatedAssignedAppliance,
    };
    let source = compile(1, 0).unwrap();
    let (ready, effects) = pulse::ready(0);
    let mut software = software(&source);
    let host = appliance_support::advertisement(&source, &mut software, &ready);
    let plan = source
        .plan_local(&host, &["conduit.base/local@1".into()], &[pulse::grant()])
        .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    let image = appliance_support::native_image(&plan, &lowered);
    let projection = assignment::NativeProjection::prepare(&image);
    let recipe = Recipe::prepare(&plan, &lowered, &image, &projection);
    for mode in 0..7 {
        let admitted = ValidatedAssignedAppliance::admit(
            &projection.bytes,
            assignment::NativeProjection::maxima(),
            projection.required(),
            &recipe.resident(&projection),
        )
        .unwrap();
        let mut texts = recipe.programs();
        let mut changed = texts[0].canonical_bytes.unwrap().to_vec();
        changed[0] ^= 1;
        match mode {
            0 => {
                texts.pop();
            }
            1 => texts[1] = texts[0],
            2 => texts[0].text = "00",
            3 => texts[0].node = u16::MAX,
            4 => texts[0].key = c::AssignedIdentity::from_text("unknown"),
            5 => texts[0].canonical_bytes = Some(&changed),
            _ => texts[0].canonical_bytes = Some(&changed[..changed.len() - 1]),
        }
        let values = k::FixedValueStore::<1, 8>::new(8).unwrap();
        let (result, memory) = allocation_probe::observe(|| admitted.initialize(&texts, values));
        assert!(matches!(result, Err(E::Configuration)));
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert_eq!(effects.get(), 0);
        let values = k::FixedValueStore::<1, 8>::new(8).unwrap();
        let (result, memory) = allocation_probe::observe(|| {
            conduit_environment_runtime::appliance::assigned::PreparedResidentSoftware::prepare(
                &recipe.resident(&projection),
                &texts,
                &conduit_time::TickConfiguration {
                    count: 1,
                    period_ms: 0,
                },
                values,
            )
        });
        assert!(matches!(result, Err(E::Configuration)));
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
}

#[test]
fn retained_resident_software_refuses_other_source_then_transfers_without_allocation() {
    use conduit_environment_runtime::appliance::assigned::{
        PreparedResidentSoftware, ValidatedAssignedAppliance,
    };
    let (projection, recipe) = transfer_fixture(1);
    let texts = recipe.programs();
    for mode in 0..3 {
        let mut resident = recipe.resident(&projection);
        match mode {
            0 => resident.computations.maximum_text_bytes = 0,
            1 => resident.computations.maximum_total_text_bytes = 0,
            _ => resident.roles.pulse = resident.roles.tick,
        }
        let values = k::FixedValueStore::<1, 8>::new(8).unwrap();
        let (result, memory) = allocation_probe::observe(|| {
            PreparedResidentSoftware::prepare(
                &resident,
                &texts,
                &conduit_time::TickConfiguration {
                    count: 1,
                    period_ms: 0,
                },
                values,
            )
        });
        assert!(matches!(result, Err(conduit_environment_runtime::appliance::assigned::NativePreparationRefusal::Configuration | conduit_environment_runtime::appliance::assigned::NativePreparationRefusal::Roles)));
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    let mut retained = Some(
        PreparedResidentSoftware::prepare(
            &recipe.resident(&projection),
            &texts,
            &conduit_time::TickConfiguration {
                count: 1,
                period_ms: 0,
            },
            conduit_environment_runtime::appliance::prepare_value_storage().unwrap(),
        )
        .unwrap(),
    );
    let (other, other_recipe) = transfer_fixture(2);
    let wrong = ValidatedAssignedAppliance::admit(
        &other.bytes,
        assignment::NativeProjection::maxima(),
        other.required(),
        &other_recipe.resident(&other),
    )
    .unwrap();
    let (result, memory) =
        allocation_probe::observe(|| PreparedResidentSoftware::bind_retained(&mut retained, wrong));
    assert!(result.is_err());
    assert!(retained.is_some());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    // Source identity remains exact even when all realized graph records match.
    let mut different_source = recipe.resident(&projection);
    different_source.source.expanded_plot_digest[0] ^= 1;
    let selected = ValidatedAssignedAppliance::admit(
        &projection.bytes,
        assignment::NativeProjection::maxima(),
        projection.required(),
        &different_source,
    )
    .unwrap();
    let (result, memory) = allocation_probe::observe(|| {
        PreparedResidentSoftware::bind_retained(&mut retained, selected)
    });
    assert!(result.is_err());
    assert!(retained.is_some());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    // Equivalent record order is representation, not different construction.
    let mut reversed = recipe.records.clone();
    reversed.reverse();
    let mut resident = recipe.resident(&projection);
    resident.layout.records = &reversed;
    let selected = ValidatedAssignedAppliance::admit(
        &projection.bytes,
        assignment::NativeProjection::maxima(),
        projection.required(),
        &resident,
    )
    .unwrap();
    let (result, memory) = allocation_probe::observe(|| {
        PreparedResidentSoftware::bind_retained(&mut retained, selected)
    });
    assert!(retained.is_none());
    assert!(result.is_ok());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

/// Preparation diagnostic over the same independently pinned computation text.
/// Values and native providers are excluded; these are requested heap bytes.
pub fn report_computation_memory(recipe: &Recipe) {
    for text in recipe.programs() {
        let ((driver, selector), memory) = allocation_probe::observe(|| {
            if text.key == c::AssignedIdentity::from_text("program") {
                let program = conduit_plot::PortableExpressionProgram::from_canonical_bytes(
                    text.canonical_bytes.unwrap(),
                )
                .unwrap();
                (
                    CaptureBack::expression(&program, c::PortTemporal::Flow { closes: true })
                        .unwrap(),
                    None,
                )
            } else {
                let selector = c::StructuredSelector::from_canonical_hex(text.text).unwrap();
                (
                    CaptureBack::selector(1024),
                    Some(PreparedCaptureSelector::prepare(selector, 1024, 1024).unwrap()),
                )
            }
        });
        eprintln!("environment_computation_memory pointer_bits={} node={} text_bytes={} memory={memory:?}", usize::BITS, text.node, text.text.len());
        drop((driver, selector));
    }
}

fn transfer_fixture(count: u64) -> (assignment::NativeProjection, Recipe) {
    let source = compile(count, 0).unwrap();
    let (ready, _) = pulse::ready(0);
    let mut software = software(&source);
    let host = appliance_support::advertisement(&source, &mut software, &ready);
    let plan = source
        .plan_local(&host, &["conduit.base/local@1".into()], &[pulse::grant()])
        .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    let image = appliance_support::native_image(&plan, &lowered);
    let projection = assignment::NativeProjection::prepare(&image);
    let recipe = Recipe::prepare(&plan, &lowered, &image, &projection);
    (projection, recipe)
}

#[path = "boxed_transfer.rs"]
mod boxed_transfer;
