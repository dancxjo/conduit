//! Original native authorization retained before remote planning. Source and
//! checked physical construction supply operations; incoming Plans supply none.
extern crate alloc;
use crate::execution_mask::{ReadyRenderer, RendererClock};
use crate::{
    execution::Policy,
    execution_authority::{Issuer, NativeFacts, Operation, Refusal},
};
use alloc::{string::String, vec::Vec};
use conduit_body::NativePreparationContext;
use conduit_core::AuthorityGrant;
use conduit_monotonic_wait::{MonotonicWaitProvider, ReadyMonotonicWaitBase};
use conduit_pulse_capture::{installation::ReadyPulseBase, owner::PulseReadProvider};
use conduit_tm1637_wire::{installation::ReadyWriteBase, OpenDrain};

pub struct Operations<'a, const W: usize> {
    pub pulse: Operation<'a>,
    pub write: Operation<'a>,
    pub renderer: Operation<'a>,
    pub waits: [Option<Operation<'a>>; W],
}
impl<'a, const W: usize> Operations<'a, W> {
    /// Construct finite operation ceilings from independently compiled Source
    /// nodes/clocks and checked renderer construction, never received metadata.
    /// The BSP maps each actual wait slot to its exact Source clock tuple.
    pub fn from_source(
        sampling: &'a [(u16, &'a str)],
        mask: &'a [(u16, &'a str)],
        output: &'a [(u16, &'a str)],
        clocks: [Option<(u16, u64, u64)>; W],
        maximum_frames: u32,
        face_bytes: u32,
        show_bytes: u32,
    ) -> Result<Self, Refusal> {
        fn unique<'a>(nodes: &'a [(u16, &'a str)], kind: &str) -> Result<&'a str, Refusal> {
            let mut matches = nodes.iter().filter(|n| n.1 == kind);
            let actual = matches.next().ok_or(Refusal::Scope)?.1;
            if matches.next().is_some() {
                return Err(Refusal::Scope);
            }
            Ok(actual)
        }
        fn operation<'a>(
            kind: &'a str,
            authority: &'a str,
            call: &'a str,
            input: u32,
            output: u32,
            count: u32,
        ) -> Operation<'a> {
            Operation {
                kind,
                authority,
                call,
                maximum_parameter_bytes: input,
                maximum_result_bytes: output,
                maximum_work_units: 1,
                maximum_operations: count,
            }
        }
        if maximum_frames == 0 || face_bytes == 0 || show_bytes == 0 {
            return Err(Refusal::Bounds);
        }
        if sampling
            .iter()
            .filter(|n| n.1 == conduit_time::TICK_KIND)
            .count()
            != clocks.iter().flatten().count()
        {
            return Err(Refusal::Scope);
        }
        let mut total = 0u64;
        let mut waits = core::array::from_fn(|_| None);
        for (slot, clock) in clocks.into_iter().enumerate() {
            let Some((node, period, count)) = clock else {
                continue;
            };
            if clocks[..slot].iter().flatten().any(|prior| prior.0 == node) {
                return Err(Refusal::Scope);
            }
            if period == 0 || count == 0 {
                return Err(Refusal::Bounds);
            }
            let mut matching = sampling.iter().filter(|n| n.0 == node);
            let kind = matching.next().ok_or(Refusal::Scope)?.1;
            if matching.next().is_some() || kind != conduit_time::TICK_KIND {
                return Err(Refusal::Scope);
            }
            total = total.checked_add(count).ok_or(Refusal::Bounds)?;
            waits[slot] = Some(operation(
                kind,
                conduit_monotonic_wait::WAIT_AUTHORITY,
                conduit_core::WAIT_HOST_CALL_CONTRACT,
                conduit_monotonic_wait::PARAMETER_BYTES,
                0,
                count.try_into().map_err(|_| Refusal::Bounds)?,
            ));
        }
        let total = u32::try_from(total).map_err(|_| Refusal::Bounds)?;
        if total == 0 {
            return Err(Refusal::Bounds);
        }
        Ok(Self {
            pulse: operation(
                unique(sampling, conduit_pulse_capture::PULSE_READ_KIND)?,
                conduit_pulse_capture::installation::PULSE_AUTHORITY,
                conduit_pulse_capture::PULSE_READ_CALL,
                conduit_pulse_capture::PULSE_READ_MAXIMUM_BYTES,
                conduit_pulse_capture::PULSE_READ_MAXIMUM_BYTES,
                total,
            ),
            write: operation(
                unique(output, conduit_tm1637_wire::WRITE_KIND)?,
                conduit_tm1637_wire::installation::WRITE_AUTHORITY,
                conduit_tm1637_wire::WRITE_CALL,
                conduit_tm1637_wire::MAXIMUM_BYTES,
                0,
                maximum_frames,
            ),
            renderer: operation(
                unique(mask, conduit_face::RENDERER_KIND)?,
                crate::execution_mask::AUTHORITY,
                crate::execution_mask::CALL,
                face_bytes,
                show_bytes,
                maximum_frames,
            ),
            waits,
        })
    }
}
/// One opaque issuer per actual owner, no duplicated Root/resource ledger.
pub struct Issuers<const W: usize> {
    pub recording: Issuer,
    pub pulse: Issuer,
    pub write: Issuer,
    pub renderer: Issuer,
    pub waits: [Issuer; W],
}
impl<const W: usize> Default for Issuers<W> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const W: usize> Issuers<W> {
    pub fn new() -> Self {
        Self {
            recording: Issuer::new(),
            pulse: Issuer::new(),
            write: Issuer::new(),
            renderer: Issuer::new(),
            waits: core::array::from_fn(|_| Issuer::new()),
        }
    }
    /// All Ready identities are borrowed from retained physical owners. This
    /// authorization is idempotent for the same original Boot/Source operation;
    /// no resources are admitted and no native effect may run here.
    #[allow(clippy::too_many_arguments)]
    pub fn authorize<
        P: PulseReadProvider,
        T: OpenDrain,
        C: RendererClock,
        M: MonotonicWaitProvider,
    >(
        &mut self,
        context: NativePreparationContext<'_>,
        policy: Policy<'_>,
        pulse: &ReadyPulseBase<P>,
        write: &ReadyWriteBase<T>,
        renderer: &ReadyRenderer<C>,
        waits: &[Option<ReadyMonotonicWaitBase<M>>; W],
        operations: &Operations<'_, W>,
        remaining: impl Fn() -> usize,
    ) -> Result<(), Refusal> {
        self.pulse.authorize(
            context,
            policy,
            &NativeFacts::pulse(pulse),
            &operations.pulse,
            "mcu/original-pulse",
            remaining(),
        )?;
        self.write.authorize(
            context,
            policy,
            &NativeFacts::write(write),
            &operations.write,
            "mcu/original-write",
            remaining(),
        )?;
        self.renderer.authorize(
            context,
            policy,
            &NativeFacts::renderer(renderer),
            &operations.renderer,
            "mcu/original-renderer",
            remaining(),
        )?;
        for (slot, (owner, operation)) in waits.iter().zip(&operations.waits).enumerate() {
            match (owner, operation) {
                (Some(owner), Some(operation)) => {
                    self.waits[slot].authorize(
                        context,
                        policy,
                        &NativeFacts::wait(owner),
                        operation,
                        owner.native_identity().provider_instance_id.as_str(),
                        remaining(),
                    )?;
                }
                (None, None) => {}
                _ => return Err(Refusal::Scope),
            }
        }
        Ok(())
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        core::iter::once(&self.recording)
            .chain(core::iter::once(&self.pulse))
            .chain(core::iter::once(&self.write))
            .chain(core::iter::once(&self.renderer))
            .chain(self.waits.iter())
            .try_fold(0usize, |n, i| n.checked_add(i.retained_backing_bytes()?))
    }
    /// Read the retained original descriptions in the same order as publication.
    /// Borrowing preserves the Issuers' custody and issues no native token.
    pub fn grants(&self) -> impl Iterator<Item = &AuthorityGrant> + Clone {
        core::iter::once(&self.recording)
            .chain(core::iter::once(&self.pulse))
            .chain(core::iter::once(&self.write))
            .chain(core::iter::once(&self.renderer))
            .chain(self.waits.iter())
            .filter_map(|issuer| issuer.grant())
    }
    /// Copies only retained original grants after funding the exact containing
    /// vector and fallible text allocations. Descriptions create no new tokens.
    pub fn copy_grants(&self, maximum_bytes: usize) -> Result<Vec<AuthorityGrant>, Refusal> {
        let count = self.grants().count();
        let mut budget = maximum_bytes;
        let bytes = count
            .checked_mul(core::mem::size_of::<AuthorityGrant>())
            .ok_or(Refusal::Bounds)?;
        if bytes > budget {
            return Err(Refusal::Bounds);
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| Refusal::Allocation)?;
        budget = budget
            .checked_sub(
                result
                    .capacity()
                    .checked_mul(core::mem::size_of::<AuthorityGrant>())
                    .ok_or(Refusal::Bounds)?,
            )
            .ok_or(Refusal::Bounds)?;
        for grant in self.grants() {
            let mut copy = |text: &str| -> Result<String, Refusal> {
                if text.len() > budget {
                    return Err(Refusal::Bounds);
                }
                let mut s = String::new();
                s.try_reserve_exact(text.len())
                    .map_err(|_| Refusal::Allocation)?;
                budget = budget.checked_sub(s.capacity()).ok_or(Refusal::Bounds)?;
                s.push_str(text);
                Ok(s)
            };
            result.push(AuthorityGrant {
                grant_id: copy(grant.grant_id.as_str())?.into(),
                contract_id: copy(grant.contract_id.as_str())?.into(),
                host_call_contract_id: copy(grant.host_call_contract_id.as_str())?.into(),
                subject_kind: copy(grant.subject_kind.as_str())?.into(),
                host_id: copy(grant.host_id.as_str())?.into(),
                boot_id: copy(grant.boot_id.as_str())?.into(),
                capability_id: copy(grant.capability_id.as_str())?.into(),
            });
        }
        Ok(result)
    }
}

