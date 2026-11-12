//! Cold authenticated workset decoding before native kernel construction.
use super::*;
use alloc::vec::Vec;

// This private boundary returns the same opaque approvals and command stamp.
// No reservation, native bind or execution authority is created by decoding.
#[allow(clippy::type_complexity)]
#[inline(never)]
pub(super) fn receive<'w, 'p, const I: usize, const F: usize, const PORTS: usize, E>(
    workspace: &'w b::HostCommandInput<I>,
    context: b::NativePreparationContext<'_>,
    policy: Policy<'_>,
    mut profile_for: impl FnMut(usize, &[u8]) -> Result<AssignedPreparationProfile<'p>, E>,
) -> Result<
    (
        Box<[b::DelegatedBodyFragment<'w>; F]>,
        Box<[b::ApprovedBodyForePorts<PORTS>; F]>,
        b::RetainedHostCommandStamp,
    ),
    Refusal<E>,
> {
    if F == 0 {
        return Err(Refusal::Bounds);
    }
    let command = workspace
        .reborrow(
            context.member,
            context.actual,
            &context.bootstrap.owner,
            b::HOST_PREPARED_WORKSET_CONTRACT,
            context.now_millis,
        )
        .map_err(Refusal::Retention)?;
    let set = b::receive_delegated_body_workset::<F>(policy.preparation, context, command)
        .map_err(Refusal::Workset)?;
    set.check_execution_delegation(
        policy.execution,
        policy.preparation,
        context,
        policy.active_worksets,
    )
    .map_err(Refusal::Execution)?;
    if set.fragment_count() != F {
        return Err(Refusal::Scope);
    }
    // Finite temporary preparation storage, independently funded by Root.
    let mut fragments = Vec::with_capacity(F);
    let mut ports = Vec::with_capacity(F);
    for index in 0..F {
        let profile = profile_for(index, set.document(index).ok_or(Refusal::Scope)?)
            .map_err(Refusal::Back)?;
        let received = match profile {
            AssignedPreparationProfile::Legacy(profile) if profile.fore.is_some() => set
                .receive_fragment_with_pinned_fore::<PORTS>(
                    index,
                    policy.preparation,
                    context,
                    profile,
                ),
            AssignedPreparationProfile::Legacy(profile) => set
                .receive_fragment_with_approved_fore::<PORTS>(
                    index,
                    policy.preparation,
                    context,
                    profile,
                ),
            AssignedPreparationProfile::LineFore(profile) => set
                .receive_fragment_with_line_fore::<PORTS>(
                    index,
                    policy.preparation,
                    context,
                    profile,
                ),
        };
        let (fragment, fore) = received.map_err(Refusal::Workset)?;
        fragments.push(fragment);
        ports.push(fore);
    }
    // Preserve the preparation allocation instead of moving every approval
    // through successive affine return frames.
    let fragments: Box<[_; F]> = fragments
        .into_boxed_slice()
        .try_into()
        .map_err(|_| Refusal::Bounds)?;
    let ports: Box<[_; F]> = ports
        .into_boxed_slice()
        .try_into()
        .map_err(|_| Refusal::Bounds)?;
    let stamp = set.into_retained_stamp().map_err(Refusal::Retention)?;
    Ok((fragments, ports, stamp))
}
