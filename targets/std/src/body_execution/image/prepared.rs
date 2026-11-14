//! One cooperative IMAGE counter owns synchronous Root preparation and result.
use super::*;

/// Affine retained preparation, not Source, adoption, provider or Play authority.
/// Only borrowed access is exposed; owned data drops before its original charge.
/// Captures, shared stores and other caller metadata need independent custody.
pub struct ImagePreparedValue<T> {
    value: T,
    memory: ImageMemoryLease,
}
/// Retains the original memory charge without retaining the prepared value.
/// Consumers must still own their data. This witness supplies neither access
/// nor authority and cannot reserve, enlarge or relabel the original budget.
#[derive(Clone)]
pub struct ImageMemoryRetention {
    _memory: ImageMemoryLease,
}

impl ImageMemoryRetention {
    pub(crate) fn verify_owner(&self, host: &StdHost) -> Result<(), String> {
        host.kernel_resources
            .require_input_memory_owner(&self._memory)
    }
}

/// Synchronous Root preparation against an already verified IMAGE counter.
/// Only the constructor can create this borrowed scope. It supplies memory
/// custody, never Source, provider, adoption or execution authority.
pub struct ImageRootPreparation<'a> {
    reserve: std::cell::RefCell<&'a mut dyn FnMut(usize) -> Result<ImageMemoryLease, String>>,
}
impl<'a> ImageRootPreparation<'a> {
    pub(crate) fn new(
        reserve: &'a mut dyn FnMut(usize) -> Result<ImageMemoryLease, String>,
    ) -> Self {
        Self {
            reserve: std::cell::RefCell::new(reserve),
        }
    }

    /// Reserve before effects and retain the result on the original counter.
    ///
    /// # Safety
    /// Root must independently qualify the complete synchronous factory peak,
    /// all retained backing/capacity and bounded destruction for the entire
    /// result lifetime. Shared stores/captures require independent custody;
    /// mutation must stay within the envelope, and effects/workers must quiesce
    /// before return, refusal or unwind. This is cooperative admission rather
    /// than allocator containment or permission to perform those effects.
    pub unsafe fn prepare_value<T>(
        &self,
        preparation_bytes: usize,
        retained_bytes: usize,
        prepare: impl FnOnce() -> Result<T, String>,
    ) -> Result<ImagePreparedValue<T>, String> {
        if preparation_bytes == 0
            || retained_bytes > preparation_bytes
            || retained_bytes < core::mem::size_of::<T>()
        {
            return Err("IMAGE preparation/retention envelope is invalid".into());
        }
        let metadata = ImageMemoryLease::owned_backing_bytes();
        let admitted = preparation_bytes
            .checked_add(metadata)
            .ok_or("IMAGE preparation size overflow")?;
        let mut memory = {
            let mut reserve = self.reserve.borrow_mut();
            (*reserve)(admitted)?
        };
        let value = prepare()?;
        memory.retain_result(retained_bytes + metadata);
        Ok(ImagePreparedValue { value, memory })
    }
}

