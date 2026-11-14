//! Immutable Crèche context on the selected Host's original IMAGE counter.
use super::*;
use crate::body_execution::{ImagePreparedValue, ImageRootPreparation};
use crate::StdHost;

impl StdHost {
    /// Fund owner construction and retained mutable lifecycle state before
    /// entering the adapter or serializing its exact request identity.
    ///
    /// # Safety
    /// Root must qualify the full preparation peak (including serialization,
    /// errors and drain) and owner lifetime backing. Adapter, worker, immutable
    /// context and kernel storage require independent custody. Mutation through
    /// this cell must stay within those qualified bounds. This grants no effects
    /// and does not enable the currently unsupported IMAGE execution profile.
    pub unsafe fn prepare_selected_image_creche_owner<'a>(
        &mut self,
        adapter: &'a mut dyn HostedCrecheAdapter,
        context: &'a ImagePreparedValue<CrecheExecutionContext>,
        preparation_bytes: usize,
        retained_bytes: usize,
    ) -> Result<ImagePreparedValue<std::cell::RefCell<CrecheHost<'a>>>, String> {
        self.verify_selected_image_prepared_value(context)?;
        if context.value().host() != self.advertisement() {
            return Err("Crèche IMAGE context differs from current producing Host".into());
        }
        // SAFETY: Root qualifies construction and every subsequent lifecycle
        // transition; references keep independently funded inputs alive.
        let mut uncertain = false;
        let result = unsafe {
            self.prepare_selected_image_value(preparation_bytes, retained_bytes, || {
                CrecheHost::prepare_context(
                    adapter,
                    Ok(ExecutionContext::Borrowed(context.value())),
                )
                .map(std::cell::RefCell::new)
                .map_err(|(error, drain_failed)| {
                    uncertain = drain_failed;
                    error
                })
            })
        };
        if uncertain {
            self.kernel_resources.quarantine_retirement(Vec::new());
        }
        let prepared = result?;
        prepared.value().borrow_mut().image_memory =
            Some((context.retain_memory(), prepared.retain_memory()));
        Ok(prepared)
    }

    /// Prepare the exact prospective Crèche context after reserving its complete
    /// workspace. No adapter is entered and no Play is started. The retained
    /// context keeps its charge even if this Host is dropped first.
    ///
    /// # Safety
    /// Root must independently qualify the complete validation/cloning peak and
    /// all retained context backing, including Plan and advertisement capacities.
    /// Caller-owned Plan/resource inputs require separate custody. These bounds
    /// are cooperative, not allocator containment. This does not qualify adapter
    /// preparation/effects, grant Source access or authorize Crèche execution.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn prepare_selected_image_creche_context(
        &mut self,
        plan: &BodyPlan,
        prospective: &BodyPlayIdentity,
        resource: ResourceOffer,
        limits: CrecheLimits,
        preparation_bytes: usize,
        retained_bytes: usize,
    ) -> Result<ImagePreparedValue<CrecheExecutionContext>, String> {
        self.kernel_resources.require_available()?;
        let host = &self.advertisement;
        let mut reserve = |bytes| self.kernel_resources.reserve_managed_base_memory(bytes);
        let scope = ImageRootPreparation::new(&mut reserve);
        // SAFETY: Root supplies the qualified complete peak and lifetime bound.
        unsafe {
            scope.prepare_value(preparation_bytes, retained_bytes, || {
                admission::context(host, plan, prospective, resource, limits)
            })
        }
    }
}

impl<'a> CrecheHost<'a> {
    pub(crate) fn verify_image_memory(&self, host: &StdHost) -> Result<(), String> {
        let (context, owner) = self
            .image_memory
            .as_ref()
            .ok_or("Crèche IMAGE owner lacks funded context preparation")?;
        context.verify_owner(host)?;
        owner.verify_owner(host)
    }

    /// Borrow the retained context only after checking its actual producing
    /// Host counter. Matching descriptive Host/Boot IDs alone do not suffice.
    /// Adapter, worker and kernel storage still need independent custody;
    /// this preparation does not authorize begin or enable IMAGE execution.
    pub fn prepare_from_image_context(
        host: &StdHost,
        adapter: &'a mut dyn HostedCrecheAdapter,
        context: &'a ImagePreparedValue<CrecheExecutionContext>,
    ) -> Result<Self, String> {
        host.verify_selected_image_prepared_value(context)?;
        if context.value().host() != host.advertisement() {
            return Err("Crèche IMAGE context differs from current producing Host".into());
        }
        Self::prepare_from_context(adapter, context.value())
    }
}

#[cfg(test)]
#[path = "image/tests.rs"]
mod tests;
