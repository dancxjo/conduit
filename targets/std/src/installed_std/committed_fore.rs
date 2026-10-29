//! Borrowed evidence of an actual kernel egress offer, never an effect grant.
use super::InstalledScheduler;
use conduit_core::{ActivePlayIdentity, PlanFragment, PlannedForePort, PortDirection};
use conduit_kernel::{scheduler::RemoteValueOffer, CordEndpoint};
use conduit_plan_lowering::lowering::{LoweredForePort, LoweredPlanFragment};

/// Inspection of the exact checked Source and child Play that committed a value.
/// These borrowed descriptions cannot construct a `CommittedForeValue`.
pub struct CommittedForeScope<'a> {
    pub fragment: &'a PlanFragment,
    pub play: &'a ActivePlayIdentity,
    pub fore: &'a PlannedForePort,
    pub lowered: &'a LoweredForePort,
}

/// One borrowed, post-commit Cord offer from ordinary installed execution.
/// It grants neither native resource possession nor Body membership. A consumer
/// needing native Source authority must also retain its independently admitted,
/// selected Source owner. The value cannot outlive the actual callback.
///
/// An old effect request cannot be upgraded into this evidence:
/// ```compile_fail
/// use conduit_std_host::{CommittedForeValue, ExternalForeDelivery};
/// fn upgrade(delivery: &ExternalForeDelivery) -> CommittedForeValue<'_> {
///     delivery.into()
/// }
/// ```
/// Commit evidence is affine; retaining inspection does not duplicate it:
/// ```compile_fail
/// use conduit_std_host::CommittedForeValue;
/// fn replay<'a>(value: CommittedForeValue<'a>) ->
///     (CommittedForeValue<'a>, CommittedForeValue<'a>) {
///     (value, value)
/// }
/// ```
/// Evidence cannot be retained after its actual execution callback:
/// ```compile_fail
/// use conduit_std_host::CommittedForeValue;
/// fn retain(value: CommittedForeValue<'_>) -> CommittedForeValue<'static> {
///     value
/// }
/// ```
pub struct CommittedForeValue<'a> {
    scope: CommittedForeScope<'a>,
    sequence: u64,
    bytes: &'a [u8],
}
impl CommittedForeValue<'_> {
    pub fn scope(&self) -> &CommittedForeScope<'_> {
        &self.scope
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }
}

struct Slot<'a> {
    fore: &'a PlannedForePort,
    lowered: &'a LoweredForePort,
    next_sequence: Option<u64>,
}
pub(super) struct PreparedCommittedFore<'a> {
    fragment: &'a PlanFragment,
    play: &'a ActivePlayIdentity,
    slots: Vec<Slot<'a>>,
}
impl<'a> PreparedCommittedFore<'a> {
    pub(super) fn prepare(
        fragment: &'a PlanFragment,
        lowered: &'a LoweredPlanFragment,
        play: &'a ActivePlayIdentity,
    ) -> Result<Self, String> {
        if lowered.identity.plan_id != fragment.plan_id
            || lowered.identity.fragment_id != fragment.fragment_id
            || *play
                != conduit_core::bind_active_play(
                    &fragment.plan_id,
                    &fragment.host_id,
                    &fragment.boot_id,
                    play.play_sequence,
                )
        {
            return Err("committed Fore requires the exact installed child Play".into());
        }
        let slots = lowered
            .fore_ports
            .iter()
            .filter(|port| port.direction == PortDirection::Output)
            .map(|port| {
                let mut candidates = fragment.fore_ports.iter().filter(|fore| {
                    fore.direction == PortDirection::Output
                        && fore.front_port_id == port.front_port_id
                        && fore.track == port.track
                });
                let fore = candidates
                    .next()
                    .ok_or("committed Fore has no sealed output")?;
                if candidates.next().is_some()
                    || fore.value_kind != port.value_kind
                    || fore.value_contract != port.value_contract
                    || fore.temporal != port.temporal
                    || fore.byte_capacity != port.byte_capacity
                    || fore.item_capacity != port.item_capacity
                {
                    return Err("committed Fore output does not match its sealed contract");
                }
                let gear = fragment
                    .placements
                    .iter()
                    .find(|gear| gear.placement_id == fore.placement_id)
                    .ok_or("committed Fore producer is absent")?;
                let output = gear
                    .outputs
                    .iter()
                    .position(|output| output.port_id == fore.gear_port_id)
                    .ok_or("committed Fore producer port is absent")?;
                let node = lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or("committed Fore producer was not lowered")?;
                let cord = lowered
                    .cords
                    .iter()
                    .find(|cord| cord.spec.cord == port.cord)
                    .ok_or("committed Fore Cord is absent")?;
                if cord.spec.source
                    != (CordEndpoint::Local {
                        node,
                        port: conduit_kernel::PortId(
                            u16::try_from(output).map_err(|_| {
                                "committed Fore producer port exceeds numeric bound"
                            })?,
                        ),
                    })
                    || cord.spec.sink != CordEndpoint::Remote(port.endpoint)
                {
                    return Err("committed Fore Cord does not belong to the sealed producer edge");
                }
                Ok(Slot {
                    fore,
                    lowered: port,
                    next_sequence: Some(0),
                })
            })
            .collect::<Result<Vec<_>, &str>>()?;
        Ok(Self {
            fragment,
            play,
            slots,
        })
    }

    /// This is the only minting entrance. It reads the actual scheduler offer;
    /// neither a public delivery DTO nor caller-supplied bytes enter here.
    pub(super) fn offer<'s>(
        &'s mut self,
        scheduler: &'s mut InstalledScheduler,
        port: &LoweredForePort,
    ) -> Result<Option<(RemoteValueOffer, CommittedForeValue<'s>)>, String> {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| std::ptr::eq(slot.lowered, port))
            .ok_or("committed Fore has no prepared output slot")?;
        let Some(offer) = scheduler
            .remote_egress_offer(port.endpoint, port.cord)
            .map_err(|error| format!("offer committed Fore output: {error:?}"))?
        else {
            return Ok(None);
        };
        if Some(offer.sequence) != slot.next_sequence {
            return Err("committed Fore offer was repeated, regressed or exhausted".into());
        }
        let bytes = scheduler
            .host_value(offer.value)
            .map_err(|error| format!("read committed Fore output: {error:?}"))?;
        port.validate_value(bytes).map_err(|error| {
            format!("committed Fore value violates its sealed contract: {error:?}")
        })?;
        slot.next_sequence = offer.sequence.checked_add(1);
        Ok(Some((
            offer,
            CommittedForeValue {
                scope: CommittedForeScope {
                    fragment: self.fragment,
                    play: self.play,
                    fore: slot.fore,
                    lowered: slot.lowered,
                },
                sequence: offer.sequence,
                bytes,
            },
        )))
    }
}
