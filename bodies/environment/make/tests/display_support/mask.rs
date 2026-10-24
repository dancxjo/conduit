//! Interpreted actual Body Faces feed typed Frames to the ordinary output
//! kernel. Caller ownership, Boot and pins remain explicit CPU fixtures here;
//! this is not an installed Mask route, network Line or physical delivery.
use super::{
    execution::{native_service::DisplayService, output_owner},
    fixture::fixture,
};
use conduit_environment_make::{DisplayOutputSource, PreparedDisplayOutput};
use conduit_environment_runtime::display_output::PreparedDisplayKernel;
use conduit_face::Face;
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_seven_segment_mask::{DigitWiring, SevenSegmentLook, Tm1637Profile, WireBit};
use std::{cell::Cell, rc::Rc};
#[path = "../../examples/native_owner/display.rs"]
mod owner_display;

#[test]
fn body_face_temperature_and_rh_frames_execute_the_checked_output_graph() {
    let face: Face = serde_json::from_str(include_str!(
        "../../../evidence/2026-10-23-owner-shared-face/physical-face-1.json"
    ))
    .unwrap();
    let original = face.clone();
    let profile = Tm1637Profile {
        digits: core::array::from_fn(|index| DigitWiring {
            address: index as u8,
            segment_bits: [0, 1, 2, 3, 4, 5, 6],
        }),
        decimal_points: [None; 4],
        colon: Some(WireBit { address: 1, bit: 7 }),
    };
    let mut mask = owner_display::Display::prepare(
        profile,
        SevenSegmentLook::fahrenheit().with_colon_fraction(),
        3000,
        6000,
        3,
    )
    .unwrap();
    let frames = [0, 3000].map(|elapsed| {
        let frame = mask.render(&face, elapsed).unwrap();
        assert_eq!(frame.rendered.face_content_id, face.identity.as_str());
        assert_eq!(frame.rendered.face_revision, face.revision);
        (frame.canonical.to_vec(), frame.registers)
    });
    assert_ne!(frames[0].1, frames[1].1);
    let source = DisplayOutputSource::compile().unwrap();
    let (host, grant, mut software) = fixture(&source);
    let plan = PreparedDisplayOutput::prepare(
        &source,
        &host,
        &["fixture/base".into(), "conduit.base/local@1".into()],
        &[grant],
    )
    .unwrap();
    let mut execution =
        PreparedDisplayKernel::prepare(plan.plan(), plan.lowered(), &mut software, 2).unwrap();
    let edges = Rc::new(Cell::new(0));
    let mut owner = output_owner::bind(&plan, output_owner::Pins(edges.clone()));
    for (frame, _) in &frames {
        execution.input.validate_value(frame).unwrap();
    }
    assert_eq!(
        execution
            .kernel
            .admit_remote_input(
                execution.input.endpoint,
                execution.input.cord,
                0,
                &frames[0].0
            )
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0 }
    );
    let mut writes = 0;
    let mut drained = false;
    let ((), memory) = crate::allocation_probe::observe(|| {
        for _ in 0..100 {
            let pending = DisplayService {
                kernel: &mut execution.kernel,
                selectors: &mut execution.selectors,
                write: execution.write,
                owner: &mut owner,
            }
            .take();
            if let Some(pending) = pending {
                let request = pending.request();
                let writing = request.node == execution.write;
                if writing {
                    let packets = conduit_core::validate_canonical_structured_value(
                        execution.kernel.host_value(request.input.value).unwrap(),
                    )
                    .unwrap()
                    .record_field("packets")
                    .unwrap()
                    .unwrap();
                    assert_eq!(packets.collection_length().unwrap(), 3);
                    // Assert protocol bytes independently of the Source evaluator.
                    for (index, expected) in [
                        [0x40, 0, 0, 0, 0, 0, 0],
                        {
                            let mut wire = [0; 7];
                            wire[0] = 0xc0;
                            wire[1..].copy_from_slice(&frames[writes].1);
                            wire
                        },
                        [0x8b, 0, 0, 0, 0, 0, 0],
                    ]
                    .iter()
                    .enumerate()
                    {
                        let mut wire = [0; 7];
                        assert_eq!(
                            packets
                                .collection_index(index as u16)
                                .unwrap()
                                .unwrap()
                                .record_field("wire")
                                .unwrap()
                                .unwrap()
                                .copy_octet_collection("value/u8", &mut wire)
                                .unwrap(),
                            7
                        );
                        assert_eq!(&wire, expected);
                    }
                }
                let result = DisplayService {
                    kernel: &mut execution.kernel,
                    selectors: &mut execution.selectors,
                    write: execution.write,
                    owner: &mut owner,
                }
                .service(pending, || false)
                .unwrap();
                assert_eq!(result.failure, None);
                if writing {
                    writes += 1;
                    if writes == 1 {
                        assert_eq!(
                            execution
                                .kernel
                                .admit_remote_input(
                                    execution.input.endpoint,
                                    execution.input.cord,
                                    1,
                                    &frames[1].0
                                )
                                .unwrap(),
                            RemoteIngressOutcome::Accepted { sequence: 1 }
                        );
                        execution
                            .kernel
                            .close_remote_input(execution.input.endpoint, execution.input.cord)
                            .unwrap();
                    }
                }
            }
            assert!(execution
                .kernel
                .remote_egress_offer(execution.refusal.endpoint, execution.refusal.cord)
                .unwrap()
                .is_none());
            if execution.kernel.step().unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert!(drained);
    assert_eq!(writes, 2);
    assert!(edges.get() > 0);
    assert_eq!(face, original);
}
