//! Compact ordinary Mask preparation; CPU services never claim physical output.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/mask.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_embedded_build as b;
use conduit_environment_runtime::{assigned::ResidentLayout, mask_kernel::*};
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
struct Material {
    wire: Vec<u8>,
    pins: Vec<c::AssignedRecordRequirement>,
    fore: Vec<c::AssignedForeRequirement>,
    roles: [MaskRole; 3],
    counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS],
    scope: AssignedMaskScope,
    context: conduit_face::PreparedMaskShowContext,
    active: c::ActivePlayIdentity,
    face: Vec<u8>,
    show: Vec<u8>,
}
impl Material {
    fn new() -> Self {
        let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
        let mut host = fixture::host();
        // Data-only fixture: it has no Base/resource/electrical attachment and
        // its renderer can return Prepared metadata only. Not a native provider.
        host.resources.clear();
        for offer in &mut host.capabilities {
            offer.resource_requirements.clear();
        }
        let planned = source.plan(&host, 8192, &[]).unwrap();
        let fragment = &planned.plan.fragments[0];
        let image = b::generate_fore_native_plan(
            &planned.plan,
            &fragment.fragment_id,
            b::EmbeddedImageBounds::HOST_TOOLING,
            b::EmbeddedComputationBounds {
                maximum_program_bytes: 0,
                maximum_total_program_bytes: 0,
            },
            3,
        )
        .unwrap();
        let maxima = Self::maxima();
        let wire = image.encode(maxima).unwrap();
        let mut pins = image.layout_requirements(maxima).unwrap();
        for record in c::AssignedPlanRecords::new(&wire).unwrap() {
            let record = record.unwrap();
            if record.tag == c::ASSIGNED_CONFIGURATION {
                pins.push(c::AssignedRecordRequirement::from_kernel_record(
                    record,
                    c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA,
                ));
            }
        }
        let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
        counts.copy_from_slice(&wire[80..93]);
        let roles = core::array::from_fn(|i| match image.kernel().nodes[i].kind_id.as_str() {
            conduit_face::FACE_TEE_KIND => MaskRole::Tee,
            conduit_face::RENDERER_KIND => MaskRole::Renderer,
            conduit_face::FACE_INTERACTION_KIND => MaskRole::ReadOnly,
            _ => panic!("unexpected checked Mask Kind"),
        });
        let scope = AssignedMaskScope {
            host: c::AssignedIdentity::from_text(host.host_id.as_str()),
            boot: c::AssignedIdentity::from_text(host.boot_id.as_str()),
            plan: c::AssignedIdentity::from_text(planned.plan.plan_id.as_str()),
            fragment: c::AssignedIdentity::from_text(fragment.fragment_id.as_str()),
        };
        let face = fixture::face();
        let active = c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1);
        let context = conduit_face::PreparedMaskShowContext::from_wire(
            serde_json::to_vec(&planned).unwrap().into_boxed_slice(),
            8192,
            32768,
        )
        .unwrap();
        let show = conduit_face::MaskShow::prepared(
            &planned,
            &face,
            c::bind_active_play(&planned.plan.plan_id, &host.host_id, &host.boot_id, 1),
            "environment".into(),
            "fixture/display".into(),
            "fixture/prepared".into(),
        )
        .unwrap();
        show.validate(&face).unwrap();
        let face = serde_json::to_vec(&face).unwrap();
        let show = serde_json::to_vec(&show).unwrap();
        Self {
            wire,
            pins,
            fore: image.fore().to_vec(),
            roles,
            counts,
            scope,
            context,
            active,
            face,
            show,
        }
    }
    fn maxima() -> c::AssignedPlanMaxima {
        c::AssignedPlanMaxima {
            encoded_bytes: 8192,
            runtime_state_bytes: u16::MAX,
            counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
        }
    }
    fn profile(&self) -> AssignedMaskProfile<'_> {
        AssignedMaskProfile {
            layout: ResidentLayout {
                records: &self.pins,
                counts: self.counts,
            },
            fore: &self.fore,
            roles: self.roles,
            present_call: c::AssignedIdentity::from_text("fixture/present"),
            resources: &[],
            face_bytes: 8192,
            show_bytes: 8192,
        }
    }
    fn records(&self) -> Vec<c::AssignedRecord<'_>> {
        c::AssignedPlanRecords::new(&self.wire)
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }
    fn prepare(&self) -> Result<PreparedAssignedMaskKernel, AssignedMaskRefusal> {
        let records = self.records();
        PreparedAssignedMaskKernel::prepare(
            AssignedMaskInput {
                header: &self.wire[..c::ASSIGNED_PLAN_HEADER_BYTES],
                records: &records,
                maxima: Self::maxima(),
            },
            &self.profile(),
            self.scope,
        )
    }
    fn change_record(&mut self, tag: u8, offset: usize) {
        let mut at = c::ASSIGNED_PLAN_HEADER_BYTES;
        while at < self.wire.len() {
            let length = u16::from_le_bytes(self.wire[at + 1..at + 3].try_into().unwrap()) as usize;
            if self.wire[at] == tag {
                self.wire[at + 3 + offset] ^= 1;
                break;
            }
            at += 3 + length;
        }
        let digest = c::assigned_plan_payload_digest(&self.wire[c::ASSIGNED_PLAN_HEADER_BYTES..]);
        self.wire[93..125].copy_from_slice(&digest);
    }
}
#[test]
fn compact_mask_requires_exact_scope_layout_configuration_and_fore() {
    let mut material = Material::new();
    assert!(material.prepare().is_ok());
    for offset in [16, 32, 48, 64] {
        material.wire[offset] ^= 1;
        assert!(material.prepare().is_err());
        material.wire[offset] ^= 1;
    }
    for (tag, offset) in [
        (c::ASSIGNED_NODE, 20),
        (c::ASSIGNED_CONFIGURATION, 19),
        (c::ASSIGNED_HOST_CALL, 4),
        (c::ASSIGNED_PORT, 21),
        (c::ASSIGNED_CORD, 32),
    ] {
        let original = material.wire.clone();
        material.change_record(tag, offset);
        assert!(material.prepare().is_err(), "tampered {tag}");
        material.wire = original;
    }
    let records = material.records();
    let input = AssignedMaskInput {
        header: &material.wire[..c::ASSIGNED_PLAN_HEADER_BYTES],
        records: &records,
        maxima: Material::maxima(),
    };
    let mut profile = material.profile();
    profile.roles.swap(0, 1);
    assert!(PreparedAssignedMaskKernel::prepare(input, &profile, material.scope).is_err());
    profile = material.profile();
    profile.face_bytes = 8191;
    assert!(PreparedAssignedMaskKernel::prepare(input, &profile, material.scope).is_err());
    profile = material.profile();
    profile.fore = &material.fore[..2];
    assert!(PreparedAssignedMaskKernel::prepare(input, &profile, material.scope).is_err());
}
#[test]
fn compact_mask_prepares_without_rich_plan_and_executes_the_ordinary_graph() {
    let material = Material::new();
    let records = material.records();
    let profile = material.profile();
    let face_model = fixture::face();
    let occurrence = material
        .context
        .prepared_show(
            &face_model,
            material.active.clone(),
            "environment".into(),
            "fixture/display".into(),
            "fixture/prepared".into(),
        )
        .unwrap();
    drop(face_model);
    assert_eq!(
        c::AssignedIdentity::from_text(material.context.plan_id().as_str()),
        material.scope.plan
    );
    let (mut prepared, memory) = allocation_probe::observe(|| {
        PreparedAssignedMaskKernel::prepare(
            AssignedMaskInput {
                header: &material.wire[..c::ASSIGNED_PLAN_HEADER_BYTES],
                records: &records,
                maxima: Material::maxima(),
            },
            &profile,
            material.scope,
        )
        .unwrap()
    });
    assert_eq!(
        memory.retained_bytes(),
        PreparedAssignedMaskKernel::required_heap_bytes(&profile).unwrap()
    );
    assert_eq!(memory.peak_bytes, memory.retained_bytes());
    println!(
        "Assigned Mask wire={} pins={} retained={} peak={}",
        material.wire.len(),
        material.pins.len(),
        memory.retained_bytes(),
        memory.peak_bytes
    );
    // Complete canonical Face and Prepared Show survive after the cold rich
    // Plan/models have dropped; no local handles or placeholder values enter Play.
    let face = &material.face;
    let show = &material.show[..];
    let (_, play) = allocation_probe::observe(|| {
        prepared
            .kernel
            .admit_remote_input_encoded(
                k::RemoteEndpointId(prepared.face.endpoint()),
                k::CordId(prepared.face.cord()),
                0,
                profile.face_bytes as usize,
                |out| {
                    out[..face.len()].copy_from_slice(face);
                    Ok::<_, ()>(face.len())
                },
            )
            .unwrap();
        prepared
            .kernel
            .close_remote_input(
                k::RemoteEndpointId(prepared.face.endpoint()),
                k::CordId(prepared.face.cord()),
            )
            .unwrap();
        let mut delivered = false;
        let mut drained = false;
        for _ in 0..64 {
            let status = prepared.kernel.step().unwrap();
            if let Some(request) = prepared.kernel.next_host_request() {
                assert_eq!(request.node, prepared.renderer);
                assert_eq!(request.request, k::RequestId(0));
                assert_eq!(
                    prepared.kernel.host_value(request.input.value).unwrap(),
                    *face
                );
                let value = prepared
                    .kernel
                    .store_host_value_encoded(profile.show_bytes as usize, |out| {
                        occurrence.encode_json_with(out, |part, buffer| {
                            serde_json_core::to_slice(&part, buffer)
                                .map_err(|_| conduit_face::MaskJsonRefusal::Json)
                        })
                    })
                    .unwrap();
                prepared
                    .kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Completed,
                            output: Some(k::BoundedValueRef::new(value, 8192).unwrap()),
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            if !delivered {
                if let Some(offer) = prepared
                    .kernel
                    .remote_egress_offer(
                        k::RemoteEndpointId(prepared.show.endpoint()),
                        k::CordId(prepared.show.cord()),
                    )
                    .unwrap()
                {
                    assert_eq!(prepared.kernel.host_value(offer.value).unwrap(), show);
                    prepared
                        .kernel
                        .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence)
                        .unwrap();
                    prepared
                        .kernel
                        .remote_egress_delivered(offer.endpoint, offer.cord, offer.sequence)
                        .unwrap();
                    delivered = true;
                }
            }
            if status == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained && delivered);
        assert_eq!(prepared.kernel.values().used_items(), 0);
    });
    assert_eq!((play.allocations, play.reallocations), (0, 0));
}