/// Installed Source supplies a finite record count; receiving a Plan supplies none.
/// One operation owns the whole finite NOR residence, independently of call count.
pub fn append_operation<'a>(
    nodes: &'a [(u16, &'a str)],
    maximum_records: Option<u32>,
) -> Result<Option<Operation<'a>>, Refusal> {
    use conduit_resource_snapshot::{append::MAXIMUM_RECORD_PAYLOAD_BYTES, native_append::*};
    let mut selected = nodes.iter().filter(|n| n.1 == APPEND_KIND);
    let Some((_, kind)) = selected.next() else {
        return Ok(None);
    };
    if selected.next().is_some() {
        return Err(Refusal::Scope);
    }
    let count = maximum_records.filter(|n| *n != 0).ok_or(Refusal::Bounds)?;
    Ok(Some(Operation {
        kind,
        authority: APPEND_AUTHORITY,
        call: APPEND_OPERATION,
        maximum_parameter_bytes: MAXIMUM_RECORD_PAYLOAD_BYTES as u32,
        maximum_result_bytes: APPEND_RESULT_BYTES,
        // Two programs and four readbacks/preflights per append, independent
        // of physical capacity and the separately bounded initial recovery scan.
        maximum_work_units: 6,
        maximum_operations: count,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SAMPLE: &[(u16, &str)] = &[
        (2, conduit_pulse_capture::PULSE_READ_KIND),
        (5, conduit_time::TICK_KIND),
    ];
    const MASK: &[(u16, &str)] = &[(1, conduit_face::RENDERER_KIND)];
    const OUTPUT: &[(u16, &str)] = &[(0, conduit_tm1637_wire::WRITE_KIND)];
    #[test]
    fn source_operations_preserve_finite_cadence_and_refuse_unchecked_slots() {
        let ops = Operations::<2>::from_source(
            SAMPLE,
            MASK,
            OUTPUT,
            [None, Some((5, 60000, 12))],
            90,
            8192,
            8192,
        )
        .unwrap();
        assert_eq!(ops.pulse.maximum_operations, 12);
        assert_eq!(ops.write.maximum_operations, 90);
        assert_eq!(ops.waits[1].as_ref().unwrap().maximum_operations, 12);
        assert!(ops.waits[0].is_none());
        assert!(Operations::<1>::from_source(
            SAMPLE,
            MASK,
            OUTPUT,
            [Some((5, 0, 12))],
            90,
            8192,
            8192
        )
        .is_err());
        assert!(Operations::<1>::from_source(
            SAMPLE,
            MASK,
            OUTPUT,
            [Some((2, 60000, 12))],
            90,
            8192,
            8192
        )
        .is_err());
        assert!(Operations::<1>::from_source(
            SAMPLE,
            MASK,
            OUTPUT,
            [Some((5, 60000, 0))],
            90,
            8192,
            8192
        )
        .is_err());
        assert!(Operations::<1>::from_source(
            SAMPLE,
            MASK,
            OUTPUT,
            [Some((5, 60000, 12))],
            0,
            8192,
            8192
        )
        .is_err());
    }
}

#[path = "native_authority_continuation.rs"]
mod continuation;
