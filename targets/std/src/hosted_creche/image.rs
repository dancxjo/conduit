//! Immutable Crèche context on the selected Host's original IMAGE counter.
use super::*;
use crate::body_execution::{ImagePreparedValue, ImageRootPreparation};
use crate::StdHost;

impl StdHost {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosted_creche::test_support::{planned_on_host, Adapter};
    use std::sync::atomic::Ordering;

    fn fixture() -> (StdHost, Adapter, BodyPlan, BodyPlayIdentity) {
        let adapter = Adapter::new();
        let mut host = StdHost::new_with_creche_encounter(
            crate::StdHostConfig {
                host_id: "host/image-creche-context".into(),
                boot_id: "boot/image-creche-context".into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            crate::StdHostComposition::minimal(),
            adapter.resource_offer(),
        )
        .unwrap();
        // Isolated memory-lifetime fixture, not selected IMAGE construction or
        // qualification of Crèche execution. The normal runner still refuses it.
        let catalog = conduit_workspace_make::catalog();
        let packages = conduit_workspace_make::package_set();
        let checked = conduit_host_make::check_host_configuration(
            conduit_host_make::parse_host_configuration_conduit(include_str!(
                "../../profiles/raspberry-pi-3-tick-workset.host.conduit"
            ))
            .unwrap(),
            &catalog,
            &packages,
        )
        .unwrap();
        let image = conduit_host_make::build_default_host_image(
            checked.profile().clone(),
            &catalog,
            &packages,
            &conduit_host_make::BuildInputs {
                source_identity: "fixture/image-creche-context".into(),
                toolchain_available: true,
            },
        )
        .unwrap()
        .0;
        host.kernel_resources
            .bind_image_bounds(&image.manifest.bounds)
            .unwrap();
        let (_, plan, play, _) = planned_on_host(host.advertisement().clone());
        (host, adapter, plan, play)
    }

    #[test]
    fn image_creche_context_retains_original_charge_and_does_not_enter_adapter() {
        let (mut host, adapter, plan, play) = fixture();
        let used = host.kernel_resources.image_memory_observer();
        let before = used.load(Ordering::Acquire);
        // SAFETY: fixed software fixture context fits this conservative envelope;
        // no external operation or adapter method is entered by preparation.
        let prepared = unsafe {
            host.prepare_selected_image_creche_context(
                &plan,
                &play,
                adapter.resource_offer(),
                adapter.limits(),
                4 * 1024 * 1024,
                2 * 1024 * 1024,
            )
        }
        .unwrap();
        host.verify_selected_image_prepared_value(&prepared)
            .unwrap();
        let (other, _, _, _) = fixture();
        assert!(other
            .verify_selected_image_prepared_value(&prepared)
            .is_err());
        assert!(used.load(Ordering::Acquire) >= before + 2 * 1024 * 1024);
        assert_eq!(prepared.value().host(), host.advertisement());
        assert_eq!(prepared.value().plan(), &plan);
        drop(host);
        assert_eq!(prepared.value().play(), &play);
        assert_eq!((adapter.begins, adapter.polls, adapter.drains), (0, 0, 0));
        drop(prepared);
        assert_eq!(used.load(Ordering::Acquire), before);
    }

    #[test]
    fn image_creche_context_is_borrowed_by_the_real_owner_without_a_second_copy() {
        let (mut host, mut adapter, plan, play) = fixture();
        // SAFETY: fixed context only; the fixture adapter retains its separate
        // bounded storage and has no asynchronous worker or external effects.
        let prepared = unsafe {
            host.prepare_selected_image_creche_context(
                &plan,
                &play,
                adapter.resource_offer(),
                adapter.limits(),
                4 * 1024 * 1024,
                2 * 1024 * 1024,
            )
        }
        .unwrap();
        let used = host.kernel_resources.image_memory_observer();
        let charged = used.load(Ordering::Acquire);
        let mut owner = CrecheHost::prepare_from_context(&mut adapter, prepared.value()).unwrap();
        assert!(core::ptr::eq(owner.context(), prepared.value()));
        assert!(owner.begin(owner.request()).is_err());
        owner.confirm_started(host.advertisement(), &play).unwrap();
        assert_eq!(owner.begin(owner.request()), Ok(CrechePoll::Pending));
        assert_eq!(used.load(Ordering::Acquire), charged);
        owner.retire().unwrap();
        drop(owner);
        assert_eq!((adapter.begins, adapter.drains), (1, 1));
        assert_eq!(used.load(Ordering::Acquire), charged);
        drop(prepared);
        assert_eq!(used.load(Ordering::Acquire), 0);
    }

    #[test]
    fn image_creche_context_refuses_pressure_and_stale_plan_without_retention() {
        let (mut host, adapter, plan, play) = fixture();
        let used = host.kernel_resources.image_memory_observer();
        let before = used.load(Ordering::Acquire);
        // SAFETY: pressure refuses before construction. The invalid prospective
        // identity has only bounded fixture validation and retains no context.
        unsafe {
            assert!(host
                .prepare_selected_image_creche_context(
                    &plan,
                    &play,
                    adapter.resource_offer(),
                    adapter.limits(),
                    65 * 1024 * 1024,
                    2 * 1024 * 1024,
                )
                .err()
                .unwrap()
                .contains("backing capacity"));
            assert!(host
                .prepare_selected_image_creche_context(
                    &plan,
                    &play,
                    adapter.resource_offer(),
                    adapter.limits(),
                    usize::MAX,
                    2 * 1024 * 1024,
                )
                .is_err());
            assert!(host
                .prepare_selected_image_creche_context(
                    &plan,
                    &BodyPlayIdentity::bind(&plan, 1),
                    adapter.resource_offer(),
                    CrecheLimits {
                        maximum_polls: 0,
                        ..adapter.limits()
                    },
                    4 * 1024 * 1024,
                    2 * 1024 * 1024,
                )
                .is_err());
        }
        let mut stale = play.clone();
        stale.plan_id = "plan/stale-creche".into();
        // SAFETY: fixed invalid fixture context is checked within these bounds.
        assert!(unsafe {
            host.prepare_selected_image_creche_context(
                &plan,
                &stale,
                adapter.resource_offer(),
                adapter.limits(),
                4 * 1024 * 1024,
                2 * 1024 * 1024,
            )
        }
        .is_err());
        assert_eq!(used.load(Ordering::Acquire), before);
        assert_eq!((adapter.begins, adapter.polls, adapter.drains), (0, 0, 0));
    }
}
