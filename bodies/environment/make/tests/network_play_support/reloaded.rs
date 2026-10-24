//! Reloaded checked meaning retains exact identity and still plans against
//! independently supplied current providers/grants before the real CPU Play.
use super::{allocation_probe, c, PreparedNetworkSetup};
pub(super) fn prepare(
    host: &c::HostAdvertisement,
    bases: &[c::BaseImplementationId],
    grants: &[c::AuthorityGrant],
) -> (PreparedNetworkSetup, allocation_probe::Observation) {
    let source = conduit_environment_make::compile_network_setup().unwrap();
    let pinned = c::PlotIdentity {
        source_document_id: source.source_document_id.clone(),
        checked_plot_id: source.checked_plot_id.clone(),
        expanded_plot_id: source.expanded_plot_id.clone(),
    };
    // Retain only the pins across a simulated construction boundary. The linked
    // Source is checked independently again; no cached rich Plan is reused.
    drop(source);
    let source = conduit_environment_make::compile_network_setup().unwrap();
    for index in 0..3 {
        let mut wrong = pinned.clone();
        match index {
            0 => wrong.source_document_id = "source/foreign".into(),
            1 => wrong.checked_plot_id = "checked/foreign".into(),
            _ => wrong.expanded_plot_id = "expanded/foreign".into(),
        }
        let (refused, memory) = allocation_probe::observe(|| {
            PreparedNetworkSetup::from_checked_for_identity(&wrong, &source, host, bases, grants)
        });
        assert!(matches!(
            refused,
            Err(conduit_environment_make::NetworkPlanningRefusal::SourceIdentity)
        ));
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    let (prepared, preparation) = allocation_probe::observe(|| {
        PreparedNetworkSetup::from_checked_for_identity(&pinned, &source, host, bases, grants)
            .unwrap()
    });
    drop(source);
    (prepared, preparation)
}