impl<T> ImagePreparedValue<T> {
    // Trusted internal transfer keeps the value and its charge together.
    pub(crate) fn into_parts(self) -> (T, ImageMemoryLease) {
        (self.value, self.memory)
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    /// Keep this original charge while an independently owned export survives.
    /// Cloning the witness allocates nothing and charges no bytes a second time.
    pub fn retain_memory(&self) -> ImageMemoryRetention {
        ImageMemoryRetention {
            _memory: self.memory.clone(),
        }
    }

    pub(super) fn shrink_qualified_retention(&mut self, bytes: usize) {
        self.memory
            .retain_result(bytes + ImageMemoryLease::owned_backing_bytes());
    }
}
impl StdHost {
    /// Reserve on the original IMAGE counter before a Root factory borrows
    /// immutable facts from this same actual Host. No Host clone, mutable Host
    /// access, Source authority or additional memory grant is provided.
    ///
    /// # Safety
    /// The caller must independently qualify the complete factory peak,
    /// retained backing/capacity, lifetime mutation and bounded destruction,
    /// exactly as for `prepare_selected_image_value`. Captures and shared
    /// owners outside the result need independent custody. All effects and
    /// workers must quiesce before return, refusal or unwind.
    pub unsafe fn prepare_selected_image_value_with_host<T>(
        &mut self,
        preparation_bytes: usize,
        retained_bytes: usize,
        prepare: impl FnOnce(&Self) -> Result<T, String>,
    ) -> Result<ImagePreparedValue<T>, String> {
        if preparation_bytes == 0
            || retained_bytes > preparation_bytes
            || retained_bytes < core::mem::size_of::<T>()
        {
            return Err("IMAGE preparation/retention envelope is invalid".into());
        }
        let metadata = ImageMemoryLease::owned_backing_bytes();
        let admitted = preparation_bytes
            .checked_add(metadata)
            .ok_or("IMAGE preparation size overflow")?;
        let retained = retained_bytes
            .checked_add(metadata)
            .ok_or("IMAGE retention size overflow")?;
        let mut memory = self
            .kernel_resources
            .reserve_managed_base_memory(admitted)?;
        let value = prepare(self)?;
        memory.retain_result(retained);
        Ok(ImagePreparedValue { value, memory })
    }

    /// Verify the original memory owner, rather than copied identity labels.
    /// This verifies cooperative backing custody only.
    pub fn verify_selected_image_prepared_value<T>(
        &self,
        value: &ImagePreparedValue<T>,
    ) -> Result<(), String> {
        self.kernel_resources
            .require_input_memory_owner(&value.memory)
    }

    /// Verify the original counter of a retained shared-store charge. Copied
    /// Host/Boot labels cannot transfer custody to another Root in this process.
    pub fn verify_selected_image_memory_retention(
        &self,
        retention: &ImageMemoryRetention,
    ) -> Result<(), String> {
        retention.verify_owner(self)
    }

