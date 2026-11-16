//! Actual native NOR service plus the production kernel execution fixture.
use super::*;

pub(super) struct Workload {
    pub(super) kernel: Kernel,
    pub(super) service: RecordServiceHostCall<fixture::Storage>,
    pub(super) stats: fixture::Stats,
    pub(super) sampling: Option<HostCallRequest>,
    pub(super) expected: Vec<ArchivedSign>,
    pub(super) next_evidence: u32,
    pub(super) samples: u32,
    pub(super) maximum_result_bytes: usize,
}
// This fixture owns the complete kernel/value store and actual NOR provider.
// Its fixed unused input-reference allowance remains owned by Sampling until
// whole-fixture teardown; dropping the finite stores releases their backing.
// No such drop is claimed as a production replan/receiver-retirement protocol.
impl Workload {
    pub(super) fn new() -> Self {
        let (mut service, stats) = fixture::fixture_with_limits(
            false,
            1400,
            [APPEND_OPERATIONS, READ_OPERATIONS],
            NodeId(0),
        );
        // Test setup models already committed NOR contents. These are not new
        // Play operations or purported measurements/recording acknowledgments.
        for index in 0..HISTORY {
            assert_eq!(
                service.ready.provider.append(&payload(index)).unwrap(),
                DurableRecordIndex(index)
            );
        }
        let slots = [
            200,
            8,
            APPEND_BYTES,
            READ_BYTES as u32,
            200,
            APPEND_BYTES,
            READ_BYTES as u32,
            200,
            APPEND_BYTES,
            READ_BYTES as u32,
            1,
        ];
        let mut values =
            HostedValueStore::new_with_slot_capacities(&slots, slots.iter().sum()).unwrap();
        let empty = values.store(&[]).unwrap();
        // Each source Host Call transfers one input reference. Prepare exactly
        // the finite synthetic sampling allowance up front; never reuse a
        // consumed sole reference or grow storage during the workload.
        for _ in 1..APPEND_OPERATIONS {
            values.retain(empty).unwrap();
        }
        let shape = RecordServiceShape {
            append: RecordServiceOperation {
                input: PortId(0),
                output: PortId(0),
                call: HostCallId(1),
                maximum_input_bytes: 200,
                maximum_output_bytes: APPEND_BYTES,
            },
            read: RecordServiceOperation {
                input: PortId(1),
                output: PortId(1),
                call: HostCallId(0),
                maximum_input_bytes: 8,
                maximum_output_bytes: READ_BYTES as u32,
            },
        };
        let mut routes = FixedRoutes::<6, 3>::new(2);
        for (node, port, start, cord, sink) in [
            (
                0,
                0,
                0,
                2,
                CordEndpoint::Local {
                    node: NodeId(2),
                    port: PortId(0),
                },
            ),
            (0, 1, 1, 3, CordEndpoint::Remote(RemoteEndpointId(3))),
            (
                1,
                0,
                2,
                0,
                CordEndpoint::Local {
                    node: NodeId(0),
                    port: PortId(0),
                },
            ),
        ] {
            routes
                .install(
                    NodeId(node),
                    PortId(port),
                    RouteRange { start, len: 1 },
                    &[RouteTarget {
                        cord: CordId(cord),
                        sink,
                    }],
                )
                .unwrap();
        }
        routes.seal().unwrap();
        let capacity = |slot_start, byte_capacity| CordCapacity {
            slot_start,
            item_capacity: 1,
            byte_capacity,
            pressure_policy: Default::default(),
        };
        let cords = [
            CordSpec::local(
                CordId(0),
                (NodeId(1), PortId(0)),
                (NodeId(0), PortId(0)),
                capacity(0, 200),
            ),
            CordSpec::remote_ingress(
                CordId(1),
                RemoteEndpointId(1),
                (NodeId(0), PortId(1)),
                capacity(1, 8),
            ),
            CordSpec::local(
                CordId(2),
                (NodeId(0), PortId(0)),
                (NodeId(2), PortId(0)),
                capacity(2, APPEND_BYTES),
            ),
            CordSpec::remote_egress(
                CordId(3),
                (NodeId(0), PortId(1)),
                RemoteEndpointId(3),
                capacity(3, READ_BYTES as u32),
            ),
        ];
        let mut bindings = FixedHostCallBindings::<6>::new(3);
        for operation in [shape.read, shape.append] {
            bindings
                .install(
                    NodeId(0),
                    HostCallBinding {
                        call: operation.call,
                        maximum_input_bytes: operation.maximum_input_bytes,
                        maximum_output_bytes: operation.maximum_output_bytes,
                    },
                )
                .unwrap();
        }
        bindings
            .install(
                NodeId(1),
                HostCallBinding {
                    call: HostCallId(2),
                    maximum_input_bytes: 1,
                    maximum_output_bytes: 200,
                },
            )
            .unwrap();
        bindings.seal().unwrap();
        let kernel = Kernel::new_with_host_calls(
            [
                NodeSpec {
                    input_cords: [Some(CordId(0)), Some(CordId(1))],
                    maximum_step_fuel: 8,
                },
                NodeSpec {
                    input_cords: [None, None],
                    maximum_step_fuel: 8,
                },
                NodeSpec {
                    input_cords: [Some(CordId(2)), None],
                    maximum_step_fuel: 8,
                },
            ],
            cords,
            routes,
            bindings,
            [
                Back::Store(Box::new(RecordServiceBack::new(shape).unwrap())),
                Back::Sampling(Sampling {
                    empty: BoundedValueRef::new(empty, 1).unwrap(),
                    next: 0,
                    pending: false,
                    candidate: None,
                    completed: 0,
                }),
                Back::Sink(Sink {
                    candidate: None,
                    last_index: None,
                    completed: 0,
                }),
            ],
            values,
            evidence(),
        )
        .unwrap();
        let mut workload = Self {
            kernel,
            service,
            stats,
            sampling: None,
            expected: Vec::new(),
            next_evidence: 0,
            samples: 0,
            maximum_result_bytes: 0,
        };
        workload.drive();
        workload.assert_sampling_input_ownership();
        workload
    }
    fn assert_sampling_input_ownership(&self) {
        let Back::Sampling(sampling) = &self.kernel.drivers()[1] else {
            unreachable!()
        };
        let unused = APPEND_OPERATIONS - sampling.next;
        let pending = u32::from(sampling.pending);
        assert_eq!(
            self.kernel
                .values()
                .reference_count(sampling.empty.value)
                .unwrap(),
            u16::try_from(unused + pending).unwrap()
        );
        assert_eq!(sampling.next, sampling.completed + pending);
    }
    pub(super) fn drive(&mut self) {
        for _ in 0..128 {
            let status = self.kernel.step().unwrap();
            let mut requested = false;
            while let Some(request) = self.kernel.next_host_request() {
                requested = true;
                if request.node == NodeId(1) {
                    assert!(self.sampling.replace(request).is_none());
                    continue;
                }
                assert_eq!(request.node, NodeId(0));
                let mut input = [0; 200];
                let bytes = self.kernel.host_value(request.input.value).unwrap();
                input[..bytes.len()].copy_from_slice(bytes);
                let input_len = bytes.len();
                let mut output = [0; READ_BYTES];
                let length = self
                    .service
                    .invoke(
                        request.node,
                        request.call,
                        request.request,
                        &input[..input_len],
                        &mut output,
                    )
                    .unwrap();
                self.complete(request, &output[..length]);
                // Only the actual scheduler acceptance above acknowledges the
                // native result. No peer ACK or archive transfer substitutes.
                self.service
                    .acknowledge_kernel_completion(
                        request.node,
                        request.call,
                        request.request,
                        &output[..length],
                    )
                    .unwrap();
            }
            if status == SchedulerStatus::Idle && !requested {
                return;
            }
        }
        panic!("fixture failed to reach finite idle");
    }
    pub(super) fn complete(&mut self, request: HostCallRequest, bytes: &[u8]) {
        let value = self.kernel.store_host_value(bytes).unwrap();
        self.kernel
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(BoundedValueRef::new(value, bytes.len() as u32).unwrap()),
                    failure: None,
                },
            )
            .unwrap();
    }
    pub(super) fn sample(&mut self) {
        let request = self
            .sampling
            .take()
            .expect("same pending synthetic sampling Host Call");
        self.complete(request, &payload(HISTORY + self.samples));
        self.samples += 1;
        self.drive();
        let Back::Sampling(sampling) = &self.kernel.drivers()[1] else {
            unreachable!()
        };
        let Back::Sink(sink) = &self.kernel.drivers()[2] else {
            unreachable!()
        };
        assert_eq!(sampling.completed, self.samples);
        assert_eq!(sampling.next, (self.samples + 1).min(APPEND_OPERATIONS));
        self.assert_sampling_input_ownership();
        assert_eq!(sink.completed, self.samples);
        assert_eq!(sink.last_index, Some(HISTORY + self.samples - 1));
        assert_eq!(
            self.service.inspection(0).unwrap().completed_operations,
            self.samples
        );
    }
    pub(super) fn read(&mut self, index: u32, interleave: bool) {
        self.kernel
            .admit_remote_input(
                RemoteEndpointId(1),
                CordId(1),
                u64::from(index),
                &c::encode_count(u64::from(index)),
            )
            .unwrap();
        self.drive();
        let offer = self
            .kernel
            .remote_egress_offer(RemoteEndpointId(3), CordId(3))
            .unwrap()
            .unwrap();
        let bytes = self.kernel.host_value(offer.value).unwrap();
        self.maximum_result_bytes = self.maximum_result_bytes.max(bytes.len());
        let expected = payload(index);
        assert_eq!(
            crate::read_result::decode(bytes).unwrap(),
            ReadResult::Recorded {
                index,
                captured_end: HISTORY + self.samples,
                record: &expected,
            }
        );
        if interleave {
            self.sample();
            let held = self
                .kernel
                .remote_egress_offer(RemoteEndpointId(3), CordId(3))
                .unwrap()
                .unwrap();
            assert_eq!(
                held.value, offer.value,
                "append must not replace the unaccepted read"
            );
            assert_eq!(held.sequence, offer.sequence);
            assert_eq!(
                crate::read_result::decode(self.kernel.host_value(held.value).unwrap()).unwrap(),
                ReadResult::Recorded {
                    index,
                    captured_end: HISTORY + self.samples - 1,
                    record: &expected
                }
            );
        }
        self.kernel
            .remote_egress_accept(RemoteEndpointId(3), CordId(3), offer.sequence)
            .unwrap();
        self.kernel
            .remote_egress_delivered(RemoteEndpointId(3), CordId(3), offer.sequence)
            .unwrap();
        assert_eq!(
            self.service.inspection(1).unwrap().completed_operations,
            index + 1
        );
    }
    pub(super) fn archive(&mut self) -> Result<(), TransferRefusal> {
        let fresh: Vec<_> = self
            .kernel
            .signs()
            .events_with_remote()
            .filter(|entry| entry.event.sequence >= self.next_evidence)
            .collect();
        assert!(!fresh.is_empty());
        self.next_evidence = fresh.last().unwrap().event.sequence + 1;
        self.expected.extend(fresh);
        let before = self.kernel.signs().events_with_remote().collect::<Vec<_>>();
        let counters = (
            self.kernel.decisions(),
            self.kernel.logical_invocations(),
            self.service.inspection(0).unwrap().completed_operations,
            self.service.inspection(1).unwrap().completed_operations,
            self.service.next_request,
            self.kernel.pending_host_call_count(),
            self.sampling.as_ref().map(|request| request.request),
        );
        let result = self
            .kernel
            .prepare_sign_prefix_transfer()
            .map(|token| token.commit());
        assert_eq!(
            self.kernel.signs().events_with_remote().collect::<Vec<_>>(),
            before
        );
        assert_eq!(
            (
                self.kernel.decisions(),
                self.kernel.logical_invocations(),
                self.service.inspection(0).unwrap().completed_operations,
                self.service.inspection(1).unwrap().completed_operations,
                self.service.next_request,
                self.kernel.pending_host_call_count(),
                self.sampling.as_ref().map(|request| request.request),
            ),
            counters
        );
        result.map(|_| ())
    }
    pub(super) fn report(&self, name: &str, completed: u32, refusal: Option<TransferRefusal>) {
        println!("ACTUAL_SERVICE_ARCHIVE_ROW {{\"pattern\":\"{name}\",\"requested_reads\":1264,\"completed_reads\":{completed},\"sampling_completions\":{},\"archive_refusal\":\"{refusal:?}\",\"active_retained_events\":{},\"remote_retained_events\":{},\"archived_events\":{},\"archived_canonical_bytes\":{},\"owned_backing_bytes\":{},\"maximum_result_bytes\":{},\"nor_reads\":{},\"nor_programs\":{},\"active_items\":61,\"remote_items\":6,\"template_events\":128,\"template_runs\":16,\"remote_streams\":2,\"archive_event_limit\":32768,\"archive_canonical_byte_limit\":2097152,\"archive_backing_limit\":8192,\"overlap_limit\":12288,\"append_operation_limit\":970,\"read_operation_limit\":8192,\"scope\":\"real record-service Back, bound native operations and NOR; synthetic sampling Host Call; not Source graph, Root admission, transport or HIL\"}}",
            self.samples,self.kernel.signs().len(),self.kernel.signs().events_with_remote().filter(|entry|entry.remote.is_some()).count(),self.kernel.signs().archived_event_count(),self.kernel.signs().archived_canonical_bytes(),
            self.kernel.signs().owned_backing_bytes().unwrap(),self.maximum_result_bytes,
            self.stats.reads.get(),self.stats.programs.get());
    }
}
