//! Public immutable model custody plus the existing compute lifecycle.
//! A concrete runtime owns its preparation, scheduler and opaque completion
//! receipts. Callers cannot supply a boolean warm/completion assertion.
use crate::{
    AdmittedModelResource, ModelComputeLifecycle, ModelComputeOffer, ModelComputeRefusal,
    ModelComputeRequirement, ModelComputeRuntimeIdentity, ModelComputeSession,
};
use alloc::sync::Arc;
use conduit_core::{Plan, verify_plan};

pub enum ModelComputeDriverProgress<T> {
    Pending,
    Complete(T),
}
#[derive(Debug)]
pub enum OwnedModelComputeRefusal<E> {
    Compute(ModelComputeRefusal),
    Driver(E),
    ForeignModel,
    ForeignSourcePlan,
    ForeignRuntimeOffer,
    Geometry,
    Stopped,
}
/// Provider implementation boundary. The selected concrete runtime must own its
/// original Source/Plan, all Native inputs and one live scheduler across polls.
/// Receipt types should have private constructors in that runtime. Resource
/// admission must charge complete working AND preparation storage before start;
/// an unknown component must refuse, never be treated as zero.
pub trait ModelComputeRuntimeDriver {
    type Error;
    type Batch;
    type ResourceReceipt;
    type PreparationReceipt;
    type WarmReceipt;
    type Completion;
    type StopReceipt;
    fn model(&self) -> &Arc<AdmittedModelResource>;
    fn source(&self) -> &Arc<str>;
    fn plan(&self) -> &Arc<Plan>;
    fn runtime(&self) -> &ModelComputeRuntimeIdentity;
    fn offer(&self) -> &ModelComputeOffer;
    /// Must refuse unknown preparation storage before Plan verification, Source
    /// hashing, offer cloning, or lifecycle construction. Includes their peak.
    fn admit_preparation(
        &self,
        requirement: &ModelComputeRequirement,
    ) -> Result<Self::PreparationReceipt, Self::Error>;
    fn validate_preparation(
        &self,
        receipt: &Self::PreparationReceipt,
        requirement: &ModelComputeRequirement,
    ) -> Result<(), Self::Error>;
    /// Fail-closed cleanup of these exact retained originals. Must not advance
    /// input/model state or act upon a replacement basis.
    fn abandon_original(
        &mut self,
        model: &Arc<AdmittedModelResource>,
        source: &Arc<str>,
        plan: &Arc<Plan>,
    ) -> Result<(), Self::Error>;
    fn admit_resources(
        &self,
        requirement: &ModelComputeRequirement,
    ) -> Result<Self::ResourceReceipt, Self::Error>;
    fn validate_resources(
        &self,
        receipt: &Self::ResourceReceipt,
        requirement: &ModelComputeRequirement,
    ) -> Result<(), Self::Error>;
    fn begin_warm(&mut self) -> Result<(), Self::Error>;
    fn poll_warm(&mut self) -> Result<ModelComputeDriverProgress<Self::WarmReceipt>, Self::Error>;
    fn validate_warm(&self, receipt: &Self::WarmReceipt) -> Result<(), Self::Error>;
    /// Returns exact canonical input bytes and frame count after Source admission.
    fn admit_batch(&self, batch: &Self::Batch) -> Result<(u64, u32), Self::Error>;
    /// On either outcome retain the complete original batch for audit.
    fn begin_batch(&mut self, batch: Self::Batch) -> Result<(), Self::Error>;
    fn poll(&mut self) -> Result<ModelComputeDriverProgress<Self::Completion>, Self::Error>;
    fn validate_completion(&self, receipt: &Self::Completion) -> Result<u64, Self::Error>;
    fn cancel(&mut self) -> Result<Self::StopReceipt, Self::Error>;
    fn provider_lost(&mut self) -> Result<Self::StopReceipt, Self::Error>;
    fn unload(&mut self) -> Result<(), Self::Error>;
}

