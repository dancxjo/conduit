// Original continuation origin, with the same small-status affine Start path.
use super::*;
#[allow(clippy::too_many_arguments,clippy::type_complexity)]
#[inline(always)]
pub fn start_continuation_into<'a,'w,T:PreparedLocalKernels,C:RendererClock>(
    prepared:PreparedLocalWorkset<'w,T,3>,bootstrap:&b::MemberBootstrap<'_>,
    verified:&b::VerifiedContinuationIntent<'_>,configuration:&conduit_mcu_host_runtime::root::Configuration,
    scope:&b::FreshContinuationScope<'w>,resources:&mut c::ResourceAdmissionOwner,policy:Policy<'_>,
    presentation:PreparedPresentation<'a,C>,face:&LocalPublicationFace,held_pools:&[c::AssignedIdentity],
    recording_service_bytes:usize,target:&mut Option<StartedLocalWorkset<(T::Bound,RendererService<'a,C>)>>,
    scopes_target:&mut Option<Scopes>,
)->Result<(),Refusal<&'static str>> {
    start_selected_into(prepared,None,PresentationOrigin::Continuation {bootstrap,verified,configuration,scope},
        resources,policy,presentation,face,held_pools,recording_service_bytes,target,scopes_target)
}
