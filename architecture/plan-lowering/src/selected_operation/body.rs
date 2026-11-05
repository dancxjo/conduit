//! Bind an ordinary Plot's selected operation under its actual body-wide Play.
//! This verifies containment; it neither starts a child Play nor grants effects.
use super::*;
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};

pub struct SelectedBodyOperationPlan<'a> {
    pub wake: &'a Wake,
    pub plan: &'a BodyPlan,
    pub play: &'a BodyPlayIdentity,
    pub fragment: &'a PlanFragment,
    pub lowered: &'a LoweredPlanFragment,
    pub placement_id: &'a PlacementId,
}
/// Preparation only. Claim possession must already exist independently. The
/// table/claim name the containing Body Plan/Play; numeric routing still belongs
/// to the exact selected Plot fragment and is checked by the same lowerer.
pub fn bind_body_selected_configured_operation(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    definition: SelectedOperationContract<'_>,
    configuration: &[conduit_core::ConfigurationEntry],
    selection: SelectedBodyOperationPlan<'_>,
) -> Result<NodeId, SelectedCallRefusal> {
    validate_scope(claim, &selection)?;
    bind_fragment_operation(
        table,
        claim,
        definition,
        configuration,
        selection.fragment,
        selection.lowered,
        selection.placement_id,
    )
}
fn validate_scope(
    claim: &BaseOperationClaim,
    selection: &SelectedBodyOperationPlan<'_>,
) -> Result<(), SelectedCallRefusal> {
    let SelectedBodyOperationPlan {
        wake,
        plan,
        play,
        fragment,
        ..
    } = selection;
    let scope =
        crate::body_fragment_scope::PreparedBodyFragment::prepare(wake, plan, play, fragment)
            .map_err(|_| SelectedCallRefusal::WrongBinding)?;
    if claim.plan_id != scope.plan().plan_id
        || claim.active_play_id != scope.play().active_play_id
        || fragment.host_id != claim.host_id
        || fragment.boot_id != claim.boot_id
    {
        return Err(SelectedCallRefusal::WrongBinding);
    }
    Ok(())
}
#[cfg(test)]
#[path = "body/tests.rs"]
mod tests;
