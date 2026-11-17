//! Original selected IMAGE backing custody, independent of resource tokens.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Debug)]
pub(crate) struct BodyMemoryReceipt {
    total_bytes: usize,
}
impl BodyMemoryReceipt {
    pub(crate) fn prepared_tick(total_bytes: usize) -> Self {
        Self { total_bytes }
    }
    pub(crate) fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}
#[derive(Debug, Clone)]
pub(crate) struct ImageMemoryLease {
    inner: Arc<ImageMemoryCharge>,
}
#[derive(Debug)]
struct ImageMemoryCharge {
    used: Arc<AtomicUsize>,
    bytes: AtomicUsize,
}
impl Drop for ImageMemoryCharge {
    fn drop(&mut self) {
        self.used
            .fetch_sub(self.bytes.load(Ordering::Acquire), Ordering::AcqRel);
    }
}
impl ImageMemoryLease {
    pub(crate) fn owned_backing_bytes() -> usize {
        core::mem::size_of::<ImageMemoryCharge>() + 2 * core::mem::size_of::<usize>()
    }
    pub(crate) fn reserved_bytes(&self) -> usize {
        self.inner.bytes.load(Ordering::Acquire)
    }
    pub(crate) fn retain_result(&mut self, bytes: usize) {
        let admitted = self.reserved_bytes();
        assert!(bytes <= admitted, "result exceeds original funded backing");
        self.inner.bytes.store(bytes, Ordering::Release);
        self.inner
            .used
            .fetch_sub(admitted - bytes, Ordering::AcqRel);
    }
}
impl KernelResourceLedger {
    pub(crate) fn has_image_bounds(&self) -> bool {
        self.image_bounds.is_some()
    }
    pub(crate) fn require_input_memory_owner(
        &self,
        lease: &ImageMemoryLease,
    ) -> Result<(), String> {
        self.require_available()?;
        if self
            .image_memory
            .as_ref()
            .is_some_and(|used| Arc::ptr_eq(used, &lease.inner.used))
        {
            Ok(())
        } else {
            Err("IMAGE backing belongs to a different original counter".into())
        }
    }
    /// Temporary ledger clone and all retained reservation vectors coexist with the kernel.
    pub(crate) fn tick_reservation_backing_bytes(
        &self,
        plans: &[(&Plan, bool)],
    ) -> Result<usize, String> {
        let add = |a: usize, b: usize| {
            a.checked_add(b)
                .ok_or_else(|| "IMAGE reservation backing overflow".to_string())
        };
        let mut bytes = self
            .pools
            .len()
            .checked_mul(core::mem::size_of::<PoolUsage>())
            .ok_or("IMAGE pool backing overflow")?;
        for pool in &self.pools {
            bytes = add(
                bytes,
                pool.tick_clone_backing_bytes()
                    .ok_or("IMAGE Tick cannot clone compute pools")?,
            )?;
        }
        bytes = add(
            bytes,
            self.instances
                .len()
                .checked_mul(core::mem::size_of::<(conduit_core::CapabilityId, u16)>())
                .ok_or("IMAGE capability backing overflow")?,
        )?;
        for (id, _) in &self.instances {
            bytes = add(bytes, id.0.len())?;
        }
        bytes = add(
            bytes,
            plans
                .len()
                .checked_mul(core::mem::size_of::<KernelResourceReservation>())
                .ok_or("IMAGE reservation vector overflow")?,
        )?;
        for (plan, _) in plans {
            bytes = add(bytes, plan.plan_id.0.len())?;
            bytes = add(
                bytes,
                self.instances
                    .len()
                    .checked_mul(core::mem::size_of::<(conduit_core::CapabilityId, u16)>())
                    .ok_or("IMAGE instance reservation overflow")?,
            )?;
            for (id, _) in &self.instances {
                bytes = add(bytes, id.0.len())?;
            }
            for placement in plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
            {
                for binding in &placement.resources {
                    if binding.compute.is_some()
                        || binding.protected.is_some()
                        || binding.content.is_some()
                    {
                        return Err("IMAGE Tick resource payload is unsupported".into());
                    }
                    bytes = add(bytes, core::mem::size_of::<ResourceBinding>())?;
                    bytes = add(bytes, binding.pool_id.0.len())?;
                    bytes = add(bytes, binding.class_id.0.len())?;
                }
            }
        }
        Ok(bytes)
    }

