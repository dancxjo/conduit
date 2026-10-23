//! Compiler round trip with inert retained native providers. Make-owned layout
//! material here is not independent provisioning into a physical Root.
use super::*;
use conduit_environment_make::{NativeApplianceLimits, PreparedNativeAppliance};
use conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance;
pub(super) fn round_trip(
    source: &conduit_environment_runtime::construction::ApplianceSource,
    host: &c::HostAdvertisement,
    grants: &[c::AuthorityGrant],
    plan: &c::Plan,
) {
    let limits = NativeApplianceLimits::resident_local();
    let projected = PreparedNativeAppliance::prepare(
        source,
        host,
        &["conduit.base/local@1".into()],
        grants,
        limits,
    )
    .unwrap();
    assert_eq!(projected.plan(), plan);
    assert_eq!(projected.image().resources.len(), 2);
    let source_recipe =
        conduit_environment_make::NativeSoftwareSourceRecipe::prepare(source).unwrap();
    let assigned_records: Vec<_> = c::AssignedPlanRecords::new(projected.assignment())
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let source_records: Vec<_> = source_recipe.resident_computation_records().collect();
    assert_eq!(source_records.len(), projected.computations().len());
    for record in source_records {
        assert_eq!(
            assigned_records
                .iter()
                .filter(|assigned| **assigned == record)
                .count(),
            1
        );
    }
    let recipe = projected.software_recipe().unwrap();
    let resident = recipe.resident();
    let calls: Vec<_> = projected
        .image()
        .host_calls
        .iter()
        .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
        .collect();
    let resources: Vec<_> = projected
        .image()
        .resources
        .iter()
        .map(|resource| resource.resource)
        .collect();
    let required = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(host.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(host.boot_id.as_str()),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    let (result, memory) = allocation_probe::observe(|| {
        ValidatedAssignedAppliance::admit(
            projected.assignment(),
            limits.assignment,
            required,
            &resident,
        )
    });
    assert!(result.is_ok());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let mut stale = required;
    stale.boot = c::AssignedIdentity::from_text("stale/boot");
    let (result, memory) = allocation_probe::observe(|| {
        ValidatedAssignedAppliance::admit(
            projected.assignment(),
            limits.assignment,
            stale,
            &resident,
        )
    });
    assert!(result.is_err());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let mut insufficient = limits;
    insufficient.image.maximum_resources = 1;
    assert!(PreparedNativeAppliance::prepare(
        source,
        host,
        &["conduit.base/local@1".into()],
        grants,
        insufficient
    )
    .is_err());
}
