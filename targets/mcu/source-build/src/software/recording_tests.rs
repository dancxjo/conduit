use super::*;
mod allocation_probe {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../architecture/plot/tests/common/allocation_probe.rs"
    ));
}
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn initialized_recording_owner_is_unbound_until_scope_and_binding_allocates_nothing() {
    use conduit_environment_runtime::recording::*;
    use conduit_kernel::{
        scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
        Failure, FailureCode, PortId, ValueRef,
    };
    let (host, boot, plan, play): (c::HostId, c::BootId, c::PlanId, c::ActivePlayId) = (
        "host/recording".into(),
        "boot/recording".into(),
        "plan/recording".into(),
        "play/recording".into(),
    );
    let context = LogContext {
        body: "body/recording",
        producer: RecordingContext {
            host: &host,
            boot: &boot,
            plan: &plan,
            play: &play,
        },
        stream: [1; 32],
    };
    let contract = ObservationContract::prepare().unwrap();
    let mut forms =
        conduit_dht11_observation::PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let canonical = forms.unpack(&[0; 4]).unwrap().to_vec();
    let mut back = RecordingLogBack::prepare_unbound(&contract, 2, || 100).unwrap();
    let value = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: canonical.len() as u32,
    };
    let frame = || StepIo::test_frame([Some(value)], [false], [Some(200)], None, 16);
    let input = StepInputBytes::test_frame([Some(canonical.as_slice())], None);
    assert!(matches!(
        back.step(&mut frame(), &input),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
    let parts = PreparedLogContext::prepare(context, &contract, 17)
        .unwrap()
        .parts();
    let ((), allocations) = allocation_probe::observe(|| {
        back.bind_scope(context, 17).unwrap();
        assert!(back.is_bound());
        assert_eq!(back.bind_scope(context, 17), Err(LogRefusal::AlreadyBound));
        for _ in 0..parts {
            let mut io = frame();
            assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
            assert!(!io.test_consumed(PortId(0)));
            assert!(
                <RecordingLogBack as StepBack<1>>::prepared_output(&back, PortId(0))
                    .unwrap()
                    .len()
                    <= 200
            );
            <RecordingLogBack as StepBack<1>>::step_committed(&mut back);
        }
        assert_eq!(back.next_sequence(), 17);
        let mut io = frame();
        assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
        assert!(io.test_consumed(PortId(0)));
        <RecordingLogBack as StepBack<1>>::step_committed(&mut back);
        assert_eq!(back.next_sequence(), 18);
        back.validate_scope(context, 17).unwrap();
        assert_eq!(back.next_sequence(), 18);
        let mut wrong = context;
        wrong.stream = [2; 32];
        assert_eq!(
            back.validate_scope(wrong, 17),
            Err(LogRefusal::ScopeMismatch)
        );
        assert_eq!(back.next_sequence(), 18);
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    println!(
        "recording owner inline bytes={} heap={}",
        core::mem::size_of::<RecordingLogBack>(),
        back.retained_backing_bytes()
    );
}

#[test]
fn authored_observation_selection_emits_scoped_recording_constructor_and_exact_contract() {
    let source = format!(
        "{}\n{}\n{}\n{}\n{}\n{}\n{}",
        include_str!("../../../../plots/device-protocols/pulse-types.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-capture.conduit"),
        include_str!("../../../../plots/device-protocols/dht11.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-read.conduit"),
        include_str!("../../../../plots/device-protocols/dht11-observe.conduit"),
        include_str!("../../../../bodies/environment/profiles/sampling.conduit"),
        r#"plot recorded-sampling (
            record: Bytes...| >>
        ) {
            sampling: environment-sampling(2,1000)
            encode: environment/encode-recording
            sampling.observed >> select(Dht11Result.observation, unmatched=drop) >> encode.observation
            encode.record >> record
        }"#
    );
    let sha = format!("{:x}", Sha256::digest(source.as_bytes()));
    let workset =
        crate::workset::compile(source.as_bytes(), &sha, &["recorded-sampling".into()], None)
            .unwrap();
    let material = compile(&workset.entries[0].plot.expanded, 24 * 1024).unwrap();
    let record = material
        .nodes
        .iter()
        .find(|n| matches!(n.role, SoftwareRole::RecordingLog { .. }))
        .unwrap();
    assert!(matches!(
        record.role,
        SoftwareRole::RecordingLog { maximum_samples: 2 }
    ));
    let offer = record.offer.as_ref().unwrap();
    assert_eq!(
        offer.kind_id.as_str(),
        conduit_environment_runtime::recording::LOG_KIND
    );
    assert!(workset.entries[0]
        .rust
        .contains("D::RecordingLog{maximum_samples:2}"));
    let pipeline = source
        .replace(
            "record: Bytes...| >>",
            "append_status: RecordAppendResult...| >>",
        )
        .replace(
            "encode: environment/encode-recording",
            "encode: environment/encode-recording\n append: resource/try-append-record",
        )
        .replace(
            "encode.record >> record",
            "encode.record >> append.record\n append.index >> append_status",
        );
    let sha = format!("{:x}", Sha256::digest(pipeline.as_bytes()));
    let pipeline = crate::workset::compile(
        pipeline.as_bytes(),
        &sha,
        &["recorded-sampling".into()],
        None,
    )
    .unwrap();
    let expanded = &pipeline.entries[0].plot.expanded;
    let append = expanded
        .gears
        .iter()
        .find(|g| g.kind_id.as_str() == conduit_semantic_catalog::RESOURCE_APPEND_KIND)
        .unwrap();
    assert_eq!(append.inputs[0].port_id.as_str(), "record");
    assert_eq!(append.outputs[0].port_id.as_str(), "index");
    assert!(pipeline.entries[0]
        .plot
        .output_bindings
        .iter()
        .any(|binding| binding.gear_id == append.gear_id
            && binding.gear_port_id.as_str() == "index"
            && binding.front_port_id.as_str() == "append_status"));
    assert!(pipeline.entries[0]
        .rust
        .contains(conduit_semantic_catalog::RESOURCE_APPEND_KIND));
    assert!(!compile(expanded, 24 * 1024)
        .unwrap()
        .nodes
        .iter()
        .any(|node| node.kind == append.kind_id));
}
