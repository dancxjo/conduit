//! Source/configuration selection before any native owner loan, for exact finite worksets.
use super::*;

/// Grouped Source construction for inventories whose initialized entries must
/// be checked together and then split into disjoint fragment loans. Signed
/// fragment order is retained; callback roles are independent installed pins.
#[allow(clippy::too_many_arguments)]
#[inline(always)] // Avoid a second complete prepared-group return frame.
pub fn prepare_with_source<'w, 'p, T, E, const INPUT: usize, const PORTS: usize>(
    workspace: &'w b::HostCommandInput<INPUT>,
    context: b::NativePreparationContext<'_>,
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
    prepare_with_source_profiles::<T, E, INPUT, PORTS, 3>(
        workspace,
        context,
        policy,
        pins,
        configurations,
        profiles.map(crate::execution_workset::AssignedPreparationProfile::Legacy),
        factory,
    )
}

/// Opt-in mixed schema preparation. Profiles are installed Source selections;
/// this path grants no carrier, Line, IO or independent Start authority.
/// Grouped Source construction for inventories whose initialized entries must
/// be checked together and then split into disjoint fragment loans. Signed
/// fragment order is retained; callback roles are independent installed pins.
#[allow(clippy::too_many_arguments)]
#[inline(always)] // Avoid a second complete prepared-group return frame.
pub fn prepare_with_source_profiles<
    'w,
    'p,
    T,
    E,
    const INPUT: usize,
    const PORTS: usize,
    const F: usize,
>(
    workspace: &'w b::HostCommandInput<INPUT>,
    context: b::NativePreparationContext<'_>,
    policy: Policy<'_>,
    pins: [EntryPin<'p>; F],
    configurations: [&'p [c::AssignedRecordRequirement]; F],
    profiles: [crate::execution_workset::AssignedPreparationProfile<'p>; F],
    factory: impl FnOnce(
        [&b::DelegatedBodyFragment<'w>; F],
        [&[c::AssignedForeRequirement]; F],
    ) -> Result<T, E>,
) -> Result<PreparedLocalWorkset<'w, T, F>, Refusal<Refusal<E>>>
where
    T: PreparedLocalKernels,
{
    if F == 0
        || pins
            .iter()
            .enumerate()
            .any(|(i, pin)| pins[..i].contains(pin))
    {
        return Err(Refusal::Scope);
    }
    let mut profiles = profiles.map(Some);
    PreparedLocalWorkset::prepare_selected_profiles::<INPUT, PORTS, _>(
        workspace,
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
            let mut indices = [0usize; F];
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
