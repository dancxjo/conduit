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
        let prepared = unsafe {
            self.prepare_selected_image_value(preparation_bytes, retained_bytes, || {
                CrecheHost::prepare_from_context(adapter, context.value())
                    .map(std::cell::RefCell::new)
            })
        }?;
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
        let (other, _, _, _) = fixture();
        assert!(CrecheHost::prepare_from_image_context(&other, &mut adapter, &prepared).is_err());
        // A counter mismatch must not enter or cancel an untouched adapter.
        assert_eq!((adapter.begins, adapter.polls, adapter.drains), (0, 0, 0));
        // SAFETY: oversized reservation refuses before entering the adapter.
        assert!(unsafe {
            host.prepare_selected_image_creche_owner(
                &mut adapter,
                &prepared,
                65 * 1024 * 1024,
                256 * 1024,
            )
        }
        .is_err());
        assert_eq!(used.load(Ordering::Acquire), charged);
        assert_eq!((adapter.begins, adapter.polls, adapter.drains), (0, 0, 0));
        // SAFETY: fixed fixture lifecycle and request serialization fit this
        // independent envelope; the adapter has no worker or external effects.
        let prepared_owner = unsafe {
            host.prepare_selected_image_creche_owner(
                &mut adapter,
                &prepared,
                1024 * 1024,
                256 * 1024,
            )
        }
        .unwrap();
        let owner_charge = used.load(Ordering::Acquire);
        assert!(owner_charge >= charged + 256 * 1024);
        let mut owner = prepared_owner.value().borrow_mut();
        assert!(owner.verify_image_memory(&other).is_err());
        owner.verify_image_memory(&host).unwrap();
        assert!(core::ptr::eq(owner.context(), prepared.value()));
        let request = owner.request();
        assert!(owner.begin(request).is_err());
        owner.confirm_started(host.advertisement(), &play).unwrap();
        assert_eq!(owner.begin(request), Ok(CrechePoll::Pending));
        assert_eq!(used.load(Ordering::Acquire), owner_charge);
        owner.retire().unwrap();
        drop(owner);
        drop(prepared_owner);
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
    #[test]
    fn funded_creche_executes_through_selected_kernel_and_retains_result_charge() {
        use crate::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
        struct Accept;
        impl BodyForeOutputAdapter for Accept {
            fn deliver(&mut self, _: &crate::ExternalForeDelivery) -> Result<(), String> {
                Ok(())
            }
        }
        let (mut host, mut adapter, _, _) = fixture();
        let (_, plan, play, wake) = planned_on_host(host.advertisement().clone());
        // SAFETY: bounded software-only adapter and all outputs fit the retained
        // envelope; no worker, device or deployment effect is present. Fixture
        // construction is not evidence of selected production Host initialization.
        let external =
            unsafe { host.prepare_selected_image_value(1024 * 1024, 1024 * 1024, || Ok(())) }
                .unwrap();
        let context = unsafe {
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
        let owner = unsafe {
            host.prepare_selected_image_creche_owner(
                &mut adapter,
                &context,
                1024 * 1024,
                256 * 1024,
            )
        }
        .unwrap();
        let request = owner.value().borrow().request();
        let input = unsafe {
            host.prepare_selected_image_fore_input(4096, || {
                Ok(crate::ExternalForeInput {
                    front_port_id: "request".into(),
                    track: conduit_core::ConnectionTrack::Payload,
                    bytes: request.0.to_vec(),
                })
            })
        }
        .unwrap();
        let used = host.kernel_resources.image_memory_observer();
        let before = used.load(Ordering::Acquire);
        let control = crate::RunControl::default();
        let report = unsafe {
            host.run_selected_image_body_plan_with_creche_to_with_start(
                BodyRunRequest {
                    wake: &wake,
                    plan: &plan,
                    control: &control,
                    keyboard: None,
                },
                &input,
                &owner,
                &mut Vec::new(),
                &mut crate::ThreadTimer,
                &mut Accept,
                |issued, _| {
                    assert_eq!(issued, &play);
                    Ok(())
                },
            )
        }
        .unwrap();
        assert_eq!(
            report.report().terminal,
            conduit_core::TerminalDisposition::Completed,
            "failure={:?}, cleanup={:?}",
            report.report().failure,
            report.report().cleanup_failure
        );
        assert_eq!(report.report().fore_deliveries.len(), 1);
        assert!(used.load(Ordering::Acquire) > before);
        let outcome = conduit_body_make::deployment_runtime::CrecheOutcome::decode(
            &report.report().fore_deliveries[0].bytes,
        )
        .unwrap();
        assert_eq!(outcome.request, request);
        drop(report);
        assert_eq!(used.load(Ordering::Acquire), before);
        drop(owner);
        assert_eq!((adapter.begins, adapter.polls, adapter.drains), (1, 1, 1));
        drop(input);
        drop(context);
        drop(external);
        assert_eq!(used.load(Ordering::Acquire), 0);
    }
    #[test]
    fn selected_creche_preparation_refusal_drains_and_quarantines_uncertainty() {
        use crate::body_execution::{BodyForeOutputAdapter, BodyRunRequest};
        struct NoOutput;
        impl BodyForeOutputAdapter for NoOutput {
            fn deliver(&mut self, _: &crate::ExternalForeDelivery) -> Result<(), String> {
                panic!("preparation refusal cannot deliver")
            }
        }
        for uncertain in [false, true] {
            let (mut host, mut adapter, _, _) = fixture();
            adapter.fail_drain = uncertain;
            let (_, plan, play, wake) = planned_on_host(host.advertisement().clone());
            // SAFETY: bounded software fixture, no worker/device effects; the
            // deliberate failed drain must leave this Host unavailable.
            let external =
                unsafe { host.prepare_selected_image_value(1024 * 1024, 1024 * 1024, || Ok(())) }
                    .unwrap();
            let context = unsafe {
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
            let owner = unsafe {
                host.prepare_selected_image_creche_owner(
                    &mut adapter,
                    &context,
                    1024 * 1024,
                    256 * 1024,
                )
            }
            .unwrap();
            let input = unsafe {
                host.prepare_selected_image_fore_input(4096, || {
                    Ok(crate::ExternalForeInput {
                        front_port_id: "not-the-selected-request".into(),
                        track: conduit_core::ConnectionTrack::Payload,
                        bytes: owner.value().borrow().request().0.to_vec(),
                    })
                })
            }
            .unwrap();
            let control = crate::RunControl::default();
            let error = unsafe {
                host.run_selected_image_body_plan_with_creche_to_with_start(
                    BodyRunRequest {
                        wake: &wake,
                        plan: &plan,
                        control: &control,
                        keyboard: None,
                    },
                    &input,
                    &owner,
                    &mut Vec::new(),
                    &mut crate::ThreadTimer,
                    &mut NoOutput,
                    |_, _| panic!("refusal must precede Play"),
                )
            }
            .err()
            .expect("wrong Fore must refuse");
            assert_eq!(
                host.kernel_resources.require_available().is_err(),
                uncertain
            );
            assert_eq!(error.contains("pre-start retirement"), uncertain);
            drop(owner);
            assert_eq!((adapter.begins, adapter.polls), (0, 0));
            assert!(adapter.drains >= 1);
            drop(external);
        }
    }
}