    /// Admit a complete synchronous preparation envelope before invoking Root's
    /// factory; retain its independently qualified result envelope until drop.
    /// Invalid bounds and pressure refuse before the factory runs. This uses
    /// the same counter as prepared inputs, providers and admitted execution.
    ///
    /// # Safety
    /// Root must independently qualify `preparation_bytes` for the complete
    /// factory peak and `retained_bytes` for all owned result backing, including
    /// allocated spare capacities and the value itself. The result must stay
    /// within that retained envelope for its entire lifetime, including bounded
    /// destruction; Root must constrain any interior or shared mutable backing.
    /// Shared backing and
    /// captures outside the factory need separate custody that outlives their
    /// use. All effects/workers must quiesce before return, refusal or unwinding.
    /// The envelope is cooperative admission, not allocator containment, process
    /// RSS, asynchronous retirement, or authority to check/adopt/execute Source.
    pub unsafe fn prepare_selected_image_value<T>(
        &mut self,
        preparation_bytes: usize,
        retained_bytes: usize,
        prepare: impl FnOnce() -> Result<T, String>,
    ) -> Result<ImagePreparedValue<T>, String> {
        // SAFETY: preserve the caller's complete preparation/lifetime contract;
        // the borrowed Host supplies no extra mutation or authority to prepare.
        unsafe {
            self.prepare_selected_image_value_with_host(preparation_bytes, retained_bytes, |_| {
                prepare()
            })
        }
    }
}

/// Memory-only Root preparation on the SAME original IMAGE counter while the
/// sole actual Host is loaned to a Play. No provider, operation, Source, Line or
/// action authority is exposed. No identity/deserialization constructor exists.
#[derive(Clone)]
pub struct ImageRootPreparationOwner {
    state: std::sync::Arc<RootPreparationState>,
}
struct RootPreparationState {
    active: std::sync::atomic::AtomicBool,
    counter: crate::kernel_preparation::ImagePreparationCounter,
    scope: [u8; 32],
    // All state backing drops before the charge for this original Arc owner.
    _memory: ImageMemoryLease,
}
impl ImageRootPreparationOwner {
    /// Root lifetime withdrawal prevents new preparations; previously prepared
    /// values and reply guards keep their original charges until actual drop.
    pub fn invalidate(&self) {
        self.state
            .active
            .store(false, std::sync::atomic::Ordering::Release);
    }
    pub fn verify_host(&self, host: &StdHost) -> Result<(), String> {
        self.require_active()?;
        let counter = host.kernel_resources.image_preparation_counter()?;
        if !self.state.counter.same_owner(&counter) || image_scope(host)? != self.state.scope {
            return Err("Root preparation belongs to another original IMAGE/Boot/counter".into());
        }
        Ok(())
    }
    pub fn verify_prepared<T>(&self, value: &ImagePreparedValue<T>) -> Result<(), String> {
        self.require_active()?;
        if !self.state.counter.owns(&value.memory) {
            return Err("Root prepared value belongs to another original memory owner".into());
        }
        Ok(())
    }
    pub fn verify_retention(&self, value: &ImageMemoryRetention) -> Result<(), String> {
        self.require_active()?;
        if !self.state.counter.owns(&value._memory) {
            return Err("Root retained value belongs to another original memory owner".into());
        }
        Ok(())
    }
    fn require_active(&self) -> Result<(), String> {
        if !self.state.active.load(std::sync::atomic::Ordering::Acquire) {
            return Err("Root preparation IMAGE lifetime is retired".into());
        }
        Ok(())
    }
    /// # Safety
    /// The complete synchronous factory peak, retained descendants/spare
    /// capacities, lifetime mutations and destruction must be independently
    /// qualified as for ImageRootPreparation::prepare_value. Captures/shared
    /// inputs require independent custody. Effects/workers must quiesce before
    /// return, refusal or unwind. A scalar is never a verified memory claim.
    pub unsafe fn prepare_value<T>(
        &self,
        peak: usize,
        retained: usize,
        prepare: impl FnOnce() -> Result<T, String>,
    ) -> Result<ImagePreparedValue<T>, String> {
        self.require_active()?;
        let mut reserve = |bytes| self.state.counter.reserve(bytes);
        let scope = ImageRootPreparation::new(&mut reserve);
        // SAFETY: the caller supplies the unchanged closed factory/lifetime
        // proof. The same original counter atomically arbitrates Play and Root.
        let value = unsafe {
            scope.prepare_value(peak, retained, || {
                self.require_active()?;
                prepare()
            })
        }?;
        self.require_active()?;
        Ok(value)
    }
}
fn image_scope(host: &StdHost) -> Result<[u8; 32], String> {
    use sha2::{Digest, Sha256};
    let image = host
        .image_identity()
        .ok_or("Root preparation requires checked IMAGE")?;
    let mut digest = Sha256::new();
    for text in [
        &image.profile_id,
        &image.build_id,
        &image.image_id,
        &image.host_id.0,
        &image.boot_id.0,
    ] {
        digest.update((text.len() as u64).to_le_bytes());
        digest.update(text.as_bytes());
    }
    digest.update(image.offer_generation.0.to_le_bytes());
    Ok(digest.finalize().into())
}
impl StdHost {
    /// Capture only from this live verified ledger. State metadata itself is
    /// reserved before Arc construction; cloning the handle allocates nothing.
    pub fn image_root_preparation_owner(&self) -> Result<ImageRootPreparationOwner, String> {
        let scope = image_scope(self)?;
        let counter = self.kernel_resources.image_preparation_counter()?;
        let bytes = core::mem::size_of::<RootPreparationState>()
            .checked_add(2 * core::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(ImageMemoryLease::owned_backing_bytes()))
            .ok_or("Root preparation metadata size overflow")?;
        let memory = counter.reserve(bytes)?;
        Ok(ImageRootPreparationOwner {
            state: std::sync::Arc::new(RootPreparationState {
                active: std::sync::atomic::AtomicBool::new(true),
                counter,
                scope,
                _memory: memory,
            }),
        })
    }
}
