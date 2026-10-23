//! CPU correlation proof; returned storage bytes are explicit fixtures.
use super::*;
use conduit_browser_runtime::resource_snapshot::PreparedSnapshotRecord;
use conduit_environment_browser::*;
use conduit_kernel::{self as k, scheduler::*};
#[path = "../recording_browser/binding.rs"]
pub(super) mod binding;

#[test]
fn browser_reads_complete_only_the_original_kernel_request() {
    for corrupt in [false, true] {
        execute(corrupt);
    }
}
fn execute(corrupt: bool) {
    let contract = ObservationContract::prepare().unwrap();
    let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    chunk
        .append(1, 60_000, RecordingDisposition::ProviderFailed, None)
        .unwrap();
    let content = chunk.seal().unwrap().to_owned();
    let mut reference = reference();
    reference.extent.bytes = content.len() as u64;
    let mut writer = binding::for_reference("fixture/store/old-boot", true, &reference);
    // Same durable owner across boots; the producer observation context is
    // independent of the store owner and current read authority.
    writer.host_id = "fixture/recorder".into();
    writer.authority[0].host_id = writer.host_id.clone();
    writer.resources[0].content.as_mut().unwrap().owner_host = writer.host_id.clone();
    let mut record = PreparedSnapshotRecord::prepare(&writer, &reference).unwrap();
    let mut bytes = record
        .publication(&writer.authority[0], content.as_bytes())
        .unwrap()
        .to_vec();
    if corrupt {
        *bytes.last_mut().unwrap() ^= 1;
    }
    let mut fixture = batch_plan::fixture(&[reference]);
    let plan = PreparedRecordingRecovery::prepare_batch(
        &fixture.source,
        &fixture.host,
        &fixture.choices,
        &bases(),
        &fixture.grants,
    )
    .unwrap();
    let prepared = PreparedRecordingBatchKernel::<4, 3>::prepare(
        &plan.plan().fragments[0],
        plan.lowered(),
        &mut fixture.generations,
        &mut fixture.singleton,
        &mut fixture.concat,
        &mut fixture.recovery,
    )
    .unwrap();
    let fragment = &plan.plan().fragments[0];
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    let mut stale = active.clone();
    stale.boot_id = "fixture/foreign".into();
    assert!(matches!(
        PreparedRecordingBrowserReads::prepare(
            fragment,
            plan.lowered(),
            &prepared.read_nodes,
            &stale
        ),
        Err(BrowserReadRefusal::Identity)
    ));
    assert!(matches!(
        PreparedRecordingBrowserReads::prepare(
            fragment,
            plan.lowered(),
            &[prepared.read_nodes[0]; 2],
            &active
        ),
        Err(BrowserReadRefusal::Shape)
    ));
    let mut reads = PreparedRecordingBrowserReads::prepare(
        fragment,
        plan.lowered(),
        &prepared.read_nodes,
        &active,
    )
    .unwrap();
    let placement = plan
        .lowered()
        .nodes
        .iter()
        .find(|node| node.node == prepared.read_nodes[0])
        .unwrap()
        .placement_id
        .clone();
    let basis = conduit_face::FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some(fragment.source_document_id.clone()),
        checked_plot_id: Some(fragment.checked_plot_id.clone()),
        expanded_plot_id: Some(fragment.expanded_plot_id.clone()),
        plan_id: Some(fragment.plan_id.clone()),
        active_play_id: Some(active.active_play_id.clone()),
        sign_ids: vec![],
    };
    let make_face =
        |basis| conduit_environment_face::PreparedRecordingFace::prepare(basis, 1, 64, 4).unwrap();
    assert!(matches!(
        PreparedRecordingBrowserFace::prepare(
            fragment,
            plan.lowered(),
            prepared.read_nodes[0],
            &active,
            make_face(basis.clone())
        ),
        Err(RecordingBrowserFaceRefusal::Binding)
    ));
    let mut foreign_basis = basis.clone();
    foreign_basis.plan_id = Some("fixture/foreign-plan".into());
    assert!(matches!(
        PreparedRecordingBrowserFace::prepare(
            fragment,
            plan.lowered(),
            prepared.recovery_node,
            &active,
            make_face(foreign_basis)
        ),
        Err(RecordingBrowserFaceRefusal::Binding)
    ));
    let mut face = PreparedRecordingBrowserFace::prepare(
        fragment,
        plan.lowered(),
        prepared.recovery_node,
        &active,
        make_face(basis),
    )
    .unwrap();
    let recovery = prepared.recovery_node;
    let mut kernel = prepared.kernel;
    let mut pending = None;
    for _ in 0..100 {
        if let Some(request) = kernel.next_host_request() {
            pending = Some(request);
            break;
        }
        kernel.step().unwrap();
    }
    let request = pending.unwrap();
    let (effect, begin_memory) =
        allocation_probe::observe(|| reads.begin(&kernel, request).unwrap());
    assert_eq!(
        (begin_memory.allocations, begin_memory.reallocations),
        (0, 0)
    );
    let json = serde_json::to_value(&effect).unwrap();
    assert_eq!(json["schema"], "conduit.browser/resource-effect@1");
    assert_eq!(json["effect_kind"], "resource-read");
    assert_eq!(json["request_sequence"], request.request.0);
    assert_eq!(json["placement_id"], placement.as_str());
    assert_eq!(json["record"], serde_json::Value::Null);
    let scope = RecordingReadScope {
        host_id: active.host_id.as_str(),
        boot_id: active.boot_id.as_str(),
        active_play_id: active.active_play_id.as_str(),
        placement_id: placement.as_str(),
        request_sequence: request.request.0,
    };
    let recovered = |kernel: &RecordingBatchKernel<4, 3>| {
        let RecordingBack::Recovery(back) = &kernel.drivers()[usize::from(recovery.0)] else {
            unreachable!()
        };
        back.recovered()
    };
    let ((), memory) = allocation_probe::observe(|| {
        assert!(reads.begin(&kernel, request).is_err());
        let mut forged = request;
        forged.input.admitted_bytes -= 1;
        assert!(reads
            .complete(&mut kernel, forged, scope, Ok(Some(&bytes)))
            .is_err());
        let wrong = RecordingReadScope {
            boot_id: "fixture/foreign",
            ..scope
        };
        assert_eq!(
            reads.complete(&mut kernel, request, wrong, Ok(Some(&bytes))),
            Err(BrowserReadRefusal::Identity)
        );
        assert_eq!(recovered(&kernel), RecoveredHistory::default());
        face.refresh(&kernel).unwrap();
        assert_eq!(face.current().revision, 0);
        assert_eq!(
            reads
                .complete(&mut kernel, request, scope, Ok(Some(&bytes)))
                .unwrap(),
            if corrupt {
                k::HostCallDisposition::Failed
            } else {
                k::HostCallDisposition::Completed
            }
        );
        assert_eq!(recovered(&kernel), RecoveredHistory::default());
        face.refresh(&kernel).unwrap();
        assert_eq!(face.current().revision, 0);
        assert!(reads
            .complete(&mut kernel, request, scope, Ok(Some(&bytes)))
            .is_err());
        assert!(reads.begin(&kernel, request).is_err());
        let mut terminal = false;
        for _ in 0..100 {
            match kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    assert!(!corrupt);
                    terminal = true;
                    break;
                }
                Err(SchedulerError::BackFailed(_)) => {
                    assert!(corrupt);
                    terminal = true;
                    break;
                }
                Ok(_) => {}
                other => panic!("unexpected kernel result: {other:?}"),
            }
        }
        assert!(terminal);
        face.refresh(&kernel).unwrap();
        assert_eq!(face.current().revision, if corrupt { 0 } else { 1 });
        face.refresh(&kernel).unwrap();
        assert_eq!(face.current().revision, if corrupt { 0 } else { 1 });
        if corrupt {
            face.publish_refusal(
                conduit_environment_face::RecordingRecoveryFailure::InvalidContent,
            )
            .unwrap();
            assert_eq!(face.current().revision, 1);
        }

        assert_eq!(recovered(&kernel).samples, if corrupt { 0 } else { 1 });
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
