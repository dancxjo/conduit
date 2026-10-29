//! Local Plans use the same prepared output kernel and finite backing. These
//! are CPU admission/execution proofs, never claims of native GPIO possession.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "display_support/fixture.rs"]
mod fixture;
#[path = "../../runtime/tests/support/verified_metadata.rs"]
mod verified_metadata;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_environment_make::{DisplayOutputSource, DisplaySourceMaterial, PreparedDisplayOutput};
use conduit_environment_runtime::display_output::*;
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
use conduit_plan_lowering::local_tables::VerifiedLocalLowering;

#[test]
fn local_output_refuses_before_transfer_then_executes_without_play_allocation() {
    for brightness in [3, 8] {
        let source = DisplayOutputSource::compile().unwrap();
        let material = DisplaySourceMaterial::prepare(&source).unwrap();
        let fixture = fixture::fixture(&source);
        let mut host = fixture.host.clone();
        let mut software = material
            .prepare_compact(host.host_id.clone(), host.boot_id.clone())
            .unwrap();
        host.capabilities
            .retain(|o| o.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND);
        host.capabilities.extend(
            software
                .current_offers(&host.host_id, &host.boot_id, host.offer_generation)
                .unwrap()
                .iter()
                .map(|bytes| serde_json::from_slice::<c::CapabilityOffer>(bytes).unwrap()),
        );
        let plan = PreparedDisplayOutput::prepare(
            &source,
            &host,
            &["conduit.base/local@1".into(), "fixture/base".into()],
            &[fixture.grant],
        )
        .unwrap();
        // SAFETY: the private material was produced from this checked Source.
        let image = unsafe {
            conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(
                material.planning_image(),
            )
        }
        .unwrap();
        let boundary = material.planning_boundary().unwrap();
        let scope = (&host.host_id, &host.boot_id, host.offer_generation);
        let frames = 2;
        let maximum = source.maximum_value_bytes().unwrap();
        let stores = |small: bool| {
            let signs = DisplaySignBudget::derive(
                frames,
                plan.lowered().sign_items,
                plan.lowered().sign_bytes,
            )
            .unwrap();
            (
                if small {
                    k::HostedValueStore::new(1, 1, 1).unwrap()
                } else {
                    DisplayValueBudget::derive(frames, maximum)
                        .unwrap()
                        .prepare()
                        .unwrap()
                },
                k::HostedSignLog::new_with_remote_storage(
                    signs.ordinary_items(),
                    signs.ordinary_bytes(),
                    signs.remote_items(),
                    signs.remote_bytes(),
                )
                .unwrap(),
            )
        };
        // Self-consistent changed Plans cannot borrow the original Source's
        // initialized software or silently redirect/rename its checked Fore.
        for change in 0..3 {
            let mut changed = plan.plan().clone();
            match change {
                0 => {
                    changed.fragments[0].placements[0].configuration[0].value =
                        c::ConfigurationValue::Text("changed".into())
                }
                1 => changed.fragments[0].fore_ports[0].front_port_id = "different-frame".into(),
                2 => changed.fragments[0].placements[0].artifact_id = "foreign/artifact".into(),
                _ => unreachable!(),
            }
            let changed = c::seal_plan_with_activation_entries(
                c::PlotIdentity {
                    source_document_id: changed.source_document_id,
                    checked_plot_id: changed.checked_plot_id,
                    expanded_plot_id: changed.expanded_plot_id,
                },
                changed.completion_policy,
                changed.realization_backs,
                changed.activations,
                changed.fragments,
            );
            assert!(c::verify_plan(&changed));
            let changed = verified_metadata::from_plan(&changed);
            let lowered = VerifiedLocalLowering::prepare(
                &changed,
                &changed.preparation_metadata().fragments[0].fragment_id,
            )
            .unwrap();
            let (values, signs) = stores(false);
            assert!(PreparedDisplayKernel::prepare_local(
                LocalDisplayInput {
                    verified: &lowered,
                    image: &image,
                    boundary: &boundary,
                    scope,
                    maximum_frames: frames,
                    maximum_value_bytes: maximum
                },
                &mut software,
                values,
                signs
            )
            .is_err());
            assert!(software.is_ready());
        }
        let verified = verified_metadata::from_plan(plan.plan());
        boundary.check_selected_fore(&verified).unwrap();
        let lowered = VerifiedLocalLowering::prepare(
            &verified,
            &verified.preparation_metadata().fragments[0].fragment_id,
        )
        .unwrap();
        for (small, boot, epoch) in [
            (true, &host.boot_id, host.offer_generation),
            (false, &c::BootId::from("stale/boot"), host.offer_generation),
            (
                false,
                &host.boot_id,
                c::OfferGeneration(host.offer_generation.0 + 1),
            ),
        ] {
            let (values, signs) = stores(small);
            assert!(PreparedDisplayKernel::prepare_local(
                LocalDisplayInput {
                    verified: &lowered,
                    image: &image,
                    boundary: &boundary,
                    scope: (&host.host_id, boot, epoch),
                    maximum_frames: frames,
                    maximum_value_bytes: maximum
                },
                &mut software,
                values,
                signs
            )
            .is_err());
            assert!(software.is_ready());
        }
        let (values, signs) = stores(false);
        let mut prepared = PreparedDisplayKernel::prepare_local(
            LocalDisplayInput {
                verified: &lowered,
                image: &image,
                boundary: &boundary,
                scope,
                maximum_frames: frames,
                maximum_value_bytes: maximum,
            },
            &mut software,
            values,
            signs,
        )
        .unwrap();
        assert!(!software.is_ready());
        assert!(software.check_local(&lowered, &image, scope).is_err());
        let offers = software
            .current_offers(&host.host_id, &host.boot_id, host.offer_generation)
            .unwrap();
        assert_eq!(offers.iter().count(), 0);
        let scope_bytes = offers.host().as_str().len()
            + offers.boot().as_str().len()
            + offers.source().as_str().len()
            + offers.checked().as_str().len()
            + offers.expanded().as_str().len();
        // After transfer, dropping the preparation owner frees only its scope
        // strings; the operation table has already been retired. The execution
        // below must remain complete without borrowing that construction owner.
        let (_, retired) = allocation_probe::observe(|| drop(software));
        assert!(retired.deallocated_bytes <= scope_bytes + 64, "{retired:?}");
        let mut encoder =
            conduit_environment_runtime::display_packets::PreparedDisplayFrame::prepare().unwrap();
        let frame = encoder
            .encode([6, 91, 79, 102, 0, 0], brightness, true)
            .unwrap();
        let mut packets =
            conduit_environment_runtime::display_packets::PreparedDisplayPackets::prepare()
                .unwrap();
        let expected = packets
            .encode_canonical([6, 91, 79, 102, 0, 0], brightness.min(7), true)
            .unwrap()
            .to_vec();
        let (writes, allocations) = allocation_probe::observe(|| {
            let input = &prepared.input;
            input.validate_value(frame).unwrap();
            prepared
                .kernel
                .admit_remote_input(input.endpoint, input.cord, 0, frame)
                .unwrap();
            let mut submitted = 1;
            let mut closed = false;
            let mut writes = 0;
            for _ in 0..256 {
                let status = prepared.kernel.step().unwrap();
                if submitted < frames {
                    if let k::scheduler::RemoteIngressOutcome::Accepted { .. } = prepared
                        .kernel
                        .admit_remote_input(input.endpoint, input.cord, u64::from(submitted), frame)
                        .unwrap()
                    {
                        submitted += 1;
                    }
                }
                if submitted == frames && !closed {
                    prepared
                        .kernel
                        .close_remote_input(input.endpoint, input.cord)
                        .unwrap();
                    closed = true;
                }
                if let Some(request) = prepared.kernel.next_host_request() {
                    let bytes = prepared.kernel.host_value(request.input.value).unwrap();
                    let output = if request.node == prepared.write {
                        assert_eq!(bytes, expected.as_slice());
                        writes += 1;
                        None
                    } else {
                        prepared.selectors[usize::from(request.node.0)]
                            .as_mut()
                            .unwrap()
                            .select(bytes)
                            .unwrap()
                            .map(|bytes| {
                                k::BoundedValueRef::new(
                                    prepared.kernel.store_host_value(bytes).unwrap(),
                                    1024,
                                )
                                .unwrap()
                            })
                    };
                    prepared
                        .kernel
                        .complete_host_call(
                            request.node,
                            request.request,
                            k::HostCallOutcome {
                                disposition: k::HostCallDisposition::Completed,
                                output,
                                failure: None,
                            },
                        )
                        .unwrap();
                }
                let refusal = &prepared.refusal;
                if let Some(offer) = prepared
                    .kernel
                    .remote_egress_offer(refusal.endpoint, refusal.cord)
                    .unwrap()
                {
                    prepared
                        .kernel
                        .remote_egress_accept(refusal.endpoint, refusal.cord, offer.sequence)
                        .unwrap();
                    prepared
                        .kernel
                        .remote_egress_delivered(refusal.endpoint, refusal.cord, offer.sequence)
                        .unwrap();
                }
                if status == SchedulerStatus::Drained {
                    return writes;
                }
            }
            panic!("finite local output failed to drain")
        });
        assert_eq!(writes, if brightness <= 7 { frames } else { 0 });
        assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    }
}
