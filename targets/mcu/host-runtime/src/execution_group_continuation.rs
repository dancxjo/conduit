//! Same installed Source/configuration selection with explicit continuation origin.
use super::*;

#[allow(clippy::too_many_arguments)]
#[inline(always)]
pub fn prepare_continuation_with_source<'w, 'p, T, E, const PORTS: usize>(
    scope: &b::FreshContinuationScope<'w>,
    context: crate::continuation_native::ContinuationNativeContext<'_>,
    policy: Policy<'_>,
    pins: [EntryPin<'p>; 3],
    configurations: [&'p [c::AssignedRecordRequirement]; 3],
    profiles: [b::AssignedBodyPreparationProfile<'p>; 3],
    factory: impl FnOnce(
        [&b::DelegatedBodyFragment<'w>; 3],
        [&[c::AssignedForeRequirement]; 3],
    ) -> Result<T, E>,
) -> Result<PreparedLocalWorkset<'w, T, 3>, Refusal<Refusal<E>>>
where
    T: PreparedLocalKernels,
{
    if pins[0] == pins[1] || pins[0] == pins[2] || pins[1] == pins[2] {
        return Err(Refusal::Scope);
    }
    let mut profiles = profiles.map(Some);
    PreparedLocalWorkset::prepare_continuation_selected::<PORTS, _>(
        scope,
        context,
        policy,
        move |_, document| {
            let (metadata, _) =
                b::decode_body_fragment_document(document).map_err(|_| Refusal::Scope)?;
            let role = pins
                .iter()
                .position(|p| p.matches(&metadata))
                .ok_or(Refusal::Scope)?;
            profiles[role].take().ok_or(Refusal::Scope)
        },
        // Fuse this adapter's affine return storage with its caller instead of
        // retaining another complete prepared-group copy on the MCU stack.
        #[inline(always)]
        move |fragments, fore| {
            let mut indices = [0usize; 3];
            for (role, pin) in pins.iter().enumerate() {
                let index = fragments
                    .iter()
                    .position(|f| pin.matches(f.metadata()))
                    .ok_or(Refusal::Scope)?;
                // Every scope and immutable configuration is checked before a
                // single initialized owner may be borrowed by the callback.
                check_configuration(&fragments[index], configurations[role]).map_err(
                    |e| match e {
                        Refusal::Scope => Refusal::Scope,
                        _ => Refusal::Bounds,
                    },
                )?;
                indices[role] = index;
            }
            factory(
                indices.map(|i| &fragments[i]),
                indices.map(|i| fore[i].as_slice()),
            )
            .map_err(Refusal::Back)
        },
    )
}
