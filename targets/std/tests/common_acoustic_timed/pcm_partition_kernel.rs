//! Exact packed PCM across producer blocks and the admitted commit Host Call.
use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "boundary.rs"]
mod boundary;
#[path = "terminal_signs.rs"]
mod terminal_signs;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
pub fn run(
    plan: Plan,
    front: &CheckedFront,
    registry: &conduit_composite::KernelOperationRegistry,
    items: &[Vec<u8>],
    partitions: &[usize],
    gap: usize,
    hold: usize,
) -> Vec<Vec<u8>> {
    assert_eq!(partitions.iter().sum::<usize>(), items.len());
    assert!(partitions.iter().all(|size| *size > 0));
    assert!(hold > 0);
    let mut kernel = boundary::prepare(
        plan,
        front,
        registry,
        conduit_composite::KernelCompositeSignStorage {
            additional_local_items: 256,
            additional_remote_items: 128,
        },
    );
    let inputs = items
        .iter()
        .map(|bytes| ValuePayload {
            value_kind: front.inputs()[0].value_kind.clone(),
            encoded: bytes.clone(),
        })
        .collect::<Vec<_>>();
    let mut results = items
        .iter()
        .map(|_| Vec::with_capacity(2077))
        .collect::<Vec<_>>();
    let mut output = ValuePayload {
        value_kind: front.outputs()[0].value_kind.clone(),
        encoded: Vec::with_capacity(2077),
    };
    let input = &front.inputs()[0].port_id;
    let out = &front.outputs()[0].port_id;
    let mut reply = Vec::with_capacity(2077);
    let mut requests = 0;
    let mut accepted = 0;
    let mut received = 0;
    let mut pressured = 0;
    let mut closed = false;
    let (_, storage) = allocation_probe::observe(|| {
        kernel.start().unwrap();
        let mut goal = 0;
        for partition in partitions {
            goal += partition;
            for _ in 0..gap {
                kernel.step().unwrap();
            }
            let mut holds = 0;
            let mut held = None;
            for _ in 0..100000 {
                while accepted < goal {
                    match kernel
                        .admit_input(input, accepted as u64, &inputs[accepted])
                        .unwrap()
                    {
                        RemoteIngressOutcome::Accepted { sequence } => {
                            assert_eq!(sequence, accepted as u64);
                            accepted += 1;
                        }
                        RemoteIngressOutcome::Full { sequence } => {
                            assert_eq!(sequence, accepted as u64);
                            pressured += 1;
                            break;
                        }
                    }
                }
                if accepted == inputs.len() && !closed {
                    assert!(received < inputs.len(), "closure precedes final delivery");
                    kernel.close_input(input).unwrap();
                    closed = true;
                }
                kernel.step().unwrap();
                if let Some(request) = kernel.next_host_request() {
                    let obligation = kernel.host_request_obligation(&request).unwrap();
                    let admitted = kernel
                        .admit_host_request(
                            &request,
                            &obligation.host,
                            &obligation.resources,
                            &obligation.authorities,
                        )
                        .unwrap();
                    let bytes = kernel.host_request_input(&admitted).unwrap();
                    let (header, samples) =
                        conduit_audio::PcmFrameHeader::decode_frame(bytes).unwrap();
                    assert_eq!(header.sample_rate_hz(), 8000);
                    assert_eq!(header.start_frame(), requests as u64);
                    assert_eq!(header.frame_count(), 1);
                    assert_eq!(samples, [24_i16, 104, 263, 425][requests].to_le_bytes());
                    requests += 1;
                    reply.clear();
                    reply.extend_from_slice(bytes);
                    kernel.complete_host_call_bytes(&admitted, &reply).unwrap();
                }
                if let Some(sequence) = kernel.output_into(out, &mut output).unwrap() {
                    assert_eq!(sequence, received as u64);
                    let digest = semantic_digest("fixture/timed-partition-held", &output.encoded);
                    if let Some(previous) = held {
                        assert_eq!(digest, previous);
                    }
                    held = Some(digest);
                    holds += 1;
                    if holds == hold {
                        results[received].extend_from_slice(&output.encoded);
                        kernel.complete_output(out, sequence).unwrap();
                        received += 1;
                        holds = 0;
                        held = None;
                    }
                }
                if received == goal {
                    break;
                }
            }
            assert_eq!(received, goal, "finite producer block failed to drain");
        }
        assert!(closed);
        assert_eq!(received, inputs.len());
        let mut complete = false;
        for _ in 0..64 {
            complete |=
                kernel.step().unwrap() == conduit_composite::KernelCompositeStatus::Complete;
        }
        assert!(complete);
        assert_eq!(
            kernel.output_terminal_into(out, &mut output).unwrap(),
            Some(conduit_composite::KernelCompositeTerminal::Normal)
        );
    });
    assert_eq!(
        (storage.allocations, storage.reallocations),
        (0, 0),
        "{storage:?}"
    );
    assert_eq!(requests, items.len(), "one commit call per accepted frame");
    if partitions.iter().any(|size| *size > 1) {
        assert!(pressured > 0);
    }
    terminal_signs::normal(&kernel);
    println!(
        "pcm partitions={partitions:?} full={pressured} plays={:?} storage={storage:?}",
        kernel.active_plays()
    );
    results
}
