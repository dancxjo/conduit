//! Release only original admitted keys after Root has established retirement.
use conduit_body as b;
use conduit_core as c;
pub fn release(
    fragments: &[Option<&b::DelegatedBodyFragment<'_>>],
    owner: &mut c::ResourceAdmissionOwner,
    cause: c::ResourceReleaseCause,
) -> bool {
    // Validate the entire release set before modifying the existing ledger.
    for fragment in fragments.iter().flatten() {
        let m = fragment.metadata();
        for resource in conduit_body::PlainFragmentResources::new(m.resource_bindings) {
            let Ok(resource) = resource else {
                return false;
            };
            if !owner.admissions().iter().any(|a| {
                a.plan_id.as_str() == m.body_plan && a.placement_id.as_str() == resource.placement
            }) {
                return false;
            }
        }
    }
    for fragment in fragments.iter().flatten() {
        let m = fragment.metadata();
        let entries = || conduit_body::PlainFragmentResources::new(m.resource_bindings);
        for (index, resource) in entries().enumerate() {
            let resource = resource.expect("preflight checked immutable resource keys");
            if entries()
                .take(index)
                .any(|prior| prior.is_ok_and(|p| p.placement == resource.placement))
            {
                continue;
            }
            if owner
                .release_borrowed(m.body_plan, resource.placement, cause)
                .is_err()
            {
                return false;
            }
        }
    }
    true
}
