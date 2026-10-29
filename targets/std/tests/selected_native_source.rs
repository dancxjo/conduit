//! Generic CPU-only Source admission through the public installed Host.
//! No IO, Body membership, observation protocol or product provider is installed.
use conduit_core::*;
use conduit_planner::*;
use conduit_plot::*;
use conduit_std_host::*;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

const MAXIMUM_BYTES: usize = 16;
const SOURCE_KIND: &str = "test/read";
const CALL: &str = "test/read@1";
const RESOURCE: &str = "test/session@1";
const AUTHORITY: &str = "test/read-authority@1";
const VALUE: &[u8] = b"actual source";
struct ImmediateReading {
    calls: Arc<AtomicUsize>,
}
// SAFETY: This fixture owns only its counter and copies fixed bytes into the
// admitted result slot. No IO, allocation, pending work or borrow escapes read.
unsafe impl ImmediateNativeSource for ImmediateReading {
    fn read(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, String> {
        assert!(input.is_empty());
        self.calls.fetch_add(1, Ordering::Relaxed);
        output[..VALUE.len()].copy_from_slice(VALUE);
        Ok(VALUE.len())
    }
}
fn source_kind() -> Kind {
    Kind {
        kind_id: SOURCE_KIND.into(),
        kind_contract_revision: "test/read@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: vec![PortDescriptor {
            port_id: "request".into(),
            value_kind: EMPTY_INFO_ID.into(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: "observation".into(),
            value_kind: TEXT_INFO_ID.into(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: None,
        }],
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_BYTES as u32,
        },
    }
}
fn definition(host: &HostAdvertisement) -> NativeSourceDefinition {
    let kind = source_kind();
    let offer = CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: "test/source-source".into(),
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        inputs: kind.inputs.clone(),
        outputs: kind.outputs.clone(),
        semantic_contract: kind.semantic_contract(),
        implementation: ImplementationOffer {
            execution_profile_id: "test/native-immediate".into(),
            implementation_id: "test/source-source".into(),
            artifact_id: "test/source-source-cpu-v1".into(),
        },
        state_retention: None,
        realization_properties: vec![],
        host_calls: vec![HostCallRequirement {
            contract_id: CALL.into(),
            target_kind: Some(kind.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: MAXIMUM_BYTES as u32,
        }],
        resource_requirements: vec![ResourceRequirement {
            class_id: RESOURCE.into(),
            units: 1,
            protected_role: None,
            compute: None,
            content: None,
        }],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AUTHORITY.into(),
            host_call_contract_id: CALL.into(),
            subject_kind: kind.kind_id.clone(),
        }],
        limits: kind.limits.clone(),
    };
    let grant = AuthorityGrant {
        grant_id: "test/source-read-grant".into(),
        contract_id: AUTHORITY.into(),
        host_call_contract_id: CALL.into(),
        subject_kind: kind.kind_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    NativeSourceDefinition {
        kind,
        base: BaseProviderEntry {
            base_id: "test/source-base".into(),
            provider_instance_id: "test/source-session".into(),
            provider_generation: 1,
            implementation_id: "test/independent-base-provider".into(),
            mechanism_family: "test/native-source".into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![ResourceOffer {
                pool_id: "test/source-session".into(),
                class_id: RESOURCE.into(),
                capacity_units: 1,
                compute: None,
                content: None,
            }],
        },
        grant,
        resource_generation: ResourceGenerationId("test/source-session-generation-1".into()),
        configuration: vec![],
        input_bytes: 1,
        output_bytes: MAXIMUM_BYTES as u32,
        resource_bytes: MAXIMUM_BYTES as u64,
    }
}
fn prepared() -> (StdHost, Plan, Arc<AtomicUsize>) {
    let kind = source_kind();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    startup
        .insert(KindSignature {
            kind: SOURCE_KIND.into(),
            startup_parameters: vec![],
        })
        .unwrap();
    startup
        .insert_fore(SOURCE_KIND, kind.checked_front())
        .unwrap();
    profile.insert_kind(kind).unwrap();
    let text=format!("plot capture (\n >> begin: Empty...|\n reading: Text...| >>\n) {{\n sensor: {}\n begin >> sensor.request\n sensor.observation >> reading\n}}\n",SOURCE_KIND);
    let checked = check_syntax_document(&parse_syntax_document(&text), &startup).unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "capture", &profile).unwrap();
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: "test/producing-host".into(),
        boot_id: "test/actual-host-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let definition = definition(host.advertisement());
    let grant = definition.grant.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    // SAFETY: Independent definition and exact immediate CPU owner precede Plan.
    unsafe {
        host.install_immediate_native_source(
            definition,
            Box::new(ImmediateReading {
                calls: calls.clone(),
            }),
        )
    }
    .unwrap_or_else(|(_, _, reason)| panic!("{reason}"));
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "begin"),
        (PortDirection::Output, "reading"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: if direction == PortDirection::Input {
                    1
                } else {
                    MAXIMUM_BYTES as u32
                },
            },
        )
    })
    .collect();
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let plan = plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty,
            line_candidates: &lines,
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_BYTES as u32,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    (host, plan, calls)
}
struct Capture {
    expected: PlanFragment,
    calls: usize,
}
impl ExternalForeOutputAdapter for Capture {
    fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
        panic!("requires real kernel commit")
    }
    fn deliver_committed(
        &mut self,
        witness: CommittedForeValue<'_>,
        _: ExternalForeDelivery,
    ) -> Result<(), String> {
        assert_eq!(witness.scope().fragment, &self.expected);
        assert_eq!(witness.sequence(), 0);
        assert_eq!(witness.scope().play.host_id, self.expected.host_id);
        assert_eq!(witness.bytes(), VALUE);
        self.calls += 1;
        Ok(())
    }
}
#[test]
fn checked_source_uses_installed_registry_grant_kernel_cord_and_commit() {
    let (mut host, plan, calls) = prepared();
    let fragment = plan.fragments[0].clone();
    let mut capture = Capture {
        expected: fragment.clone(),
        calls: 0,
    };
    let report = host
        .run_external_plot_to(
            fragment,
            &[ExternalForeInput {
                front_port_id: "begin".into(),
                track: ConnectionTrack::Payload,
                bytes: vec![],
            }],
            &mut capture,
            &mut Vec::new(),
            &mut ThreadTimer,
        )
        .unwrap();
    assert_eq!(capture.calls, 1);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(report.external_fore_deliveries.len(), 1);
}

