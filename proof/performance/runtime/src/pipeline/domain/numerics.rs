//! Same exact 3/10 Source offered installed rational and certified binary64 Backs.
use super::*;
use conduit_core::{
    ExactNumericWork, ExactRational, MathematicalValue, NumericalAlgorithm,
    PreparedNumericalRealization, StructuredInfoValue, StructuredInfoValueShape,
};
const SOURCE:&str="plot numerics (\n exact: NumericalResult <= 8192B >>\n loose: NumericalResult <= 8192B >>\n) {\n precise: math/realize(request={value:0.3,maximum-error:0})\n approximate: math/realize(request={value:0.3,maximum-error:1})\n precise.result >> exact\n approximate.result >> loose\n}\n";
struct Projection {
    port: &'static str,
    algorithm: NumericalAlgorithm,
    expected: Vec<u8>,
    certificate: Value,
}
struct PreparedNumerics {
    host: StdHost,
    plan: Plan,
    phases: Vec<(&'static str, timing::Metrics)>,
    projections: Vec<Projection>,
    zero_error_float_refused: bool,
}
fn plan(
    authoring: &conduit_plot::ExpandedAuthoringPlot,
    host: &StdHost,
    placements: &conduit_planner::PlacementChoices,
) -> Result<Plan, String> {
    let boundaries = ["exact", "loose"]
        .into_iter()
        .map(|port| {
            (
                ForeBoundaryKey {
                    direction: PortDirection::Output,
                    front_port_id: port.into(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 8192,
                },
            )
        })
        .collect();
    plan_expanded_authoring_with_options(
        authoring,
        &[host.advertisement().clone()],
        placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8192,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .map_err(|e| format!("numerics Plan: {e:?}"))
}
fn prepare(boot: &str) -> Result<PreparedNumerics, Box<dyn Error>> {
    let mut phases = Vec::new();
    let (catalogs, m) = timing::measure(|| -> Result<_, String> {
        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profiles)?;
        Ok((startup, profiles))
    });
    phases.push(("numerical-type-catalog", m));
    let (startup, profiles) = catalogs?;
    let (syntax, m) = timing::measure(|| parse_syntax_document(SOURCE));
    phases.push(("parse", m));
    let (checked, m) = timing::measure(|| check_syntax_document(&syntax, &startup));
    phases.push(("check", m));
    let checked = checked.map_err(|e| format!("numerics check: {e:?}"))?;
    let (authored, m) =
        timing::measure(|| expand_canonical_plot_for_authoring(&checked, "numerics", &profiles));
    phases.push(("expand", m));
    let authored = authored.map_err(|e| format!("numerics expand: {e:?}"))?;
    let (host, m) = timing::measure(|| -> Result<_, String> {
        let mut host = StdHost::new_with_composition(
            StdHostConfig {
                host_id: "host/runtime-numerics".into(),
                boot_id: boot.into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            StdHostComposition::minimal(),
        );
        host.prepare_numerical_realizations(&authored.expanded)?;
        Ok(host)
    });
    phases.push(("source-specific-certified-offers", m));
    let host = host?;
    let mut placements =
        default_expanded_placements(&authored.expanded, &[host.advertisement().clone()])
            .map_err(|e| format!("numerics placement: {e:?}"))?;
    let mut projections = Vec::new();
    for gear in &authored.expanded.gears {
        let precise = gear.gear_id.as_str().ends_with("/precise");
        let port = if precise { "exact" } else { "loose" };
        let implementation = if precise {
            "conduit.std/numeric-rational@1"
        } else {
            "conduit.std/numeric-binary64-nearest@1"
        };
        let offer = host
            .advertisement()
            .capabilities
            .iter()
            .find(|o| o.implementation.implementation_id.as_str() == implementation)
            .ok_or("numeric implementation missing")?;
        placements
            .by_gear
            .get_mut(&gear.gear_id)
            .ok_or("numeric Gear placement missing")?
            .capability_id = offer.capability_id.clone();
        let mut work = ExactNumericWork::new(100_000_000);
        let prepared = PreparedNumericalRealization::from_property(
            &offer.realization_properties[0],
            &mut work,
        )
        .map_err(|e| format!("numeric certificate: {e:?}"))?;
        let expected = conduit_semantic_catalog::numerical_result_value(&prepared)?
            .canonical_bytes()
            .map_err(|e| format!("numeric receipt: {e:?}"))?;
        projections.push(Projection {port,algorithm:prepared.algorithm(),expected,certificate:json!({"port":port,"implementation":implementation,"prepared_limb_work":prepared.spent_limb_work(),"admitted_limb_work":prepared.admitted_limb_work(),"independent_validation_limb_work":work.spent(),"retained_payload_bytes":prepared.retained_payload_bytes()})});
    }
    let (sealed, m) = timing::measure(|| plan(&authored, &host, &placements));
    phases.push(("selected-plan-validation-and-sealing", m));
    let sealed = sealed?;
    if !conduit_core::verify_plan(&sealed) {
        return Err("numeric Plan seal invalid".into());
    }
    let mut bad = placements.clone();
    let precise = authored
        .expanded
        .gears
        .iter()
        .find(|g| g.gear_id.as_str().ends_with("/precise"))
        .ok_or("precise source missing")?;
    let float = host
        .advertisement()
        .capabilities
        .iter()
        .find(|o| {
            o.implementation.implementation_id.as_str() == "conduit.std/numeric-binary64-nearest@1"
        })
        .ok_or("float missing")?;
    bad.by_gear
        .get_mut(&precise.gear_id)
        .ok_or("precise placement missing")?
        .capability_id = float.capability_id.clone();
    let refused = plan(&authored, &host, &bad).is_err();
    if !refused {
        return Err("binary64 admitted for zero-error consumer".into());
    }
    Ok(PreparedNumerics {
        host,
        plan: sealed,
        phases,
        projections,
        zero_error_float_refused: refused,
    })
}
fn math_field(
    value: &StructuredInfoValue,
    name: &str,
) -> Result<MathematicalValue, Box<dyn Error>> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err("numeric receipt not record".into());
    };
    let field = fields
        .iter()
        .find(|f| f.name() == name)
        .ok_or("numeric field absent")?;
    let StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
        return Err("numeric field not mathematical leaf".into());
    };
    MathematicalValue::decode(bytes, &mut ExactNumericWork::new(100_000_000))
        .map_err(|e| format!("numeric field: {e:?}").into())
}
fn validate(out: &Outputs, projections: &[Projection]) -> Result<Value, Box<dyn Error>> {
    if out.values.len() != 2 {
        return Err("numeric output count differs".into());
    }
    let mut witnesses = Vec::new();
    for projection in projections {
        let output = out
            .values
            .iter()
            .find(|v| v.front_port_id.as_str() == projection.port)
            .ok_or("numeric output Fore missing")?;
        if output.track != ConnectionTrack::Payload
            || output.sequence != 0
            || output.bytes != projection.expected
        {
            return Err("numeric exact selected receipt differs".into());
        }
        let value = StructuredInfoValue::from_canonical_bytes(&output.bytes)
            .map_err(|e| format!("numeric receipt decode: {e:?}"))?;
        if output.value_kind
            != *value
                .value_type()
                .profile()
                .map_err(|e| format!("numeric profile: {e:?}"))?
                .value_kind()
        {
            return Err("numeric output Type differs".into());
        }
        let source = MathematicalValue::rational(
            ExactRational::ratio(3, 10, &mut ExactNumericWork::new(100_000_000))
                .map_err(|e| format!("source ratio: {e:?}"))?,
        );
        if math_field(&value, "source")? != source {
            return Err("numeric source denotation differs".into());
        }
        let lower = math_field(&value, "lower")?;
        let upper = math_field(&value, "upper")?;
        let error = math_field(&value, "maximum-error")?;
        let zero = MathematicalValue::rational(ExactRational::ZERO);
        if projection.port == "exact" {
            if lower != source || upper != source || error != zero {
                return Err("exact numerical projection differs".into());
            }
        } else {
            let target = MathematicalValue::rational(
                ExactRational::ratio(
                    5_404_319_552_844_595,
                    18_014_398_509_481_984,
                    &mut ExactNumericWork::new(100_000_000),
                )
                .map_err(|e| format!("binary64 rational: {e:?}"))?,
            );
            let expected_error = MathematicalValue::rational(
                ExactRational::ratio(
                    1,
                    90_071_992_547_409_920,
                    &mut ExactNumericWork::new(100_000_000),
                )
                .map_err(|e| format!("binary64 error: {e:?}"))?,
            );
            if lower != target || upper != target || error != expected_error {
                return Err("binary64 target/error differs".into());
            }
        }
        witnesses.push(json!({"port":projection.port,"canonical_sha256":artifact::digest(&output.bytes),"same_source_three_tenths":true,"zero_error":projection.port=="exact"}));
    }
    Ok(json!(witnesses))
}
pub(super) fn run(
    config: &Config,
    raw: &mut artifact::Raw,
    summaries: &mut Vec<Value>,
) -> Result<(), Box<dyn Error>> {
    let id = "numerical-projection-control";
    artifact::new_file(&config.output.join("sources").join(format!("{id}.plot")))?
        .write_all(SOURCE.as_bytes())?;
    let identity = artifact::digest(config.output.to_string_lossy().as_bytes());
    for repetition in 0..config.repetitions {
        let mut prepared = prepare(&format!("boot/runtime-numerics/{identity}/{repetition}"))?;
        artifact::json(
            &config
                .output
                .join("sources")
                .join(format!("{id}-{repetition}-plan.json")),
            &prepared.plan,
        )?;
        artifact::json(
            &config
                .output
                .join("sources")
                .join(format!("{id}-{repetition}-host.json")),
            prepared.host.advertisement(),
        )?;
        let mut isolation = Vec::new();
        for p in &prepared.projections {
            let implementation = if p.port == "exact" {
                "conduit.std/numeric-rational@1"
            } else {
                "conduit.std/numeric-binary64-nearest@1"
            };
            let gear = prepared.plan.fragments[0]
                .placements
                .iter()
                .find(|g| g.implementation_id.as_str() == implementation)
                .ok_or("numeric placement absent")?;
            let (back, metrics) = timing::measure(|| {
                conduit_semantic_catalog::NumericalRealizationBack::prepare(gear, p.algorithm)
            });
            let back = back?;
            if back.result_bytes() != p.expected {
                return Err("isolated numeric Back receipt differs".into());
            }
            isolation.push(json!({"port":p.port,"certificate":p.certificate,"back_preparation":metrics,"receipt_sha256":artifact::digest(back.result_bytes())}));
        }
        let mut outputs = Outputs::new(2);
        let mut phases = phase_observation::PhaseObserver::default();
        let (report, metrics) = timing::measure(|| {
            outputs.begin();
            phases.begin();
            let result = prepared.host.run_external_plot_to(
                prepared.plan.fragments[0].clone(),
                &[],
                &mut outputs,
                &mut std::io::sink(),
                &mut phases,
            );
            phases.end();
            result
        });
        let report = report?;
        observation::completed(&report)?;
        let witnesses = validate(&outputs, &prepared.projections)?;
        let kernel = report.kernel.as_ref().ok_or("numerics kernel absent")?;
        if kernel.value_allocation_capacity_before != kernel.value_allocation_capacity_after
            || kernel
                .kernel_sign
                .iter()
                .filter(|e| e.kind == conduit_kernel::KernelEventKind::RemoteValueDelivered)
                .count()
                != 2
        {
            return Err("numeric capacity/Signs differ".into());
        }
        let phases = phases.report()?;
        raw.row(&json!({"case":id,"mode":"certified-numerical-projections","phase":"ordinary-numerical-control","repetition":repetition,"source_sha256":artifact::digest(SOURCE.as_bytes()),"cold_phases":prepared.phases,"active_play":kernel.active_play,"checked_plot_id":prepared.plan.checked_plot_id,"full_host_call":metrics,"kernel_phase_wall":phases,"isolated_back_preparation":isolation,"validation_witnesses":witnesses,"zero_error_float_refused":prepared.zero_error_float_refused,"output_observations":outputs.report()?,"observations":report.observations,"signs":observation::signs(kernel),"kernel_decisions":kernel.decisions,"kernel_logical_invocations":kernel.logical_invocations,"value_capacity_before":kernel.value_allocation_capacity_before,"value_capacity_after":kernel.value_allocation_capacity_after,
            "scope":"same authored exact 3/10 with zero-error rational and one-unit-error binary64-nearest consumer; output bytes deliberately differ but decoded source, targets and exact error certificate are validated; prepared receipts contain no symbolic or bignum computation during Play, cold certificate limb work and allocator/time observations are separate; full Host pair includes mandatory typed transactions; no arithmetic-throughput or native-speed claim"}))?;
        summaries.push(json!({"case":id,"mode":"certified-numerical-projections","repetition":repetition,"full_host_call":metrics,"kernel_phase_wall":phases}));
        eprintln!("validated {id} repetition {}", repetition + 1);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_rational_and_binary64_execute_same_source_with_distinct_certified_results(
    ) -> Result<(), Box<dyn Error>> {
        let _measurement = crate::allocation::TEST_MEASUREMENT.lock().unwrap();
        let mut prepared = prepare("boot/numerical-conformance")?;
        let mut out = Outputs::new(2);
        let (result, _) = timing::measure(|| {
            out.begin();
            prepared.host.run_external_plot_to(
                prepared.plan.fragments[0].clone(),
                &[],
                &mut out,
                &mut std::io::sink(),
                &mut NoTimer,
            )
        });
        let report = result?;
        observation::completed(&report)?;
        validate(&out, &prepared.projections)?;
        assert!(prepared.zero_error_float_refused);
        let mut forged = prepared.plan.fragments[0].clone();
        forged.placements[0].artifact_id = "foreign/numeric-artifact".into();
        let mut refused = Outputs::new(2);
        assert!(prepared
            .host
            .run_external_plot_to(
                forged,
                &[],
                &mut refused,
                &mut std::io::sink(),
                &mut NoTimer
            )
            .is_err());
        assert!(refused.values.is_empty());
        let mut corrupted = out.values[0].bytes.to_vec();
        corrupted[0] ^= 1;
        out.values[0].bytes = corrupted.into();
        assert!(validate(&out, &prepared.projections).is_err());
        Ok(())
    }
}
