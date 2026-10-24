//! Actual native worker -> retained Face -> Mask/Look -> checked output kernel.
//! Same Host/Boot, explicit CPU native/authority/clock fixtures; no MCU admission.
use super::*;
use crate::native_worker::{Execution, Exit};
use crate::{display_fixture as fixture, service as display_service};
use conduit_environment_make::{DisplayOutputSource, DisplaySourceMaterial, PreparedDisplayOutput};
use conduit_environment_runtime::{
    assigned::ResidentLayout,
    display_mask::PreparedDisplayMask,
    display_output::{
        AssignedDisplayInput, AssignedDisplayProfile, AssignedDisplayScope, DisplaySignBudget,
        PreparedAssignedDisplayKernel,
    },
    publication::{PublicationSink, PublicationSnapshot},
    publication_face_custody::{LocalPublicationFace, Publisher, Reader},
    retained_publication_face::RetainedPublicationFace,
};
use conduit_face::FaceBasis;
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerError};
use conduit_seven_segment_mask::{
    DigitWiring, MaskDisposition, SevenSegmentLook, Tm1637Profile, WireBit,
};
#[allow(
    dead_code,
    reason = "shared native authority fixture also supports alternate output profiles"
)]
#[path = "../display_support/output_owner.rs"]
mod output_owner;
type Output = PreparedAssignedDisplayKernel<k::HostedValueStore, k::HostedSignLog>;
struct Sink<'a> {
    publisher: Publisher<'a>,
    reader: Reader<'a>,
    mask: PreparedDisplayMask,
    output: Output,
    owner: conduit_tm1637_wire::owner::WriteHostCall<output_owner::Pins>,
    period_ms: u64,
    writes: u32,
    cancel_output: bool,
    failure: Option<k::Failure>,
}
impl PublicationSink for Sink<'_> {
    fn poll_publication(
        &mut self,
        snapshot: PublicationSnapshot<'_>,
        _: &mut Context<'_>,
    ) -> Poll<Result<(), k::Failure>> {
        let now = snapshot.sequence.checked_mul(self.period_ms).unwrap(); // producer clock fixture
        self.publisher.publish(snapshot, now).unwrap();
        if snapshot.sequence != 1 {
            return Poll::Ready(Ok(()));
        }
        let show = self
            .reader
            .with_current(now, |face, status| {
                assert!(status.publisher_retained && status.refusal.is_none());
                self.mask.prepare_show(face).unwrap()
            })
            .unwrap();
        Poll::Ready(self.display(&show))
    }
}
impl Sink<'_> {
    // Output scratch is never nested inside canonical quantity publication.
    #[inline(never)]
    fn display(
        &mut self,
        show: &conduit_seven_segment_mask::PreparedAlternatingShow,
    ) -> Result<(), k::Failure> {
        let mut staged = 0;
        let mut registers = [[0; 6]; 2];
        let mut pending_failure = None;
        for _ in 0..200 {
            if staged < 2 && self.writes as usize == staged {
                let frame = self
                    .mask
                    .render_prepared(show, staged as u64 * 3000)
                    .unwrap();
                assert_eq!(frame.rendered.face_revision, 1);
                assert_eq!(
                    frame.rendered.disposition,
                    if staged == 0 {
                        MaskDisposition::Temperature { decimal_places: 1 }
                    } else {
                        MaskDisposition::Humidity
                    }
                );
                registers[staged] = frame.registers;
                assert_eq!(
                    self.output
                        .kernel
                        .admit_remote_input(
                            k::RemoteEndpointId(self.output.input.endpoint()),
                            k::CordId(self.output.input.cord()),
                            staged as u64,
                            frame.canonical
                        )
                        .unwrap(),
                    RemoteIngressOutcome::Accepted {
                        sequence: staged as u64
                    }
                );
                staged += 1;
                if staged == 2 {
                    self.output
                        .kernel
                        .close_remote_input(
                            k::RemoteEndpointId(self.output.input.endpoint()),
                            k::CordId(self.output.input.cord()),
                        )
                        .unwrap();
                }
            }
            let mut service = display_service::DisplayService {
                kernel: &mut self.output.kernel,
                selectors: &mut self.output.selectors,
                write: self.output.write,
                owner: &mut self.owner,
            };
            if let Some(pending) = service.take() {
                let request = pending.request();
                let writing = request.node == self.output.write;
                let report = service.service(pending, || self.cancel_output).unwrap();
                assert_eq!(report.request, request);
                if let Some(failure) = report.failure {
                    assert!(writing && self.cancel_output);
                    pending_failure = Some(failure);
                } else if writing {
                    self.writes += 1;
                }
            }
            match self.output.kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    assert_eq!(self.writes, 2);
                    assert_ne!(registers[0], registers[1]);
                    return Ok(());
                }
                Err(SchedulerError::BackFailed(failure)) => {
                    assert_eq!(pending_failure, Some(failure));
                    self.failure = Some(failure);
                    self.output.kernel.cancel().unwrap();
                    return Err(failure);
                }
                Err(error) => panic!("unexpected output failure: {error:?}"),
                _ => (),
            }
        }
        panic!("finite output did not terminate")
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn check<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    storage: native_kernel_storage::PreparedKernel,
    native: native_authority::NativeBindings<P, W>,
    source: &conduit_environment_make::PreparedNativeAppliance,
    samples: u64,
    period_ms: u64,
    cancel_output: bool,
    source_preparation: allocation_probe::Observation,
    retirement: impl FnOnce(crate::native_worker::RetiredProviders<P, W>),
) {
    let scope = native.pulse.inspection().scope.clone();
    let fragment = &source.plan().fragments[0];
    // Host Make's Source/Plan and immutable material are prepared outside the
    // observation. On the MCU these are linked material/assignment admission;
    // these CPU fixture descriptions are not a complete Root memory inventory.
    let display_source = DisplayOutputSource::compile().unwrap();
    let material = DisplaySourceMaterial::prepare(&display_source).unwrap();
    let (host, grant, software_model) = fixture::fixture_with_scope(
        &display_source,
        scope.host_id.as_str(),
        scope.boot_id.as_str(),
    );
    let display = PreparedDisplayOutput::prepare(
        &display_source,
        &host,
        &["fixture/base".into(), "conduit.base/local@1".into()],
        &[grant],
    )
    .unwrap();
    drop(software_model);
    let extent = display_source.maximum_value_bytes().unwrap();
    let signs = DisplaySignBudget::derive(
        2,
        display.lowered().sign_items,
        display.lowered().sign_bytes,
    )
    .unwrap();
    let ((mut execution, face, mask, output, owner, edges), additional) =
        allocation_probe::observe(|| {
            let mut software = material
                .prepare_software(scope.host_id.clone(), scope.boot_id.clone())
                .unwrap();
            let values = k::HostedValueStore::new(12, extent, 12 * extent).unwrap();
            let log = k::HostedSignLog::new_with_remote_storage(
                signs.ordinary_items(),
                signs.ordinary_bytes(),
                signs.remote_items(),
                signs.remote_bytes(),
            )
            .unwrap();
            // CPU Make material is included in preparation peak, then released.
            let plan = display.plan();
            let output_fragment = &plan.fragments[0];
            let generated = conduit_embedded_build::generate_fore_native_plan(
                plan,
                &output_fragment.fragment_id,
                conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING,
                conduit_embedded_build::EmbeddedComputationBounds {
                    maximum_program_bytes: 32 * 1024,
                    maximum_total_program_bytes: 96 * 1024,
                },
                2,
            )
            .unwrap();
            let maxima = c::AssignedPlanMaxima {
                encoded_bytes: 16 * 1024,
                runtime_state_bytes: u16::MAX,
                counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
            };
            let bytes = generated.encode(maxima).unwrap();
            let computations =
                conduit_embedded_build::assigned_computation_requirements(generated.kernel())
                    .unwrap();
            let calls: Vec<_> = generated
                .kernel()
                .host_calls
                .iter()
                .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
                .collect();
            let resources: Vec<_> = generated
                .kernel()
                .resources
                .iter()
                .map(|r| r.resource)
                .collect();
            let mut records = generated.layout_requirements(maxima).unwrap();
            records.extend(
                c::AssignedPlanRecords::new(&bytes)
                    .unwrap()
                    .map(Result::unwrap)
                    .filter(|r| r.tag == c::ASSIGNED_CONFIGURATION)
                    .map(c::AssignedRecordRequirement::from_record),
            );
            let canonical = c::AssignedPlanRecords::new(&bytes)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
            counts.copy_from_slice(&bytes[80..93]);
            let source = c::PlotIdentity {
                source_document_id: output_fragment.source_document_id.clone(),
                checked_plot_id: output_fragment.checked_plot_id.clone(),
                expanded_plot_id: output_fragment.expanded_plot_id.clone(),
            };
            let profile = AssignedDisplayProfile {
                source: &source,
                layout: ResidentLayout {
                    records: &records,
                    counts,
                },
                computations: c::AssignedComputationProfile {
                    maximum_text_bytes: u16::MAX,
                    maximum_total_text_bytes: u32::MAX,
                    requirements: &computations,
                },
                fore: generated.fore(),
                host_calls: &calls,
                resources: &resources,
                write: k::NodeId(
                    output_fragment
                        .placements
                        .iter()
                        .position(|p| p.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND)
                        .unwrap() as u16,
                ),
                maximum_value_bytes: extent,
                sign_items: display.lowered().sign_items,
                sign_bytes: display.lowered().sign_bytes,
            };
            software
                .current_offers(
                    &output_fragment.host_id,
                    &output_fragment.boot_id,
                    output_fragment.offer_generation,
                )
                .unwrap();
            let output = PreparedAssignedDisplayKernel::prepare(
                AssignedDisplayInput {
                    header: &bytes[..125],
                    records: &canonical,
                    maxima,
                },
                &profile,
                AssignedDisplayScope {
                    host: &output_fragment.host_id,
                    boot: &output_fragment.boot_id,
                    generation: output_fragment.offer_generation,
                },
                &mut software,
                2,
                values,
                log,
            )
            .unwrap();
            software.release_transferred_metadata().unwrap();
            let edges = Rc::new(Cell::new(0));
            let owner = output_owner::bind(&display, output_owner::Pins(edges.clone()));
            let face = LocalPublicationFace::new(
                RetainedPublicationFace::prepare(
                    FaceBasis {
                        body_id: None,
                        wake_id: None,
                        source_document_id: Some(fragment.source_document_id.clone()),
                        checked_plot_id: Some(fragment.checked_plot_id.clone()),
                        expanded_plot_id: Some(fragment.expanded_plot_id.clone()),
                        plan_id: Some(scope.plan_id.clone()),
                        active_play_id: Some(scope.active_play_id.clone()),
                        sign_ids: vec![],
                    },
                    samples.try_into().unwrap(),
                    period_ms * 2,
                )
                .unwrap(),
            );
            let profile = Tm1637Profile {
                digits: core::array::from_fn(|index| DigitWiring {
                    address: index as u8,
                    segment_bits: [0, 1, 2, 3, 4, 5, 6],
                }),
                decimal_points: [None; 4],
                colon: Some(WireBit { address: 1, bit: 7 }),
            };
            let mask = PreparedDisplayMask::prepare(
                profile,
                SevenSegmentLook::fahrenheit().with_colon_fraction(),
                3000,
                6000,
                3,
            )
            .unwrap();
            let execution =
                Execution::boxed_retained(&mut Some(storage), &mut Some(native)).unwrap();
            (execution, face, mask, output, owner, edges)
        });
    let (publisher, reader) = face.claim().unwrap();
    let mut sink = Sink {
        publisher,
        reader,
        mask,
        output,
        owner,
        period_ms,
        writes: 0,
        cancel_output,
        failure: None,
    };
    let (outcome, play) = allocation_probe::observe(|| {
        let mut future =
            core::pin::pin!(execution.run_with_sink(core::future::pending(), &mut sink));
        for _ in 0..samples * 2000 {
            if let Poll::Ready(value) = future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                return value;
            }
        }
        panic!("bounded joint fixture did not finish")
    });
    assert_eq!((play.allocations, play.reallocations), (0, 0));
    assert_eq!(outcome.cleanup, Ok(()));
    if cancel_output {
        assert_eq!(outcome.exit, Exit::Failed(sink.failure.unwrap()));
        assert_eq!(outcome.progress.publications, 0);
        assert_eq!(sink.writes, 0);
    } else {
        assert_eq!(outcome.exit, Exit::Drained);
        assert_eq!(outcome.progress.publications, samples);
        assert_eq!(sink.writes, 2);
        assert!(edges.get() > 0);
    }
    sink.owner
        .revoke(sink.output.write, k::HostCallId(0))
        .unwrap();
    let source_retained = source_preparation.retained_bytes();
    eprintln!("native_joint_display_cpu samples={samples} cancelled={cancel_output} source_peak={} source_retained={} additional_peak={} additional_retained={} source_first_execution_peak={} source_first_execution_retained={} writes={} allocations=0 mcu_admitted=false",
        source_preparation.peak_bytes,source_retained,additional.peak_bytes,additional.retained_bytes(),
        source_preparation.peak_bytes.max(source_retained+additional.peak_bytes),source_retained+additional.retained_bytes(),sink.writes);
    retirement(Execution::retire_retained(&mut Some(execution)).unwrap());
}
