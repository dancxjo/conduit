//! Installed execution of the actual imported Source graph, without an audio device.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("DSP package has no timer or device effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value);
        }
        Ok(())
    }
}
fn prepare() -> (StdHost, Plan, StructuredInfoType) {
    let package = conduit_audio::prepare_dsp_source_package().unwrap();
    let example = package
        .checked
        .plots
        .iter()
        .find(|p| p.name == "dsp/noise-gain-example")
        .unwrap();
    let mut profiles = ProfileCatalog::new();
    let mut advertisement = StdHost::new().advertisement().clone();
    for stage in example.cords.iter().flat_map(|cord| &cord.stages) {
        if let CheckedCordStage::StructuredSelector { selector, .. } = stage {
            let temporal = PortTemporal::Flow { closes: true };
            profiles
                .insert(structured_selector_definition(selector, temporal))
                .unwrap();
            advertisement
                .capabilities
                .push(conduit_std_offers::structured_selector_std_offer(
                    selector, temporal,
                ));
        }
    }
    let authored =
        expand_canonical_plot_for_authoring(&package.checked, &example.name, &profiles).unwrap();
    for gear in &authored.expanded.gears {
        if let Some(ConfigurationEntry {
            value: ConfigurationValue::Text(encoded),
            ..
        }) = gear.configuration.iter().find(|c| c.key == "program")
        {
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            let offer =
                conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                    .unwrap();
            assert!(gear.accepts_realization(&offer));
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement
        .capabilities
        .dedup_by(|a, b| a.capability_id == b.capability_id);
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "state"),
        (PortDirection::Output, "noise"),
        (PortDirection::Output, "pcm"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4096,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.source_document_id, package.checked.source_document_id);
    let input = package
        .checked
        .native_types
        .iter()
        .find(|t| t.name == "NoiseStepCandidate")
        .unwrap()
        .value_type
        .clone();
    (host, plan, input)
}
fn input(ty: &StructuredInfoType, word: i32, remaining: u32) -> ExternalForeInput {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    let value = StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| {
                let bytes = if f.name() == "word" {
                    word.to_le_bytes().to_vec()
                } else {
                    remaining.to_le_bytes().to_vec()
                };
                StructuredFieldValue::new(
                    f.name(),
                    StructuredInfoValue::leaf(f.value_type().clone(), bytes).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    ExternalForeInput {
        front_port_id: "state".into(),
        track: ConnectionTrack::Payload,
        bytes: value.canonical_bytes().unwrap(),
    }
}
#[test]
fn imported_noise_and_gain_run_through_the_installed_kernel_with_visible_refusals() {
    let (mut host, plan, ty) = prepare();
    let cases = [(0, 2), (1, 1), (65535, 2), (-1, 1), (0, 0), (0, 65537)];
    let inputs = cases
        .iter()
        .map(|&(word, remaining)| input(&ty, word, remaining))
        .collect::<Vec<_>>();
    let mut output = Outputs::default();
    let report = host
        .run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &inputs,
            &mut output,
            &mut vec![],
            &mut NoTimer,
        )
        .unwrap();
    let noise = output
        .0
        .iter()
        .filter(|o| o.front_port_id.as_str() == "noise")
        .collect::<Vec<_>>();
    let pcm = output
        .0
        .iter()
        .filter(|o| o.front_port_id.as_str() == "pcm")
        .collect::<Vec<_>>();
    assert_eq!(noise.len(), 6);
    assert_eq!(pcm.len(), 3);
    for (value, expected) in noise.iter().zip([
        "advanced",
        "advanced",
        "advanced",
        "invalid_seed",
        "exhausted",
        "invalid_profile",
    ]) {
        let decoded = StructuredInfoValue::from_canonical_bytes(&value.bytes).unwrap();
        let StructuredInfoValueShape::Variant { tag, .. } = decoded.shape() else {
            panic!("noise outcome")
        };
        assert_eq!(tag, expected);
    }
    for (value, &(word, _)) in pcm.iter().zip(&cases[..3]) {
        let decoded = StructuredInfoValue::from_canonical_bytes(&value.bytes).unwrap();
        let StructuredInfoValueShape::Variant { tag, payload } = decoded.shape() else {
            panic!("gain outcome")
        };
        assert_eq!(tag, "sample");
        let StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
            panic!("PCM sample")
        };
        let next = (i64::from(word) * 25173 + 13849) % 65536;
        let expected = ((next - 32768) * 128 / 256) as i32;
        assert_eq!(i32::from_le_bytes(bytes.try_into().unwrap()), expected);
    }
    let kernel = report.kernel.unwrap();
    assert_eq!(kernel.active_play.plan_id, plan.plan_id);
    assert_eq!(kernel.identity.plan_id, plan.plan_id);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    for kind in [
        conduit_kernel::KernelEventKind::HostCallRequested,
        conduit_kernel::KernelEventKind::HostCallCompleted,
    ] {
        assert!(kernel.kernel_sign.iter().any(|event| event.kind == kind));
    }
}
