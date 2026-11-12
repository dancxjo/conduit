// Same Source factory, independently prepared renderer and original Start.
use super::*;
use conduit_body as b;
use conduit_core as c;
use conduit_environment_runtime::publication_face_custody::Reader;
use conduit_mcu_host_runtime::continuation_native::ContinuationNativeContext;
use conduit_mcu_host_runtime::{
    execution_authority::Operation, execution_factory::SoftwareInventory,
    execution_group::NativeContracts,
};
use core::fmt::Write;
pub struct Factory<'a, 'f, 'issuers, 'diagnostic> {
    pub residents: &'a mut crate::resident::generated::Initialized,
    pub software: &'a mut SoftwareInventory<'f, { source::sampling::NODE_COUNT }>,
    pub output_software: &'a mut SoftwareInventory<'f, { source::output::NODE_COUNT }>,
    pub routing: &'a mut ReadyMaskRouting,
    pub equipment: &'a mut crate::equipment::Equipment,
    pub issuers: &'issuers mut Issuers<W>,
    pub operations: &'a Operations<'static, W>,
    #[cfg(record_service)] pub recording_resource: &'a mut Option<c::ResourceBinding>,
    pub recording: Option<&'a crate::play::RecordingOperation<'static>>,
    pub sampling_values: conduit_kernel::HostedValueStore,
    pub output_values: conduit_kernel::HostedValueStore,
    pub contracts: &'issuers mut Option<NativeContracts>,
    pub reader: Reader<'a>,
    pub face: &'a LocalPublicationFace,
    pub append_output_bytes: usize,
    pub diagnostic: &'diagnostic mut heapless::String<192>,
}
fn held(
    equipment: &crate::equipment::Equipment,
) -> Result<heapless::Vec<c::AssignedIdentity, 6>, &'static str> {
    let mut result = heapless::Vec::new();
    let mut push = |pool: &str| {
        result
            .push(c::AssignedIdentity::from_text(pool))
            .map_err(|_| "continuation-held-pools")
    };
    push(crate::config::RESIDENT_WORK_POOL)?;
    push(
        equipment
            .pulse
            .as_ref()
            .ok_or("pulse-busy")?
            .identity()
            .resource_pool_id
            .as_str(),
    )?;
    push(
        equipment
            .display
            .as_ref()
            .ok_or("write-busy")?
            .native_identity()
            .resource_pool_id
            .as_str(),
    )?;
    push(
        equipment.waits[0]
            .as_ref()
            .ok_or("wait-busy")?
            .native_identity()
            .resource_pool_id
            .as_str(),
    )?;
    push(
        equipment
            .renderer
            .native_identity()
            .resource_pool_id
            .as_str(),
    )?;
    if let Some(pool) = equipment.recording.held_pool() {
        push(pool.as_str())?;
    }
    Ok(result)
}
#[allow(clippy::too_many_arguments)]
pub(super) async fn prepare_into<'a, 'f: 'a, 'w: 'scope, 'scope>(
    bootstrap: &b::MemberBootstrap<'_>,
    verified: &b::VerifiedContinuationIntent<'_>,
    configuration: &conduit_mcu_host_runtime::root::Configuration,
    scope: &'scope b::FreshContinuationScope<'w>,
    resources: &mut c::ResourceAdmissionOwner,
    policy: Policy<'_>,
    factory: Factory<'a, 'f, '_, '_>,
    target: &mut Option<crate::play::ContinuationStarted<'a, 'f>>,
    scopes_target: &mut Option<crate::play_service::Scopes>,
    activity_target: &mut Option<&'a conduit_mcu_host_runtime::native_runner::NativeActivity>,
) -> Result<(), &'static str> {
    let Factory {
        residents,
        software,
        output_software,
        routing,
        equipment,
        issuers,
        operations,
        #[cfg(record_service)] recording_resource,
        recording,
        sampling_values,
        output_values,
        contracts,
        reader,
        face,
        append_output_bytes,
        diagnostic,
    } = factory;
    #[cfg(record_service)]
    let recording_resource = recording_resource.as_ref();
    let pools = held(equipment).map_err(|error| {
        stage(diagnostic, "held", scope);
        refusal(diagnostic, "continuation-held-pools", error)
    })?;
    *activity_target = Some(&equipment.activity);
    let mut prepared_slot = None;
    stage(diagnostic, "source", scope);
    let future = async {
        let context = ContinuationNativeContext::check_installation(
            bootstrap,
            resources.host(),
            verified,
            configuration.image_id,
            configuration.configuration_id,
            policy,
        )
        .map_err(|error| refusal(diagnostic, "continuation-native-context", error))?;
        let prepared = crate::play::prepare(
            None::<&b::HostCommandInput<{ crate::config::MAX_ASSIGNMENT_BYTES }>>,
            crate::play::NativeAuthority::Continuation { context, scope },
            policy,
            residents,
            software,
            output_software,
            routing,
            &mut equipment.pulse,
            &mut equipment.display,
            &mut equipment.waits,
            equipment.recording.ready_slot(),
            #[cfg(record_service)] recording_resource,
            {
                #[cfg(not(record_service))] { &mut issuers.recording }
                #[cfg(record_service)] { [&mut issuers.recording, &mut issuers.recording_read] }
            },
            recording,
            append_output_bytes,
            &mut issuers.pulse,
            &mut issuers.write,
            &mut issuers.waits,
            operations,
            sampling_values,
            output_values,
            contracts,
            diagnostic,
        )
        .map_err(|error| refusal(diagnostic, "continuation-source-preparation", error))?;
        prepared_slot = Some(prepared);
        Ok::<(), &'static str>(())
    };
    conduit_mcu_host_runtime::poll_boundary::separate(core::pin::pin!(future)).await?;
    let mut presentation_slot = None;
    stage(diagnostic, "renderer", scope);
    let future = async {
        let context = ContinuationNativeContext::check_installation(
            bootstrap,
            resources.host(),
            verified,
            configuration.image_id,
            configuration.configuration_id,
            policy,
        )
        .map_err(|error| refusal(diagnostic, "continuation-renderer-context", error))?;
        presentation_slot = Some(
            crate::play_service::prepare_presentation(
                prepared_slot
                    .as_ref()
                    .ok_or("continuation-prepared-empty")?,
                crate::play::NativeAuthority::Continuation { context, scope },
                policy,
                &mut equipment.renderer,
                &mut issuers.renderer,
                &operations.renderer,
                reader,
                face,
            )
            .map_err(|error| refusal(diagnostic, "continuation-renderer-preparation", error))?,
        );
        Ok::<(), &'static str>(())
    };
    conduit_mcu_host_runtime::poll_boundary::separate(core::pin::pin!(future)).await?;
    stage(diagnostic, "start", scope);
    let future = async {
        crate::play_service::start_continuation_into(
            prepared_slot.take().ok_or("continuation-prepared-empty")?,
            bootstrap,
            verified,
            configuration,
            scope,
            resources,
            policy,
            presentation_slot
                .take()
                .ok_or("continuation-presentation-empty")?,
            face,
            &pools,
            append_output_bytes
                + core::mem::size_of::<
                    conduit_mcu_host_runtime::execution_inspection::PreparedAppendStatus,
                >(),
            target,
            scopes_target,
        )
        .map_err(|error| refusal(diagnostic, "continuation-authenticated-start", error))
    };
    conduit_mcu_host_runtime::poll_boundary::separate(core::pin::pin!(future)).await
}

fn stage(detail: &mut heapless::String<192>, stage: &str, scope: &b::FreshContinuationScope<'_>) {
    detail.clear();
    let documents: usize = (0..scope.fragment_count())
        .filter_map(|index| scope.document(index))
        .map(<[u8]>::len)
        .sum();
    let _ = write!(
        detail,
        "{stage} free={} workspace={} docs={documents} ",
        remaining(),
        crate::config::MAX_ASSIGNMENT_BYTES
    );
}
fn refusal(
    detail: &mut heapless::String<192>,
    reason: &'static str,
    error: impl core::fmt::Debug,
) -> &'static str {
    let _ = write!(detail, " after={} error={error:?}", remaining());
    reason
}
