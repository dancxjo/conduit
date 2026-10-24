//! Real checked output Plan -> compact local Fore -> ordinary kernel execution.
//! Root/pins/authority remain the same explicit CPU fixtures as execution tests.
use super::execution::run_output_mode;
use conduit_core as c;
use conduit_embedded_build as build;
use conduit_environment_runtime::{assigned::*, display_output::*};
use conduit_kernel as k;
#[test]
fn checked_output_runs_from_compact_fore_tables_without_inventing_a_line() {
    for mode in 0..3 {
        run_output_mode(mode, true, |prepared, software| {
            let plan = prepared.plan();
            let fragment = &plan.fragments[0];
            let generated = build::generate_fore_native_plan(
                plan,
                &fragment.fragment_id,
                build::EmbeddedImageBounds::HOST_TOOLING,
                build::EmbeddedComputationBounds {
                    maximum_program_bytes: 32 * 1024,
                    maximum_total_program_bytes: 96 * 1024,
                },
                2,
            )
            .unwrap();
            assert!(matches!(
                build::generate_embedded_plan_with_computations(
                    fragment,
                    prepared.lowered(),
                    build::EmbeddedImageBounds::HOST_TOOLING,
                    build::EmbeddedComputationBounds {
                        maximum_program_bytes: 32 * 1024,
                        maximum_total_program_bytes: 96 * 1024
                    }
                ),
                Err(build::GenerationError::Unsupported(
                    build::UnsupportedPlanFeature::LocalFore
                ))
            ));
            assert!(matches!(
                build::generate_fore_native_plan(
                    plan,
                    &fragment.fragment_id,
                    build::EmbeddedImageBounds::HOST_TOOLING,
                    build::EmbeddedComputationBounds {
                        maximum_program_bytes: 32 * 1024,
                        maximum_total_program_bytes: 96 * 1024
                    },
                    1
                ),
                Err(build::GenerationError::BoundExceeded {
                    table: "local Fore ports",
                    ..
                })
            ));
            assert_eq!(generated.fore().len(), 2);
            assert!(generated.kernel().remote_endpoints.is_empty());
            let maxima = c::AssignedPlanMaxima {
                encoded_bytes: 16 * 1024,
                runtime_state_bytes: u16::MAX,
                counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
            };
            let bytes = generated.encode(maxima).unwrap();
            assert!(build::encode_assigned_native_plan(generated.kernel(), maxima).is_err());
            let computations =
                build::assigned_computation_requirements(generated.kernel()).unwrap();
            let profile = c::AssignedComputationProfile {
                maximum_text_bytes: u16::MAX,
                maximum_total_text_bytes: u32::MAX,
                requirements: &computations,
            };
            let calls: Vec<_> = generated
                .kernel()
                .host_calls
                .iter()
                .map(|c| c::AssignedIdentity::from_text(&c.contract_id))
                .collect();
            let resources: Vec<_> = generated
                .kernel()
                .resources
                .iter()
                .map(|r| r.resource)
                .collect();
            let required = c::AssignedPlanRequirements {
                host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
                boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
                host_calls: &calls,
                resources: &resources,
                remote_bindings: &[],
            };
            let mut records = generated.layout_requirements(maxima).unwrap();
            records.extend(
                c::AssignedPlanRecords::new(&bytes)
                    .unwrap()
                    .map(Result::unwrap)
                    .filter(|r| r.tag == c::ASSIGNED_CONFIGURATION)
                    .map(c::AssignedRecordRequirement::from_record),
            );
            let mut counts = [0; c::ASSIGNED_PLAN_COUNT_KINDS];
            counts.copy_from_slice(&bytes[80..93]);
            let layout = ResidentLayout {
                records: &records,
                counts,
            };
            type Tables = AssignedKernelTables<4, 5, 1, 5, 5, 4>;
            assert!(Tables::prepare(&bytes, maxima, required, profile, &layout).is_err());
            let (tables, memory) = crate::allocation_probe::observe(|| {
                Tables::prepare_with_fore(
                    &bytes,
                    maxima,
                    required,
                    profile,
                    &layout,
                    generated.fore(),
                )
                .unwrap()
            });
            assert_eq!((memory.allocations, memory.reallocations), (0, 0));
            for (actual, expected) in tables.cords.iter().zip(&prepared.lowered().cords) {
                assert_eq!(actual, &expected.spec);
            }
            let source = c::PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            };
            let canonical = c::AssignedPlanRecords::new(&bytes)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            let write = k::NodeId(
                fragment
                    .placements
                    .iter()
                    .position(|p| p.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND)
                    .unwrap() as u16,
            );
            let mut resident = AssignedDisplayProfile {
                source: &source,
                layout,
                computations: profile,
                fore: generated.fore(),
                host_calls: &calls,
                resources: &resources,
                write,
                sign_items: prepared.lowered().sign_items,
                sign_bytes: prepared.lowered().sign_bytes,
            };
            let make_input = || AssignedDisplayInput {
                header: &bytes[..125],
                records: &canonical,
                maxima,
            };
            let scope = |generation| AssignedDisplayScope {
                host: &fragment.host_id,
                boot: &fragment.boot_id,
                generation,
            };
            let values = || k::HostedValueStore::new(12, 1024, 12 * 1024).unwrap();
            let signs = || {
                k::HostedSignLog::new_with_remote_storage(
                    256,
                    256 * core::mem::size_of::<k::KernelEvent>() as u32,
                    256,
                    k::remote_sign_storage_bytes(256).unwrap(),
                )
                .unwrap()
            };
            assert!(matches!(
                PreparedAssignedDisplayKernel::prepare(
                    make_input(),
                    &resident,
                    scope(c::OfferGeneration(fragment.offer_generation.0 + 1)),
                    software,
                    2,
                    values(),
                    signs()
                ),
                Err(AssignedDisplayRefusal::Software(_))
            ));
            let mut foreign = source.clone();
            foreign.checked_plot_id = "fixture/foreign-source".into();
            resident.source = &foreign;
            assert!(matches!(
                PreparedAssignedDisplayKernel::prepare(
                    make_input(),
                    &resident,
                    scope(fragment.offer_generation),
                    software,
                    2,
                    values(),
                    signs()
                ),
                Err(AssignedDisplayRefusal::Software(_))
            ));
            resident.source = &source;
            assert!(matches!(
                PreparedAssignedDisplayKernel::prepare(
                    make_input(),
                    &resident,
                    scope(fragment.offer_generation),
                    software,
                    0,
                    values(),
                    signs()
                ),
                Err(AssignedDisplayRefusal::Bounds)
            ));
            let mut occupied = values();
            k::ValueStorage::store(&mut occupied, &[1]).unwrap();
            assert!(matches!(
                PreparedAssignedDisplayKernel::prepare(
                    make_input(),
                    &resident,
                    scope(fragment.offer_generation),
                    software,
                    2,
                    occupied,
                    signs()
                ),
                Err(AssignedDisplayRefusal::Bounds)
            ));
            let foreign_boot: c::BootId = "fixture/foreign-boot".into();
            assert!(matches!(
                PreparedAssignedDisplayKernel::prepare(
                    make_input(),
                    &resident,
                    AssignedDisplayScope {
                        host: &fragment.host_id,
                        boot: &foreign_boot,
                        generation: fragment.offer_generation
                    },
                    software,
                    2,
                    values(),
                    signs()
                ),
                Err(AssignedDisplayRefusal::Assignment(_))
            ));
            let execution = PreparedAssignedDisplayKernel::prepare(
                make_input(),
                &resident,
                scope(fragment.offer_generation),
                software,
                2,
                values(),
                signs(),
            )
            .unwrap();
            assert!(matches!(
                PreparedAssignedDisplayKernel::prepare(
                    make_input(),
                    &resident,
                    scope(fragment.offer_generation),
                    software,
                    2,
                    values(),
                    signs()
                ),
                Err(AssignedDisplayRefusal::Software(conduit_environment_runtime::appliance::software::SoftwareRefusal::AlreadySelected))
            ));
            assert_eq!(
                execution.input,
                *generated
                    .fore()
                    .iter()
                    .find(|f| f.direction() == c::AssignedForeDirection::Input)
                    .unwrap()
            );
            assert_eq!(
                execution.refusal,
                *generated
                    .fore()
                    .iter()
                    .find(|f| f.direction() == c::AssignedForeDirection::Output)
                    .unwrap()
            );
            let fore = |name: &str| {
                prepared
                    .lowered()
                    .fore_ports
                    .iter()
                    .find(|f| f.front_port_id.as_str() == name)
                    .unwrap()
                    .clone()
            };
            PreparedDisplayKernel {
                kernel: execution.kernel,
                selectors: execution.selectors,
                write: execution.write,
                input: fore("frame"),
                refusal: fore("invalid_brightness"),
            }
        });
    }
}
