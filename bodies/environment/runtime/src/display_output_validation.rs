//! Validated finite output topology retained independently of ingress bytes.
//! This token grants neither native possession nor permission to start Play.
use super::assigned::*;
use super::*;
use crate::assigned::AssignedKernelTables;
type Tables = AssignedKernelTables<4, 5, 1, 5, 5, 4>;
pub struct ValidatedAssignedDisplay {
    pub(super) tables: Tables,
    pub(super) frame: c::AssignedForeRequirement,
    pub(super) refusal: c::AssignedForeRequirement,
}
impl ValidatedAssignedDisplay {
    pub fn identity(&self) -> c::AssignedPlanView {
        self.tables.identity
    }
    /// Validate against independently linked Make facts and current Root scope.
    /// No retained software is consumed and no Host Call is performed.
    pub fn admit(
        input: AssignedDisplayInput<'_>,
        profile: &AssignedDisplayProfile<'_>,
        scope: AssignedDisplayScope<'_>,
    ) -> Result<Self, AssignedDisplayRefusal> {
        use AssignedDisplayRefusal as E;
        if profile.fore.len() != 2 || profile.write.0 >= 4 || scope.generation.0 == 0 {
            return Err(E::Shape);
        }
        let fore = |name: &str, direction| {
            profile
                .fore
                .iter()
                .find(|f| {
                    f.direction() == direction
                        && f.payload()[5..21] == c::AssignedIdentity::from_text(name).0
                })
                .copied()
                .ok_or(E::Shape)
        };
        let frame = fore("frame", c::AssignedForeDirection::Input)?;
        let refusal = fore("invalid_brightness", c::AssignedForeDirection::Output)?;
        let required = c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(scope.host.as_str()),
            boot: c::AssignedIdentity::from_text(scope.boot.as_str()),
            host_calls: profile.host_calls,
            resources: profile.resources,
            remote_bindings: &[],
        };
        let tables = || {
            Tables::prepare_records_with_fore(
                input.header,
                input.records,
                input.maxima,
                required,
                profile.computations,
                &profile.layout,
                profile.fore,
            )
            .map_err(E::Assignment)
        };
        let first = tables()?;
        if first.identity.counts[6] != 3
            || first.identity.counts[7] != 1
            || first.cords.iter().any(|c| {
                c.item_capacity != 1 || c.byte_capacity > 1024 || c.maximum_value_bytes > 1024
            })
        {
            return Err(E::Shape);
        }
        let mut write_calls = input.records.iter().filter(|r| {
            r.tag == c::ASSIGNED_HOST_CALL
                && r.payload.get(..2) == Some(&profile.write.0.to_le_bytes())
        });
        let call = write_calls.next().ok_or(E::Shape)?.payload;
        if write_calls.next().is_some()
            || call.len() != c::ASSIGNED_NATIVE_CALL_BYTES
            || call[4..20] != c::AssignedIdentity::from_text(conduit_tm1637_wire::WRITE_CALL).0
            || c::assigned_native_binding(call)
                .map_err(|_| E::Shape)?
                .is_none()
            || !call[2..4].iter().all(|b| *b == 0)
            || call[36..38] != 1u16.to_le_bytes()
            || call[38..42] != 1024u32.to_le_bytes()
            || call[42..46] != 0u32.to_le_bytes()
        {
            return Err(E::Shape);
        }
        type Check = FixedScheduler<
            NoStep,
            k::FixedValueStore<1, 1>,
            k::FixedSignLog<1>,
            4,
            5,
            1,
            5,
            5,
            5,
            4,
            4,
        >;
        Check::new_with_host_calls(
            first.nodes,
            first.cords,
            first.routes,
            first.calls,
            core::array::from_fn(|_| NoStep),
            k::FixedValueStore::new(1).map_err(|_| E::Bounds)?,
            k::FixedSignLog::new(core::mem::size_of::<k::KernelEvent>() as u32)
                .map_err(|_| E::Bounds)?,
        )
        .map_err(|_| E::Tables)?;
        Ok(Self {
            tables: tables()?,
            frame,
            refusal,
        })
    }
}