#[test]
fn encoded_ingress_refuses_before_writing_and_never_publishes_failed_encodings() {
    use k::SignSink;
    let material = Material::new();
    let mut prepared = material.prepare().unwrap();
    let endpoint = k::RemoteEndpointId(prepared.face.endpoint());
    let cord = k::CordId(prepared.face.cord());
    assert_eq!(
        prepared.kernel.admit_remote_input_encoded(
            k::RemoteEndpointId(u16::MAX),
            cord,
            0,
            8192,
            |_| panic!("refused encoder must not run")
        ),
        Err::<RemoteIngressOutcome, HostEncodingRefusal<()>>(HostEncodingRefusal::Scheduler(
            SchedulerError::InvalidRemoteCordAccess
        ))
    );
    assert_eq!(
        prepared
            .kernel
            .admit_remote_input_encoded(endpoint, cord, 0, 8192, |out| {
                out.fill(7);
                Err::<usize, _>(19)
            }),
        Err(HostEncodingRefusal::Encoding(
            k::EncodedValueRefusal::Encoder(19)
        ))
    );
    assert_eq!(prepared.kernel.values().used_items(), 0);
    assert!(prepared.kernel.signs().is_empty());
    assert_eq!(
        prepared
            .kernel
            .admit_remote_input_encoded(endpoint, cord, 0, 8192, |_| Ok::<_, ()>(8193)),
        Err(HostEncodingRefusal::Encoding(
            k::EncodedValueRefusal::Length
        ))
    );
    assert_eq!(prepared.kernel.values().used_items(), 0);
    assert!(prepared.kernel.signs().is_empty());
    assert_eq!(
        prepared
            .kernel
            .admit_remote_input_encoded(endpoint, cord, 0, 8192, |out| {
                out[..material.face.len()].copy_from_slice(&material.face);
                Ok::<_, ()>(material.face.len())
            })
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0 }
    );
    assert_eq!(
        prepared
            .kernel
            .admit_remote_input_encoded(endpoint, cord, 1, 8192, |_| panic!(
                "pressured encoder must not run"
            )),
        Ok::<_, HostEncodingRefusal<()>>(RemoteIngressOutcome::Full { sequence: 1 })
    );
    assert_eq!(prepared.kernel.values().used_items(), 1);
    prepared.kernel.cancel().unwrap();
    assert_eq!(prepared.kernel.values().used_items(), 0);
}
