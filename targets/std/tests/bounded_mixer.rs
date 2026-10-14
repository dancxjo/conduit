use conduit_core::*;
use conduit_std_host::ExternalForeInput;
#[path = "bounded_mixer/fixture.rs"]
mod fixture;
use fixture::*;
#[test]
fn installed_two_and_four_port_graphs_preserve_exact_pairing_and_bounded_values() {
    for count in [2, 4] {
        let (mut host, plan, channel, _) = prepare(count);
        let ports = ["first", "second", "third", "fourth"];
        let inputs: Vec<_> = (0..2)
            .flat_map(|frame| {
                let channel = &channel;
                (0..count).rev().map(move |index| ExternalForeInput {
                    front_port_id: ports[index].into(),
                    track: ConnectionTrack::Payload,
                    bytes: channel_value(
                        channel,
                        (index as i32 + 1) * 100 + frame * 10,
                        256,
                        7,
                        11 + frame,
                        48000,
                    ),
                })
            })
            .collect();
        let mut outputs = Outputs::default();
        let report = host
            .run_external_plot_sequence_to(
                plan.fragments[0].clone(),
                &inputs,
                &mut outputs,
                &mut vec![],
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(outputs.0.len(), 2);
        for (frame, bytes) in outputs.0.iter().enumerate() {
            let result = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
            let StructuredInfoValueShape::Variant { tag, payload } = result.shape() else {
                panic!("outcome")
            };
            assert_eq!(tag, "sample");
            let StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
                panic!("sample")
            };
            let expected = if count == 2 { 300 } else { 1000 } + frame as i32 * count as i32 * 10;
            assert_eq!(i32::from_le_bytes(bytes.try_into().unwrap()), expected);
        }
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    }
}

#[test]
fn missing_input_uses_declared_finite_close_policy_without_fabricating_a_sample() {
    for count in [2, 4] {
        let (mut host, plan, channel, _) = prepare(count);
        let ports = ["first", "second", "third", "fourth"];
        let inputs: Vec<_> = ports[..count - 1]
            .iter()
            .map(|port| ExternalForeInput {
                front_port_id: (*port).into(),
                track: ConnectionTrack::Payload,
                bytes: channel_value(&channel, 100, 256, 7, 11, 48000),
            })
            .collect();
        let mut outputs = Outputs::default();
        let report = host
            .run_external_plot_sequence_to(
                plan.fragments[0].clone(),
                &inputs,
                &mut outputs,
                &mut vec![],
                &mut NoTimer,
            )
            .unwrap();
        assert!(outputs.0.is_empty());
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
    }
}
#[test]
fn installed_mixer_retains_source_profile_and_timebase_refusals() {
    for (sample, rate, expected) in [
        (32768, 48000, "invalid_profile"),
        (100, 44100, "timebase_mismatch"),
    ] {
        let (mut host, plan, channel, _) = prepare(2);
        let inputs = [
            ExternalForeInput {
                front_port_id: "first".into(),
                track: ConnectionTrack::Payload,
                bytes: channel_value(&channel, 100, 256, 7, 11, 48000),
            },
            ExternalForeInput {
                front_port_id: "second".into(),
                track: ConnectionTrack::Payload,
                bytes: channel_value(&channel, sample, 256, 7, 11, rate),
            },
        ];
        let mut outputs = Outputs::default();
        host.run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &inputs,
            &mut outputs,
            &mut vec![],
            &mut NoTimer,
        )
        .unwrap();
        assert_eq!(outputs.0.len(), 1);
        let value = StructuredInfoValue::from_canonical_bytes(&outputs.0[0]).unwrap();
        let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
            panic!("outcome")
        };
        assert_eq!(tag, expected);
        assert!(
            !matches!(payload.shape(), StructuredInfoValueShape::Leaf(bytes) if bytes.len()==4)
        );
    }
}