#[test]
fn a_plan_cannot_supply_its_own_grant_executable_boot_provider_or_resource() {
    let (_, plan, _) = prepared();
    for change in 0..5 {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: "test/producing-host".into(),
            boot_id: if change == 3 {
                "test/different-current-boot"
            } else {
                "test/actual-host-boot"
            }
            .into(),
            offer_generation: OfferGeneration(1),
        });
        let mut selected = definition(host.advertisement());
        match change {
            0 => selected.grant.grant_id = "test/independent-other-grant".into(),
            1 => {
                selected.base.capabilities[0].implementation.artifact_id =
                    "test/different-executable".into()
            }
            2 => selected.base.provider_generation = 2,
            3 => {}
            _ => {
                selected.base.resources[0].pool_id =
                    "test/different-current-session-resource".into()
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        unsafe {
            host.install_immediate_native_source(
                selected,
                Box::new(ImmediateReading {
                    calls: calls.clone(),
                }),
            )
        }
        .unwrap_or_else(|(_, _, reason)| panic!("{reason}"));
        let fragment = plan.fragments[0].clone();
        let mut capture = Capture {
            expected: fragment.clone(),
            calls: 0,
        };
        assert!(
            host.run_external_plot_to(
                fragment,
                &[ExternalForeInput {
                    front_port_id: "begin".into(),
                    track: ConnectionTrack::Payload,
                    bytes: vec![]
                }],
                &mut capture,
                &mut Vec::new(),
                &mut ThreadTimer
            )
            .is_err(),
            "case {change}"
        );
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(capture.calls, 0);
    }
}

#[test]
fn installation_refusal_returns_the_owner_without_replacing_live_continuity() {
    let (mut host, _, original_calls) = prepared();
    let calls = Arc::new(AtomicUsize::new(0));
    let result = unsafe {
        host.install_immediate_native_source(
            definition(host.advertisement()),
            Box::new(ImmediateReading {
                calls: calls.clone(),
            }),
        )
    };
    let (_, mut returned, _) =
        result.expect_err("one retained Source slot cannot be overwritten");
    let mut buffer = [0; 128];
    assert_eq!(returned.read(&[], &mut buffer).unwrap(), VALUE.len());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(original_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn cancelled_native_source_is_not_invoked_or_committed() {
    let (mut host, plan, calls) = prepared();
    let fragment = plan.fragments[0].clone();
    let mut capture = Capture {
        expected: fragment.clone(),
        calls: 0,
    };
    let control = RunControl::default();
    control
        .request_stop(RunControlRequestId::new("before-native-read").unwrap())
        .unwrap();
    host.run_external_plot_controlled_to(
        fragment,
        &[ExternalForeInput {
            front_port_id: "begin".into(),
            track: ConnectionTrack::Payload,
            bytes: vec![],
        }],
        &mut capture,
        &mut Vec::new(),
        &mut ThreadTimer,
        &control,
    )
    .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(capture.calls, 0);
}

struct FaultyImmediate {
    oversize: bool,
    calls: Arc<AtomicUsize>,
}
// SAFETY: The fixture returns immediately without allocation, IO, background
// work or borrowed state. Reporting a refused result is not a memory write.
unsafe impl ImmediateNativeSource for FaultyImmediate {
    fn read(&mut self, _: &[u8], output: &mut [u8]) -> Result<usize, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.oversize {
            Ok(output.len() + 1)
        } else {
            Err(String::new())
        }
    }
}
#[test]
fn failed_or_oversize_return_retires_without_a_value_and_releases_the_reservation() {
    let (_, plan, _) = prepared();
    for oversize in [false, true] {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: "test/producing-host".into(),
            boot_id: "test/actual-host-boot".into(),
            offer_generation: OfferGeneration(1),
        });
        let calls = Arc::new(AtomicUsize::new(0));
        // SAFETY: Actual immediate fixture, independent definition/grant and
        // fixed bounded result backing are installed before receiving a Plan.
        unsafe {
            host.install_immediate_native_source(
                definition(host.advertisement()),
                Box::new(FaultyImmediate {
                    oversize,
                    calls: calls.clone(),
                }),
            )
        }
        .unwrap_or_else(|(_, _, reason)| panic!("{reason}"));
        for completed in 1..=2 {
            let fragment = plan.fragments[0].clone();
            let mut capture = Capture {
                expected: fragment.clone(),
                calls: 0,
            };
            assert!(host
                .run_external_plot_to(
                    fragment,
                    &[ExternalForeInput {
                        front_port_id: "begin".into(),
                        track: ConnectionTrack::Payload,
                        bytes: vec![]
                    }],
                    &mut capture,
                    &mut Vec::new(),
                    &mut ThreadTimer
                )
                .is_err());
            assert_eq!(capture.calls, 0);
            // A second real admission and invocation requires the first exact
            // session resource reservation to have been released, without
            // replacing the provider or treating its first result as a commit.
            assert_eq!(calls.load(Ordering::Relaxed), completed);
        }
    }
}