    /// Called only with the closed borrowed-Plan preparation footprint.
    pub(crate) fn fund_tick_preparation(&self, bytes: usize) -> Result<ImageMemoryLease, String> {
        self.require_available()?;
        let bounds = self
            .image_bounds
            .as_ref()
            .ok_or("selected IMAGE bounds missing")?;
        let limit = usize::try_from(bounds.heap_arena_bytes)
            .map_err(|_| "IMAGE heap exceeds local size")?;
        let used = self
            .image_memory
            .as_ref()
            .ok_or("original IMAGE counter missing")?;
        reserve_image_memory(used, limit, bytes)
    }
    pub(crate) fn reserve_funded_body(
        &mut self,
        advertisement: &HostAdvertisement,
        plans: &[(&Plan, bool)],
        receipt: &BodyMemoryReceipt,
        lease: &ImageMemoryLease,
    ) -> Result<Vec<KernelResourceReservation>, String> {
        self.require_input_memory_owner(lease)?;
        if receipt.total_bytes() > lease.reserved_bytes() {
            return Err("prepared Tick exceeds its admitted footprint".into());
        }
        if plans.is_empty() || plans.len() > conduit_body::MAX_BODY_PLOTS {
            return Err("funded Body partition count exceeds the admitted profile".into());
        }
        self.preflight_image_shape(plans.iter().flat_map(|(plan, _)| &plan.fragments))?;
        // A separate local candidate carries the same original counter. There
        // is no mutable admission flag that could survive a Result or unwind.
        let mut staged = self.clone();
        let mut reservations = Vec::with_capacity(plans.len());
        for (index, (plan, continuity)) in plans.iter().enumerate() {
            if *continuity || plan.fragments.len() != 1 {
                return Err("funded Tick requires one fresh local fragment".into());
            }
            let fragment = &plan.fragments[0];
            if plans[..index].iter().any(|(prior, _)| {
                prior.fragments.iter().any(|part| {
                    part.plan_id == fragment.plan_id && part.fragment_id == fragment.fragment_id
                })
            }) {
                return Err("duplicate funded Body partition".into());
            }
            let mut lowered = conduit_plan_lowering::fragment_set::lower_local_fragment_set_from_plans_with_limit(
                &[*plan], conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE,
                crate::installed_std::body_kernel::funded::bounds(), lease.reserved_bytes(),
            ).map_err(|_| "funded Body resource lowering refused")?;
            let lowered = lowered
                .partitions
                .pop()
                .ok_or("funded Body resource partition missing")?;
            validate_exact_profile(advertisement, fragment)?;
            reservations.push(staged.reserve_funded_placement_resources(
                advertisement,
                fragment,
                &lowered,
            )?);
        }
        for (live, admitted) in self.pools.iter_mut().zip(staged.pools) {
            live.copy_usage_from(&admitted);
        }
        for (live, admitted) in self.instances.iter_mut().zip(staged.instances) {
            live.1 = admitted.1;
        }
        Ok(reservations)
    }
    #[cfg(test)]
    pub(crate) fn image_memory_observer(&self) -> Arc<AtomicUsize> {
        self.image_memory.as_ref().unwrap().clone()
    }
}

pub(super) fn reserve_image_memory(
    used: &Arc<AtomicUsize>,
    limit: usize,
    bytes: usize,
) -> Result<ImageMemoryLease, String> {
    if bytes < ImageMemoryLease::owned_backing_bytes() {
        return Err("IMAGE footprint omits lease backing".into());
    }
    used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |prior| {
        prior.checked_add(bytes).filter(|next| *next <= limit)
    })
    .map_err(|_| "selected IMAGE managed backing capacity is unavailable")?;
    Ok(ImageMemoryLease {
        inner: Arc::new(ImageMemoryCharge {
            used: used.clone(),
            bytes: AtomicUsize::new(bytes),
        }),
    })
}
