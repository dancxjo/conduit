//! Original complete-factory call site; setup uses real Source initializers.
use crate::{
    runtime::{allocator_used, now_ms},
    workset::{generated as source, renderer as display},
};
use conduit_environment_runtime::{
    publication::PublicationSequence, publication_face_custody::LocalPublicationFace,
    retained_publication_face::RetainedPublicationBacking,
};
use conduit_mcu_host_runtime::{
    execution::Policy,
    execution_mask::ReadyMaskRouting,
    native_authority::{Issuers, Operations},
};
use core::cell::RefCell;
const W: usize = crate::equipment::WAIT_SLOTS;
mod initialization {
    include!(concat!(env!("OUT_DIR"), "/source_play_initialization.rs"));
}
pub fn probe(
    authority: &crate::authority::Authority,
    root: &mut conduit_mcu_host_runtime::root::Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }>,
    mut equipment: crate::equipment::Equipment,
) {
    let mut residents = crate::resident::inventory().unwrap();
    let mut routing = initialization::routing();
    let clocks: &[(u16, u64, u64); W] = source::sampling::CLOCKS.try_into().unwrap();
    let (sampling_values, parameters) = crate::workset::values(
        source::sampling_layout::CORD_VALUE_SLOT_CAPACITIES,
        source::sampling_layout::STEP_VALUE_SLOT_CAPACITIES,
        source::sampling_layout::HOST_CALL_VALUE_SLOT_CAPACITIES,
        clocks,
        crate::runtime::remaining(),
    )
    .unwrap();
    let (output_values, _) = crate::workset::values(
        source::output_layout::CORD_VALUE_SLOT_CAPACITIES,
        source::output_layout::STEP_VALUE_SLOT_CAPACITIES,
        source::output_layout::HOST_CALL_VALUE_SLOT_CAPACITIES,
        &[],
        crate::runtime::remaining(),
    )
    .unwrap();
    let (events, recording_operation) = initialization::publication_bounds(&parameters);
    let face = LocalPublicationFace::from_backing(
        RetainedPublicationBacking::prepare(events, display::RENDERER_SHOW_LIFETIME_MS).unwrap(),
    );
    let (publisher, reader) = face.claim().unwrap();
    let publisher = RefCell::new(Some(publisher));
    let sequence = PublicationSequence::prepare(events as u64);
    let mut software = None;
    let mut output_software = None;
    initialization::sampling(
        &mut software,
        &parameters,
        clocks,
        &equipment,
        &publisher,
        sequence.clone(),
    );
    initialization::output(&mut output_software, &publisher, sequence);
    let operations = initialization::operations(clocks);
    let mut issuers = initialization::issuers();
    let mut contracts = Some(
        conduit_mcu_host_runtime::execution_group::NativeContracts::prepare_with_recording(false)
            .unwrap(),
    );
    let policy = Policy {
        preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
        execution: crate::config::EXECUTION_DELEGATION.as_ref(),
        active_worksets: 0,
        maximum_retained_bytes: crate::config::HEAP_BYTES,
    };
    let context = root.return_context(now_ms()).unwrap();
    issuers
        .authorize(
            context,
            policy,
            equipment.pulse.as_ref().unwrap(),
            equipment.display.as_ref().unwrap(),
            &equipment.renderer,
            &equipment.waits,
            &operations,
            crate::runtime::remaining,
        )
        .unwrap();
    let recording_operation = recording_operation.unwrap();
    equipment
        .recording
        .authorize(
            context,
            policy,
            [&mut issuers.recording, &mut issuers.recording_read],
            [&recording_operation[0], &recording_operation[1]],
            crate::runtime::remaining(),
        )
        .unwrap();
    let resource = equipment
        .recording
        .resource_binding(crate::runtime::remaining())
        .unwrap();
    let (hot, cold) = crate::play::fore::installation::prepare(
        context,
        crate::config::FORE_CARRIER_IMPLEMENTATION,
        crate::config::FORE_CARRIER_BOUNDS.unwrap(),
        &source::sampling_layout::fore(),
    )
    .unwrap();
    let payload = crate::payload::workset(context, &issuers, &hot);
    crate::authority::prepare(root, authority, &payload);
    let parts = root.execution_parts().unwrap();
    let context = conduit_body::NativePreparationContext {
        bootstrap: parts.bootstrap,
        member: parts.member,
        actual: parts.resources.host(),
        image_id: parts.configuration.image_id,
        configuration_id: parts.configuration.configuration_id,
        now_millis: now_ms(),
    };
    let mut cold = Some(cold);
    let mut link = crate::transport::Link;
    let mut buffers = crate::play::fore::buffers().unwrap();
    let mut carrier_inputs = Some((&mut link, &mut buffers));
    let mut diagnostic = heapless::String::new();
    // Original complete factory consumes actual retained signed command and
    // prepared native owners; this baseline expects the structural Layout refusal.
    let identity = conduit_core::AssignedIdentity::from_text;
    let held_pools = heapless::Vec::<_, 6>::from_slice(&[
        identity(crate::config::RESIDENT_WORK_POOL),
        identity(
            equipment
                .pulse
                .as_ref()
                .unwrap()
                .identity()
                .resource_pool_id
                .as_str(),
        ),
        identity(
            equipment
                .display
                .as_ref()
                .unwrap()
                .native_identity()
                .resource_pool_id
                .as_str(),
        ),
        identity(
            equipment.waits[0]
                .as_ref()
                .unwrap()
                .native_identity()
                .resource_pool_id
                .as_str(),
        ),
        identity(
            equipment
                .renderer
                .native_identity()
                .resource_pool_id
                .as_str(),
        ),
        identity(equipment.recording.held_pool().unwrap().as_str()),
    ])
    .unwrap();
    let original_input = parts.workspace.payload_digest().unwrap();
    let result = crate::play::prepare(
        &*parts.workspace,
        context.into(),
        policy,
        &mut residents,
        software.as_mut().unwrap(),
        output_software.as_mut().unwrap(),
        &mut routing,
        &mut equipment.pulse,
        &mut equipment.display,
        &mut equipment.waits,
        equipment.recording.ready_slot(),
        resource.as_ref(),
        [&mut issuers.recording, &mut issuers.recording_read],
        Some(&recording_operation),
        crate::play::RECORDING_OUTPUT_BYTES,
        &mut issuers.pulse,
        &mut issuers.write,
        &mut issuers.waits,
        &operations,
        sampling_values,
        output_values,
        &mut contracts,
        &mut diagnostic,
        Some(crate::play::fore::Selection {
            profile: hot.profile(),
            bindings: &hot.bindings,
        }),
        &mut |approved, ports, profile| {
            let (link, buffers) = carrier_inputs.take().unwrap();
            crate::play::fore::prepare_components(
                &parts,
                context,
                &hot,
                cold.take().unwrap(),
                link,
                buffers,
                approved,
                ports.unwrap(),
                profile,
            )
            .map(Some)
        },
    );
    println!(
        "CPU_FACTORY_RETURN original_complete_factory_called=true error={:?} detail={}",
        result.as_ref().err(),
        diagnostic
    );
    match result {
        Err(error) => {
            assert!(
                std::env::var_os("CPU_EXPECT_PREPARED").is_none(),
                "expected successful factory, got {error:?}: {diagnostic}"
            );
            assert!(matches!(
                error,
                conduit_mcu_host_runtime::execution::Refusal::Back(
                    conduit_mcu_host_runtime::execution::Refusal::Back("sampling-kernel")
                )
            ));
            assert_eq!(diagnostic.as_str(), "Tables(Layout)");
            assert_eq!(parts.workspace.payload_digest(), Some(original_input));
            assert!(parts.resources.admissions().is_empty());
            assert!(parts.resources.assignments().is_empty());
            println!("CPU_FACTORY_NEGATIVE original_complete_factory_called=true supplied_recording_output={} result=sampling-kernel/Tables(Layout) retained_input=true renderer_prepared=false admission=false Start=false",crate::play::RECORDING_OUTPUT_BYTES);
        }
        Ok(prepared) => {
            assert!(
                std::env::var_os("CPU_EXPECT_PREPARED").is_some(),
                "baseline unexpectedly prepared; explicitly qualify corrected case"
            );
            let presentation = crate::play_service::prepare_presentation(
                &prepared,
                context.into(),
                policy,
                &mut equipment.renderer,
                &mut issuers.renderer,
                &operations.renderer,
                reader,
                &face,
            )
            .expect("original renderer preparation");
            let mut started = None;
            let mut scopes = None;
            crate::play_service::start_prepared_into(
                prepared,
                &*parts.workspace,
                parts.bootstrap,
                parts.member,
                parts.configuration,
                parts.resources,
                policy,
                presentation,
                &face,
                &held_pools,
                crate::play::RECORDING_OUTPUT_BYTES
                    + core::mem::size_of::<
                        conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus,
                    >()
                    + core::mem::size_of::<Option<crate::play::fore::installation::Hot>>()
                    + core::mem::size_of::<crate::play::fore::Buffers>(),
                &mut started,
                &mut scopes,
            )
            .expect("original complete admission and Start");
            assert!(started.is_some());
            assert!(scopes.is_some());
            assert_eq!(parts.workspace.payload_digest(), Some(original_input));
            assert!(!parts.resources.admissions().is_empty());
            assert_ne!(
                started.as_ref().unwrap().evidence().custody_identity(),
                &[0; 32]
            );
            println!("CPU_FACTORY_STARTED original_complete_factory_called=true renderer_prepared=true admission=true Start=true provider_effects=0 retained_input=true");
            println!(
                "CPU_STARTED_RAM retained={} available={} original_limit={}",
                allocator_used(),
                crate::runtime::remaining(),
                crate::config::HEAP_BYTES
            );
            core::hint::black_box((&started, &scopes));
        }
    }
    println!("CPU_PEAK preparation_heap={} original_limit={} all_fixture_metadata_included=true target_fit=false",crate::accounting::peak(),crate::config::HEAP_BYTES);
    println!("CPU_AFTER_CASE_RAM charged_retained={} available={} original_limit={} simulated_nor_bytes={} fixture_metadata_and_sender_included=true",allocator_used(),crate::runtime::remaining(),crate::config::HEAP_BYTES,crate::config::RECORDING_PREFIX_BYTES as usize+crate::config::RECORDING_SLOTS as usize*512);
}
