//! Actual Root Source gate: re-sealing forged Plan metadata cannot replace the
//! independent checked graph/configuration. No MCU or effect readiness claim.
#[allow(
    dead_code,
    reason = "only the Host descriptor fixture is used by this Source gate"
)]
#[path = "support/mask.rs"]
mod fixture;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_mask_source.rs"]
mod source_check;
use conduit_core as c;
#[test]
fn root_cold_context_refuses_resealed_source_substitution_before_kernel_allocation() {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let planned = source.plan(&fixture::host(), 8192, &[]).unwrap();
    let configurations: Vec<_> = source
        .plot
        .expanded
        .gears
        .iter()
        .map(|g| {
            (
                g.gear_id.as_str(),
                g.kind_id.as_str(),
                c::face_data_label(&g.configuration).unwrap(),
            )
        })
        .collect();
    let connections: Vec<_> = source
        .plot
        .expanded
        .connections
        .iter()
        .map(|x| {
            (
                x.source_gear_id.as_str(),
                x.source_port_id.as_str(),
                x.sink_gear_id.as_str(),
                x.sink_port_id.as_str(),
            )
        })
        .collect();
    assert!(source_check::matches(
        &planned,
        &configurations,
        &connections
    ));
    for changed in ["label", "capacity", "route"] {
        let mut forged = planned.clone();
        let f = &mut forged.plan.fragments[0];
        match changed {
            "label" => {
                f.placements[0].configuration[0].value =
                    c::ConfigurationValue::Text("foreign".into())
            }
            "capacity" => f.connections[0].byte_capacity = 8191,
            _ => {
                let n = f.connections.len();
                f.connections[0] = f.connections[n - 1].clone();
            }
        }
        // Same Source identity, new correctly sealed Plan identity. The source
        // fingerprint on its own would be insufficient admission.
        forged.plan = c::seal_plan(forged.mask.plot_identity.clone(), forged.plan.fragments);
        if changed != "route" {
            assert!(c::verify_plan(&forged.plan), "resealed {changed}");
        }
        assert!(!source_check::matches(
            &forged,
            &configurations,
            &connections
        ));
        let result = conduit_face::PreparedMaskShowContext::from_wire_checked(
            serde_json::to_vec(&forged).unwrap().into_boxed_slice(),
            8192,
            32768,
            |candidate| source_check::matches(candidate, &configurations, &connections),
        );
        assert!(matches!(
            result,
            Err(conduit_face::MaskJsonRefusal::InvalidPlan)
        ));
    }
}