pub struct OwnedModelComputeSession<D: ModelComputeRuntimeDriver> {
    original: Arc<AdmittedModelResource>,
    source: Arc<str>,
    plan: Arc<Plan>,
    offer: ModelComputeOffer,
    loaded: Option<Arc<AdmittedModelResource>>,
    lifecycle: ModelComputeSession,
    requirement: ModelComputeRequirement,
    driver: D,
    queued: Option<D::Batch>,
    resources: Option<D::ResourceReceipt>,
    preparation: D::PreparationReceipt,
    queued_bytes: u64,
    stopped: bool,
    cancellation_supported: bool,
}
impl<D: ModelComputeRuntimeDriver> OwnedModelComputeSession<D> {
    pub fn prepare(
        model: Arc<AdmittedModelResource>,
        offer: ModelComputeOffer,
        requirement: ModelComputeRequirement,
        runtime: ModelComputeRuntimeIdentity,
        driver: D,
    ) -> Result<Self, OwnedModelComputeRefusal<D::Error>> {
        if !Arc::ptr_eq(&model, driver.model())
            || requirement.model_bytes != model.bytes().len() as u64
            || requirement.model_format != model.artifact().format_profile
            || runtime.precision_profile != model.artifact().precision_profile
        {
            return Err(OwnedModelComputeRefusal::ForeignModel);
        }
        if &runtime != driver.runtime() || &offer != driver.offer() {
            return Err(OwnedModelComputeRefusal::ForeignRuntimeOffer);
        }
        let preparation = driver
            .admit_preparation(&requirement)
            .map_err(OwnedModelComputeRefusal::Driver)?;
        driver
            .validate_preparation(&preparation, &requirement)
            .map_err(OwnedModelComputeRefusal::Driver)?;
        if !verify_plan(driver.plan())
            || conduit_plot::syntax_source_document_identity(driver.source())
                != driver.plan().source_document_id
        {
            return Err(OwnedModelComputeRefusal::ForeignSourcePlan);
        }
        offer
            .admits(&requirement)
            .map_err(OwnedModelComputeRefusal::Compute)?;
        let original_offer = offer.clone();
        let source = Arc::clone(driver.source());
        let plan = Arc::clone(driver.plan());
        let cancellation_supported = offer.limits.cancellation_supported;
        let lifecycle = ModelComputeSession::discovered(offer, runtime)
            .map_err(OwnedModelComputeRefusal::Compute)?;
        Ok(Self {
            original: model,
            source,
            plan,
            offer: original_offer,
            loaded: None,
            lifecycle,
            requirement,
            driver,
            queued: None,
            resources: None,
            preparation,
            queued_bytes: 0,
            stopped: false,
            cancellation_supported,
        })
    }
    fn basis(&self) -> Result<(), OwnedModelComputeRefusal<D::Error>> {
        if !Arc::ptr_eq(&self.original, self.driver.model()) {
            return Err(OwnedModelComputeRefusal::ForeignModel);
        }
        if !Arc::ptr_eq(&self.source, self.driver.source())
            || !Arc::ptr_eq(&self.plan, self.driver.plan())
        {
            return Err(OwnedModelComputeRefusal::ForeignSourcePlan);
        }
        if self.lifecycle.runtime() != self.driver.runtime() || &self.offer != self.driver.offer() {
            return Err(OwnedModelComputeRefusal::ForeignRuntimeOffer);
        }
        // prepare checked this exact immutable Source/Plan once. Both this
        // session and the driver retain Arc clones, so safe mutation requires a
        // different allocation, which the pointer checks above refuse. Avoid
        // allocating a complete Plan verification on each scheduler poll.
        Ok(())
    }
    fn stop_basis(&mut self) -> Result<(), OwnedModelComputeRefusal<D::Error>> {
        if let Err(refusal) = self.basis() {
            // Invalidate the session even if cleanup fails; never emit a normal
            // stop/unload receipt for a replacement driver basis.
            self.stopped = true;
            self.lifecycle.provider_lost();
            self.loaded = None;
            self.queued = None;
            self.queued_bytes = 0;
            self.driver
                .abandon_original(&self.original, &self.source, &self.plan)
                .map_err(OwnedModelComputeRefusal::Driver)?;
            return Err(refusal);
        }
        Ok(())
    }
    /// Unknown working/preparation inventory refuses before load or warm starts.
    pub fn load(&mut self) -> Result<(), OwnedModelComputeRefusal<D::Error>> {
        if self.stopped {
            return Err(OwnedModelComputeRefusal::Stopped);
        }
        self.basis()?;
        let resources = self
            .driver
            .admit_resources(&self.requirement)
            .map_err(OwnedModelComputeRefusal::Driver)?;
        self.driver
            .validate_resources(&resources, &self.requirement)
            .map_err(OwnedModelComputeRefusal::Driver)?;
        self.lifecycle
            .begin_load(
                self.original.artifact().content_identity(),
                self.original.bytes().len() as u64,
            )
            .map_err(OwnedModelComputeRefusal::Compute)?;
        self.loaded = Some(Arc::clone(&self.original));
        self.resources = Some(resources);
        Ok(())
    }
    pub fn begin_warm(&mut self) -> Result<(), OwnedModelComputeRefusal<D::Error>> {
        if self.stopped {
            return Err(OwnedModelComputeRefusal::Stopped);
        }
        self.basis()?;
        let resources = self
            .resources
            .as_ref()
            .ok_or(OwnedModelComputeRefusal::Geometry)?;
        self.driver
            .validate_resources(resources, &self.requirement)
            .map_err(OwnedModelComputeRefusal::Driver)?;
        self.lifecycle
            .begin_warming()
            .map_err(OwnedModelComputeRefusal::Compute)?;
        if let Err(e) = self.driver.begin_warm() {
            self.stopped = true;
            return Err(OwnedModelComputeRefusal::Driver(e));
        }
        Ok(())
    }
    pub fn poll_warm(
        &mut self,
    ) -> Result<ModelComputeDriverProgress<D::WarmReceipt>, OwnedModelComputeRefusal<D::Error>>
    {
        if self.stopped {
            return Err(OwnedModelComputeRefusal::Stopped);
        }
        if self.lifecycle.state() != ModelComputeLifecycle::Warming {
            return Err(OwnedModelComputeRefusal::Compute(
                ModelComputeRefusal::InvalidLifecycleTransition,
            ));
        }
        self.basis()?;
        match self
            .driver
            .poll_warm()
            .map_err(OwnedModelComputeRefusal::Driver)?
        {
            ModelComputeDriverProgress::Pending => Ok(ModelComputeDriverProgress::Pending),
            ModelComputeDriverProgress::Complete(receipt) => {
                self.basis()?;
                self.driver
                    .validate_warm(&receipt)
                    .map_err(OwnedModelComputeRefusal::Driver)?;
                self.lifecycle
                    .ready()
                    .map_err(OwnedModelComputeRefusal::Compute)?;
                Ok(ModelComputeDriverProgress::Complete(receipt))
            }
        }
    }
    pub fn enqueue(&mut self, batch: D::Batch) -> Result<(), OwnedModelComputeRefusal<D::Error>> {
        if self.stopped {
            return Err(OwnedModelComputeRefusal::Stopped);
        }
        if self.queued.is_some() || self.lifecycle.state() != ModelComputeLifecycle::Ready {
            return Err(OwnedModelComputeRefusal::Compute(
                ModelComputeRefusal::InvalidLifecycleTransition,
            ));
        }
        self.basis()?;
        let (bytes, frames) = self
            .driver
            .admit_batch(&batch)
            .map_err(OwnedModelComputeRefusal::Driver)?;
        if bytes == 0
            || frames == 0
            || bytes > self.requirement.input_bytes
            || frames > self.requirement.batch_items
        {
            return Err(OwnedModelComputeRefusal::Geometry);
        }
        self.lifecycle
            .enqueue(bytes)
            .map_err(OwnedModelComputeRefusal::Compute)?;
        self.queued = Some(batch);
        self.queued_bytes = bytes;
        Ok(())
    }
    pub fn begin(&mut self) -> Result<(), OwnedModelComputeRefusal<D::Error>> {
        if self.stopped {
            return Err(OwnedModelComputeRefusal::Stopped);
        }
        self.basis()?;
        let resources = self
            .resources
            .as_ref()
            .ok_or(OwnedModelComputeRefusal::Geometry)?;
        self.driver
            .validate_resources(resources, &self.requirement)
            .map_err(OwnedModelComputeRefusal::Driver)?;
        if self.queued.is_none() {
            return Err(OwnedModelComputeRefusal::Geometry);
        }
        self.lifecycle
            .begin(&self.requirement, self.queued_bytes)
            .map_err(OwnedModelComputeRefusal::Compute)?;
        let batch = self
            .queued
            .take()
            .ok_or(OwnedModelComputeRefusal::Geometry)?;
        self.queued_bytes = 0;
        if let Err(e) = self.driver.begin_batch(batch) {
            self.stopped = true;
            return Err(OwnedModelComputeRefusal::Driver(e));
        }
        Ok(())
    }
    pub fn poll(
        &mut self,
    ) -> Result<ModelComputeDriverProgress<D::Completion>, OwnedModelComputeRefusal<D::Error>> {
        if self.stopped {
            return Err(OwnedModelComputeRefusal::Stopped);
        }
        if !matches!(self.lifecycle.state(), ModelComputeLifecycle::Active(_)) {
            return Err(OwnedModelComputeRefusal::Compute(
                ModelComputeRefusal::InvalidLifecycleTransition,
            ));
        }
        self.basis()?;
        match self
            .driver
            .poll()
            .map_err(OwnedModelComputeRefusal::Driver)?
        {
            ModelComputeDriverProgress::Pending => Ok(ModelComputeDriverProgress::Pending),
            ModelComputeDriverProgress::Complete(receipt) => {
                self.basis()?;
                let bytes = self
                    .driver
                    .validate_completion(&receipt)
                    .map_err(OwnedModelComputeRefusal::Driver)?;
                if bytes > self.requirement.output_bytes {
                    return Err(OwnedModelComputeRefusal::Geometry);
                }
                self.lifecycle
                    .finish()
                    .map_err(OwnedModelComputeRefusal::Compute)?;
                Ok(ModelComputeDriverProgress::Complete(receipt))
            }
        }
    }
    pub fn cancel(&mut self) -> Result<D::StopReceipt, OwnedModelComputeRefusal<D::Error>> {
        self.stop_basis()?;
        if !self.cancellation_supported {
            return Err(OwnedModelComputeRefusal::Compute(
                ModelComputeRefusal::CancellationUnsupported,
            ));
        }
        if self.stopped || !matches!(self.lifecycle.state(), ModelComputeLifecycle::Active(_)) {
            return Err(OwnedModelComputeRefusal::Stopped);
        }
        let receipt = self
            .driver
            .cancel()
            .map_err(OwnedModelComputeRefusal::Driver)?;
        self.lifecycle
            .cancel()
            .map_err(OwnedModelComputeRefusal::Compute)?;
        self.stopped = true;
        Ok(receipt)
    }
    pub fn provider_lost(&mut self) -> Result<D::StopReceipt, OwnedModelComputeRefusal<D::Error>> {
        self.stop_basis()?;
        let receipt = self
            .driver
            .provider_lost()
            .map_err(OwnedModelComputeRefusal::Driver)?;
        self.lifecycle.provider_lost();
        self.loaded = None;
        self.queued = None;
        self.queued_bytes = 0;
        self.stopped = true;
        Ok(receipt)
    }
    pub fn unload(&mut self) -> Result<(), OwnedModelComputeRefusal<D::Error>> {
        self.stop_basis()?;
        if self.queued.is_some() || self.lifecycle.state() != ModelComputeLifecycle::Ready {
            return Err(OwnedModelComputeRefusal::Compute(
                ModelComputeRefusal::InvalidLifecycleTransition,
            ));
        }
        self.driver
            .unload()
            .map_err(OwnedModelComputeRefusal::Driver)?;
        self.lifecycle
            .begin_unload()
            .map_err(OwnedModelComputeRefusal::Compute)?;
        self.loaded = None;
        self.lifecycle
            .shutdown()
            .map_err(OwnedModelComputeRefusal::Compute)?;
        self.stopped = true;
        Ok(())
    }
    pub fn state(&self) -> ModelComputeLifecycle {
        self.lifecycle.state()
    }
    pub fn model(&self) -> &Arc<AdmittedModelResource> {
        &self.original
    }
    pub fn source(&self) -> &str {
        self.source.as_ref()
    }
    pub fn plan(&self) -> &Plan {
        self.plan.as_ref()
    }
    pub fn runtime(&self) -> &ModelComputeRuntimeIdentity {
        self.lifecycle.runtime()
    }
    pub fn preparation(&self) -> &D::PreparationReceipt {
        &self.preparation
    }
    pub fn resources(&self) -> Option<&D::ResourceReceipt> {
        self.resources.as_ref()
    }
    pub fn loaded_model(&self) -> Option<&Arc<AdmittedModelResource>> {
        self.loaded.as_ref()
    }
}
