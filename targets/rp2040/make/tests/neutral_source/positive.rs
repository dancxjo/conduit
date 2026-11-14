//! CPU preparation aid using the exact installed recipe and original Root facts.
//! This only constructs and authenticates input. It grants no native possession,
//! binds no provider, admits no resources, and never Starts a Play.
use crate::neutral_workset::layout;
use conduit_body as b;
use conduit_core as c;
use conduit_mcu_host_runtime::root::Root;
use ed25519_dalek::{Signer, SigningKey};

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Create the existing canonical workset document against the exact original
/// inventory. Every immutable Base/pool/implementation pin must already match;
/// this helper never rewrites the template or creates a resource advertisement.
fn payload(
    context: b::NativePreparationContext<'_>,
    issuers: &crate::neutral_authority::Issuers,
) -> Vec<u8> {
    let actual = context.actual;
    let id = c::AssignedIdentity::from_text;
    let grants = [issuers.bus.grant().unwrap(), issuers.clock.grant().unwrap()];
    let mut native = Vec::new();
    let mut native_text = Vec::new();
    for record in c::AssignedPlanRecords::new(layout::PROJECTION_TEMPLATE).unwrap() {
        let record = record.unwrap();
        if record.tag != c::ASSIGNED_HOST_CALL {
            continue;
        }
        let Some(mut binding) = c::assigned_native_binding(record.payload).unwrap() else {
            continue;
        };
        let base = actual
            .bases
            .iter()
            .find(|p| id(p.base_id.as_str()) == binding.base)
            .expect("original inventory must have the installed Base identity");
        assert_eq!(
            id(base.implementation_id.as_str()),
            binding.base_implementation
        );
        assert_eq!(id(base.mechanism_family.as_str()), binding.mechanism);
        assert_eq!(base.lifecycle, c::BaseLifecycle::Ready);
        let mut selected = grants.iter().copied().filter(|g| {
            id(g.contract_id.as_str()) == binding.authority
                && id(g.host_call_contract_id.as_str()) == binding.call
                && id(g.subject_kind.as_str()) == binding.subject
                && id(g.capability_id.as_str()) == binding.authority_capability
        });
        let grant = selected
            .next()
            .expect("original Issuer grant for exact operation");
        assert!(selected.next().is_none());
        assert_eq!(grant.host_id, actual.host_id);
        assert_eq!(grant.boot_id, actual.boot_id);
        assert!(base.capability_ids.contains(&grant.capability_id));
        let node = u16::from_le_bytes(record.payload[..2].try_into().unwrap());
        let call = u16::from_le_bytes(record.payload[2..4].try_into().unwrap());
        let pool = layout::CONTINUATION_RESOURCES
            .iter()
            .filter(|r| r.0 == node && id(r.4) == binding.resource_class)
            .map(|r| r.3)
            .next()
            .expect("exact native resource pin");
        assert!(base.resource_pool_ids.iter().any(|p| p.as_str() == pool));
        binding.instance = id(base.provider_instance_id.as_str());
        binding.generation = base.provider_generation;
        binding.grant = id(grant.grant_id.as_str());
        binding.host = id(actual.host_id.as_str());
        binding.boot = id(actual.boot_id.as_str());
        binding.pool = id(pool);
        native.push(c::FreshAssignedNative {
            node,
            call,
            contract: c::AssignedIdentity(record.payload[4..20].try_into().unwrap()),
            binding,
        });
        native_text.push((
            node,
            call,
            base.provider_instance_id.as_str(),
            base.provider_generation,
            grant.grant_id.as_str(),
        ));
    }
    let mut resources = Vec::new();
    for &(node, ordinal, placement, pool, class, units) in layout::CONTINUATION_RESOURCES {
        let mut selected = actual
            .resources
            .iter()
            .filter(|r| r.pool_id.as_str() == pool);
        let held = selected
            .next()
            .expect("original inventory must own each installed resource pool");
        assert!(selected.next().is_none());
        assert_eq!(held.class_id.as_str(), class);
        assert!(units != 0 && units <= held.capacity_units);
        assert!(held.compute.is_none() && held.content.is_none());
        let placement_ordinal = layout::FRAGMENT_SEAL_TO_KERNEL
            .iter()
            .position(|n| *n == node)
            .unwrap();
        assert_eq!(layout::FRAGMENT_PLACEMENT_IDS[placement_ordinal], placement);
        c::fragment_seal_resource_matches(
            layout::FRAGMENT_SEAL_RECIPE,
            placement_ordinal.try_into().unwrap(),
            ordinal,
            held,
            units,
        )
        .unwrap();
        b::PlainFragmentResource {
            node,
            resource: ordinal,
            placement,
            pool,
            class,
            units,
        }
        .append_to(&mut resources)
        .unwrap();
    }
    use c::{FragmentScopeRole as R, FragmentSealValue as V};
    let mut bindings = Vec::new();
    for piece in layout::FRAGMENT_SEAL_RECIPE {
        let c::FragmentSealPiece::Field(field, _) = piece else {
            continue;
        };
        if bindings
            .iter()
            .any(|b: &c::FragmentSealBinding<'_>| b.field == *field)
        {
            continue;
        }
        let value = match field.placement {
            None => match field.role {
                R::Host => V::Text(actual.host_id.as_str()),
                R::Boot => V::Text(actual.boot_id.as_str()),
                R::OfferGeneration => V::U64(actual.offer_generation.0),
                _ => panic!("unsupported header field in checked local recipe"),
            },
            Some(placement) => {
                let ordinal = usize::from(placement);
                let node = layout::FRAGMENT_SEAL_TO_KERNEL[ordinal];
                let native_owner = || {
                    let mut rows = native_text.iter().filter(|r| r.0 == node);
                    let first = rows
                        .next()
                        .expect("original native provider for recipe field");
                    assert!(rows.all(|r| r.2 == first.2 && r.3 == first.3));
                    first
                };
                match field.role {
                    R::PlacementId => V::Text(layout::FRAGMENT_PLACEMENT_IDS[ordinal]),
                    R::Host | R::AuthorityHost => V::Text(actual.host_id.as_str()),
                    R::Boot | R::AuthorityBoot => V::Text(actual.boot_id.as_str()),
                    R::OfferGeneration => V::U64(actual.offer_generation.0),
                    R::ProviderInstance => V::Text(native_owner().2),
                    R::ProviderGeneration => V::U64(native_owner().3),
                    R::AuthorityGrant => {
                        let expected =
                            b::continuation_recipe_authority(layout::FRAGMENT_SEAL_RECIPE, *field)
                                .unwrap();
                        let mut rows = native.iter().filter(|n| {
                            n.node == node
                                && n.binding.authority == expected.authority
                                && n.binding.call == expected.call
                                && n.binding.subject == expected.subject
                                && n.binding.authority_capability == expected.capability
                        });
                        let row = rows.next().unwrap();
                        assert!(rows.next().is_none());
                        let text = native_text
                            .iter()
                            .find(|r| r.0 == node && r.1 == row.call)
                            .unwrap();
                        V::Text(text.4)
                    }
                    R::ResourcePool => V::Text(
                        layout::CONTINUATION_RESOURCES
                            .iter()
                            .find(|r| r.0 == node && r.1 == field.entry)
                            .unwrap()
                            .3,
                    ),
                    _ => panic!("unsupported field in exact installed recipe"),
                }
            }
        };
        bindings.push(c::FragmentSealBinding {
            field: *field,
            value,
        });
    }
    let fragment =
        hex(c::fragment_seal_recipe_digest(layout::FRAGMENT_SEAL_RECIPE, &bindings).unwrap());
    let plot = hex(c::PlainPlotPlanSeal {
        source_document_id: layout::SOURCE_DOCUMENT_ID,
        checked_plot_id: layout::CHECKED_PLOT_ID,
        expanded_plot_id: layout::EXPANDED_PLOT_ID,
    }
    .digest(&[c::PlotFragmentSeal {
        host_id: actual.host_id.as_str(),
        fragment_id: &fragment,
    }])
    .unwrap());
    let wake = "cpu/wake";
    let body_plan = format!(
        "body-plan/sha256:{}",
        hex(b::body_plan_commitment_digest(
            context.member.body_id.as_str(),
            wake,
            1,
            &[b::BodyPlotSeal {
                source_document_id: layout::SOURCE_DOCUMENT_ID,
                checked_plot_id: layout::CHECKED_PLOT_ID,
                plan_id: &plot
            }],
            &[],
            None
        )
        .unwrap())
    );
    let body_play = format!(
        "body-play/sha256:{}",
        hex(b::body_play_digest(
            context.member.body_id.as_str(),
            wake,
            &body_plan,
            1
        ))
    );
    let mut projection = vec![0; layout::PROJECTION_TEMPLATE.len()];
    let n = c::AssignedLocalProjectionTemplate::check(
        layout::PROJECTION_TEMPLATE,
        &layout::PROJECTION_TEMPLATE_DIGEST,
        layout::MAXIMA,
    )
    .unwrap()
    .instantiate(
        c::FreshAssignedHeader {
            plan: id(&plot),
            fragment: id(&fragment),
            host: id(actual.host_id.as_str()),
            boot: id(actual.boot_id.as_str()),
        },
        &native,
        layout::FRESH_FORE,
        &mut projection,
    )
    .unwrap();
    projection.truncate(n);
    let pinned_fore = layout::fore();
    let profile = crate::neutral_workset::profile(actual, &pinned_fore);
    let validation = c::decode_assigned_plan_with_fore(
        &projection,
        profile.maxima,
        profile.requirements,
        profile.computations,
        &pinned_fore,
    );
    println!("cpu_original_projection_resource_requirements={} cpu_original_projection_host_call_requirements={} cpu_original_projection_remote_requirements={} cpu_original_projection_validation={:?}",
        profile.requirements.resources.len(), profile.requirements.host_calls.len(),
        profile.requirements.remote_bindings.len(), validation.map(|_| "validated"));
    let mut fore = Vec::new();
    for record in c::AssignedPlanRecords::new(&projection).unwrap() {
        let record = record.unwrap();
        if record.tag == c::ASSIGNED_LOCAL_FORE {
            fore.extend_from_slice(record.payload);
        }
    }
    let (local_fragment_count, local_fragment_digest) =
        b::local_fragment_set_commitment(&[&fragment]).unwrap();
    // Reference only the independently emitted exact plain table. The receiver
    // must resolve this through its installed profile before normal admission.
    assert_eq!(
        resources.as_slice(),
        layout::INSTALLED_PLAIN_RESOURCE_BINDINGS.unwrap()
    );
    let resource_reference = b::plain_resource_table_reference(&resources).unwrap();
    let metadata = b::BodyFragmentApprovalMetadata {
        schema: 4,
        body: context.member.body_id.as_str(),
        wake,
        body_plan: &body_plan,
        body_play: &body_play,
        play_sequence: 1,
        workload_revision: 1,
        source: layout::SOURCE_DOCUMENT_ID,
        checked_plot: layout::CHECKED_PLOT_ID,
        expanded_plot: layout::EXPANDED_PLOT_ID,
        plot_plan: &plot,
        fragment: &fragment,
        host: actual.host_id.as_str(),
        boot: actual.boot_id.as_str(),
        offer_generation: actual.offer_generation.0,
        local_fragment_count,
        local_fragment_digest,
        resource_bindings: &resource_reference,
        fore_ports: &fore,
        projection_length: projection.len().try_into().unwrap(),
        projection_digest: c::assigned_plan_payload_digest(&projection),
    };
    // Independently funded CPU-only diagnostic encoding. Its larger scratch
    // measures the complete existing document; it is never retained by Root or
    // accepted under an enlarged policy. The installed-limit encoder below must
    // still succeed before any authenticated command can be retained.
    let diagnostic = b::encode_body_fragment_document(&metadata, &projection, 65_535).unwrap();
    let header = diagnostic.len() - projection.len();
    drop(diagnostic);
    println!("cpu_signed_assignment_projection_bytes={} cpu_signed_assignment_metadata_bytes={header} cpu_signed_assignment_workset_bytes={} installed_assignment_bytes={} resource_reference_bytes={} installed_resource_bytes={} fore_metadata_bytes={} no_limit_change",
        projection.len(), header + projection.len() + 14, crate::config::MAX_ASSIGNMENT_BYTES,
        resource_reference.len(), resources.len(), fore.len());
    let document = b::encode_body_fragment_document(
        &metadata,
        &projection,
        crate::config::MAX_ASSIGNMENT_BYTES.try_into().unwrap(),
    )
    .unwrap();
    b::encode_body_workset_document(
        &[&document],
        crate::config::MAX_ASSIGNMENT_BYTES.try_into().unwrap(),
    )
    .unwrap()
}

/// Retain a legitimately signed online command in Root's ORIGINAL input storage.
/// The test signer must be the independently pinned fixture owner. This entrance
/// deliberately leaves resources unadmitted and providers untouched.
pub fn retain<const N: usize>(
    root: &mut Root<'_, N>,
    issuers: &crate::neutral_authority::Issuers,
    signer: &SigningKey,
    now: u64,
) {
    let parts = root.execution_parts().unwrap();
    let actual = parts.resources.host();
    let context = b::NativePreparationContext {
        bootstrap: parts.bootstrap,
        member: parts.member,
        actual,
        image_id: parts.configuration.image_id,
        configuration_id: parts.configuration.configuration_id,
        now_millis: now,
    };
    assert_eq!(
        parts.bootstrap.owner.verifying_key,
        signer.verifying_key().to_bytes()
    );
    let policy = parts
        .configuration
        .preparation
        .expect("checked Host must select preparation");
    assert!(parts.configuration.execution.is_some());
    let bytes = payload(context, issuers);
    assert!(bytes.len() <= N && bytes.len() <= policy.maximum_assignment_bytes as usize);
    let challenge = b::IssuedHostCommandChallenge::issue(
        parts.member,
        actual,
        &parts.bootstrap.owner,
        b::HostCommandReceiveBounds {
            payload_contract: b::HOST_PREPARED_WORKSET_CONTRACT,
            maximum_payload_bytes: policy.maximum_assignment_bytes,
            lifetime_millis: 100,
        },
        [0x67; 32],
        now,
    )
    .unwrap();
    let signature = signer
        .sign(&b::host_command_attribution_transcript(challenge.challenge(), &bytes).unwrap())
        .to_bytes();
    let attributed = challenge.authenticate(&bytes, &signature, now).unwrap();
    b::check_host_workset_preparation_delegation(Some(policy), context, &attributed).unwrap();
    let stamp = attributed.into_retained_stamp().unwrap();
    parts.workspace.writable().unwrap()[..bytes.len()].copy_from_slice(&bytes);
    parts.workspace.retain(stamp).unwrap();
}

/// These are inline sizes for this CPU ABI, not RP2040 memory measurements.
/// The negative fixture never constructed the Kernel, Sign store, or shared
/// execution workspace. Requested-allocation totals exclude these inline values
/// unless they are actually boxed inside the measured interval.
pub fn report_cpu_inline_sizes<const N: usize, I, D>(
    root: &Root<'_, N>,
    equipment: &crate::neutral_equipment::Equipment<I, D>,
    residents: &crate::resident::generated::Initialized,
) {
    println!("cpu_inline_kernel_bytes={} cpu_inline_root_bytes={} cpu_inline_equipment_bytes={} cpu_inline_residents_bytes={} cpu_pointer_bits={} not_target_fit",
        core::mem::size_of::<crate::neutral_factory::Kernel<'static, 'static>>(),
        core::mem::size_of_val(root), core::mem::size_of_val(equipment),
        core::mem::size_of_val(residents), usize::BITS);
}

/// Measure successful construction of the whole original factory, including
/// value backing, software owners, native tickets, Sign store and kernel box.
/// Original Root/equipment/residents existed before this CPU interval. Their
/// backing, allocator metadata and stack remain explicit exclusions.
pub fn measure_preparation<
    I: conduit_i2c_transaction::I2cProvider,
    D: conduit_monotonic_wait::clock_at::owner::MonotonicDeadlineProvider,
>(
    root: &mut Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    equipment: &mut crate::neutral_equipment::Equipment<I, D>,
    residents: &mut crate::resident::generated::Initialized,
    issuers: &mut crate::neutral_authority::Issuers,
    operations: &crate::neutral_authority::Operations,
    runtime: &crate::neutral_preflight::Runtime,
    maximum: usize,
) {
    use conduit_mcu_host_runtime::{execution::Policy, execution_factory::SoftwareInventory};
    // Named mandatory receipt bytes remain the original Plan quota. The concrete
    // numeric event log uses its separate existing representation and extent.
    let event_items = u16::try_from(layout::SIGN_ITEMS).unwrap();
    let event_bytes = u32::try_from(
        layout::SIGN_ITEMS
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>())
            .unwrap(),
    )
    .unwrap();
    let sign_shape = conduit_kernel::HostedSignLog::new_with_remote_storage(
        event_items,
        event_bytes,
        event_items,
        conduit_kernel::remote_sign_storage_bytes(event_items).unwrap(),
    );
    println!("baseline_full_remote_trace_diagnostic=true cpu_original_sign_items={} cpu_plan_named_sign_receipt_bytes={} cpu_kernel_event_bytes={} cpu_numeric_event_log_byte_capacity={event_bytes} cpu_remote_sign_extent={} cpu_numeric_event_log_constructor={:?}",
        layout::SIGN_ITEMS, layout::SIGN_BYTES, core::mem::size_of::<conduit_kernel::KernelEvent>(),
        conduit_kernel::remote_sign_storage_bytes(event_items).unwrap(), sign_shape.map(|_| "prepared"));
    let parts = root.execution_parts().unwrap();
    let actual = parts.resources.host();
    let context = b::NativePreparationContext {
        bootstrap: parts.bootstrap,
        member: parts.member,
        actual,
        image_id: parts.configuration.image_id,
        configuration_id: parts.configuration.configuration_id,
        now_millis: (runtime.now)(),
    };
    let policy = Policy {
        preparation: parts.configuration.preparation,
        execution: parts.configuration.execution,
        active_worksets: 0,
        maximum_retained_bytes: maximum,
    };
    let publisher = core::cell::RefCell::new(None);
    let interval = crate::probe::ObservationInterval::begin();
    let mut software = SoftwareInventory::<{ crate::neutral_workset::N }>::empty();
    // SAFETY: definitions and identities are emitted together from the original
    // checked source, just as in production preflight. No caller material chooses
    // a provider or rewrites any generated schema/identity.
    unsafe {
        software
            .initialize_by(
                |n| crate::neutral_workset::source::definition_at(n, &[]),
                |n| crate::neutral_workset::source::identity_at(n, &[]),
                &publisher,
                conduit_environment_runtime::publication::PublicationSequence::prepare(0),
                runtime.now,
                runtime.allocated_bytes,
                crate::neutral_workset::source::CONFIGURATION,
                None,
            )
            .unwrap();
    }
    let prepared = crate::neutral_factory::prepare(
        parts.workspace,
        context,
        policy,
        residents,
        &mut software,
        &mut equipment.bus,
        &mut equipment.clock,
        &mut issuers.bus,
        &mut issuers.clock,
        operations,
    )
    .unwrap_or_else(|error| panic!("positive whole factory preparation: {error:?}"));
    let retained = crate::probe::current();
    drop(prepared);
    drop(software);
    let observation = interval.finish();
    println!("cpu_positive_whole_factory_retained_requested_bytes={retained} cpu_positive_whole_factory_peak_requested_bytes={} cpu_pointer_bits={} exclusions=preexisting_Root_equipment_residents_issuer_grants_allocator_overhead_stack no_admission_or_target_fit", observation.peak_bytes, usize::BITS);
    assert!(equipment.bus.is_some() && equipment.clock.is_some());
}
