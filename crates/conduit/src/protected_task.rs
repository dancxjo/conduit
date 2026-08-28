use conduit_core::{ActivePlayId, BootId, CheckedFormId, HostId, PlanId, ResourceHandleId};
use conduit_std_host::IssuedKernelPlay;
use std::sync::mpsc::{self, TryRecvError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProtectedTaskIdentity {
    pub(crate) checked_form_id: CheckedFormId,
    pub(crate) plan_id: PlanId,
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
    pub(crate) request_id: String,
    pub(crate) resource_binding_ids: Vec<ResourceHandleId>,
}

pub(crate) trait StopRequest: Clone + Send + 'static {
    fn request_stop(&self);
}

pub(crate) trait ProtectedTaskReceipt: Send + 'static {
    fn request_id(&self) -> &str;
    fn plan_id(&self) -> &PlanId;
    fn play_id(&self) -> &ActivePlayId;
}

pub(crate) trait ProtectedTaskAdapter: Send + 'static {
    type Receipt: ProtectedTaskReceipt;
    type Stop: StopRequest;

    fn stop(&self) -> Self::Stop;
    fn execute_admitted_effects(
        self,
        request_id: &str,
        play: IssuedKernelPlay,
    ) -> Result<Self::Receipt, String>;
}

pub(crate) struct PreparedProtectedTask<A: ProtectedTaskAdapter> {
    identity: ProtectedTaskIdentity,
    form: conduit_form::CheckedForm,
    plan: conduit_core::Plan,
    play: IssuedKernelPlay,
    adapter: A,
}

impl<A: ProtectedTaskAdapter> PreparedProtectedTask<A> {
    pub(crate) fn new(
        request_id: impl Into<String>,
        resource_binding_ids: Vec<ResourceHandleId>,
        form: conduit_form::CheckedForm,
        plan: conduit_core::Plan,
        play: IssuedKernelPlay,
        adapter: A,
    ) -> Result<Self, String> {
        let request_id = request_id.into();
        if request_id.is_empty() || request_id.len() > 128 {
            return Err("protected task request identity must contain 1..=128 bytes".to_string());
        }
        if resource_binding_ids.is_empty() {
            return Err("protected task requires at least one exact resource binding".to_string());
        }
        if form.checked_form_id != plan.checked_form_id {
            return Err("protected task Form and Plan identities do not match".to_string());
        }
        let fragment = plan
            .fragments
            .first()
            .ok_or_else(|| "protected task Plan has no assigned fragment".to_string())?;
        if fragment.plan_id != plan.plan_id || fragment.checked_form_id != form.checked_form_id {
            return Err(
                "protected task assigned fragment does not match its immutable Plan".to_string(),
            );
        }
        let planned_bindings = fragment
            .placements
            .iter()
            .flat_map(|placement| &placement.resources)
            .filter_map(|binding| binding.protected.as_ref())
            .map(|binding| binding.handle_id.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let requested_bindings = resource_binding_ids
            .iter()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        if requested_bindings.len() != resource_binding_ids.len()
            || requested_bindings != planned_bindings
        {
            return Err(
                "protected task resource identities do not equal its immutable Plan bindings"
                    .to_string(),
            );
        }
        let identity = ProtectedTaskIdentity {
            checked_form_id: form.checked_form_id.clone(),
            plan_id: plan.plan_id.clone(),
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            request_id,
            resource_binding_ids,
        };
        let issued = play.identity();
        if issued.plan_id != identity.plan_id
            || issued.host_id != identity.host_id
            || issued.boot_id != identity.boot_id
        {
            return Err("protected task Play identity does not match its immutable Plan".into());
        }
        Ok(Self {
            identity,
            form,
            plan,
            play,
            adapter,
        })
    }

    pub(crate) fn identity(&self) -> &ProtectedTaskIdentity {
        &self.identity
    }

    pub(crate) fn form(&self) -> &conduit_form::CheckedForm {
        &self.form
    }

    pub(crate) fn plan(&self) -> &conduit_core::Plan {
        &self.plan
    }

    pub(crate) fn stop(&self) -> A::Stop {
        self.adapter.stop()
    }

    pub(crate) fn run(self) -> Result<A::Receipt, String> {
        let expected_play_id = self.play.identity().active_play_id.clone();
        let receipt = self
            .adapter
            .execute_admitted_effects(&self.identity.request_id, self.play)?;
        if receipt.request_id() != self.identity.request_id
            || receipt.plan_id() != &self.identity.plan_id
            || receipt.play_id() != &expected_play_id
        {
            return Err(
                "protected task terminal receipt has stale or mismatched identity".to_string(),
            );
        }
        Ok(receipt)
    }

    pub(crate) fn spawn(self) -> RunningProtectedTask<A::Receipt, A::Stop> {
        let stop = self.stop();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let _ = sender.send(self.run());
        });
        RunningProtectedTask { stop, receiver }
    }
}

pub(crate) enum TaskProgress<T> {
    Running,
    Complete(T),
}

pub(crate) struct RunningProtectedTask<R, S> {
    stop: S,
    receiver: mpsc::Receiver<Result<R, String>>,
}

impl<R, S: StopRequest> RunningProtectedTask<R, S> {
    pub(crate) fn request_stop(&self) {
        self.stop.request_stop();
    }

    pub(crate) fn try_receipt(&self) -> Result<TaskProgress<R>, String> {
        match self.receiver.try_recv() {
            Ok(receipt) => receipt.map(TaskProgress::Complete),
            Err(TryRecvError::Empty) => Ok(TaskProgress::Running),
            Err(TryRecvError::Disconnected) => {
                Err("protected task worker ended without a terminal receipt".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[derive(Clone, Default)]
    struct TestStop(Arc<AtomicBool>);

    impl StopRequest for TestStop {
        fn request_stop(&self) {
            self.0.store(true, Ordering::Release);
        }
    }

    struct CounterAdapter {
        stop: TestStop,
        stale_receipt: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CounterReceipt {
        request_id: String,
        plan_id: PlanId,
        play_id: ActivePlayId,
    }

    impl ProtectedTaskReceipt for CounterReceipt {
        fn request_id(&self) -> &str {
            &self.request_id
        }

        fn plan_id(&self) -> &PlanId {
            &self.plan_id
        }

        fn play_id(&self) -> &ActivePlayId {
            &self.play_id
        }
    }

    impl ProtectedTaskAdapter for CounterAdapter {
        type Receipt = CounterReceipt;
        type Stop = TestStop;

        fn stop(&self) -> Self::Stop {
            self.stop.clone()
        }

        fn execute_admitted_effects(
            self,
            request_id: &str,
            play: IssuedKernelPlay,
        ) -> Result<Self::Receipt, String> {
            while !self.stop.0.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            Ok(CounterReceipt {
                request_id: request_id.to_string(),
                plan_id: if self.stale_receipt {
                    PlanId::from("stale-plan")
                } else {
                    play.identity().plan_id.clone()
                },
                play_id: play.identity().active_play_id.clone(),
            })
        }
    }

    #[test]
    fn non_filesystem_adapter_prepares_runs_stops_and_returns_exact_receipt() {
        let mut host = conduit_std_host::StdHost::new();
        let prepared =
            conduit_std_host::prepare_copy_task(&host, &test_protected_grants(&host)).unwrap();
        let expected_checked_form_id = prepared.form.checked_form_id.clone();
        let expected_plan_id = prepared.plan.plan_id.clone();
        let expected_host_id = prepared.plan.fragments[0].host_id.clone();
        let expected_boot_id = prepared.plan.fragments[0].boot_id.clone();
        let resource_binding_ids = vec![
            ResourceHandleId::from("counter/source"),
            ResourceHandleId::from("counter/destination"),
        ];
        let play = host.issue_kernel_play(&prepared.fragment).unwrap();
        let expected_play_id = play.identity().active_play_id.clone();
        let task = PreparedProtectedTask::new(
            "counter/request-1",
            resource_binding_ids.clone(),
            prepared.form,
            prepared.plan,
            play,
            CounterAdapter {
                stop: TestStop::default(),
                stale_receipt: false,
            },
        )
        .unwrap();
        assert_eq!(task.identity().request_id, "counter/request-1");
        assert_eq!(task.identity().checked_form_id, expected_checked_form_id);
        assert_eq!(task.identity().plan_id, expected_plan_id);
        assert_eq!(task.identity().host_id, expected_host_id);
        assert_eq!(task.identity().boot_id, expected_boot_id);
        assert_eq!(task.identity().resource_binding_ids, resource_binding_ids);
        let running = task.spawn();
        running.request_stop();
        loop {
            match running.try_receipt().unwrap() {
                TaskProgress::Running => std::thread::yield_now(),
                TaskProgress::Complete(receipt) => {
                    assert_eq!(receipt.request_id, "counter/request-1");
                    assert_eq!(receipt.plan_id, expected_plan_id);
                    assert_eq!(receipt.play_id, expected_play_id);
                    break;
                }
            }
        }
    }

    #[test]
    fn adapter_contract_cannot_receive_form_plan_or_construct_play_identity() {
        let source = include_str!("protected_task.rs");
        let contract = source
            .split("pub(crate) trait ProtectedTaskAdapter")
            .nth(1)
            .unwrap()
            .split("pub(crate) struct PreparedProtectedTask")
            .next()
            .unwrap();
        assert!(!contract.contains("CheckedForm"));
        assert!(!contract.contains("Plan,"));
        assert!(!contract.contains("ProtectedTaskIdentity"));
        assert!(!contract.contains("ActivePlayId"));
        assert!(contract.contains("IssuedKernelPlay"));
    }

    #[test]
    fn terminal_receipt_cannot_replace_the_kernel_issued_plan_identity() {
        let mut host = conduit_std_host::StdHost::new();
        let prepared =
            conduit_std_host::prepare_copy_task(&host, &test_protected_grants(&host)).unwrap();
        let play = host.issue_kernel_play(&prepared.fragment).unwrap();
        let stop = TestStop::default();
        stop.request_stop();
        let task = PreparedProtectedTask::new(
            "counter/request-stale",
            vec![
                ResourceHandleId::from("counter/source"),
                ResourceHandleId::from("counter/destination"),
            ],
            prepared.form,
            prepared.plan,
            play,
            CounterAdapter {
                stop,
                stale_receipt: true,
            },
        )
        .unwrap();
        assert_eq!(
            task.run().unwrap_err(),
            "protected task terminal receipt has stale or mismatched identity"
        );
    }

    fn test_protected_grants(
        host: &conduit_std_host::StdHost,
    ) -> [conduit_core::ProtectedResourceGrant; 2] {
        use conduit_core::{
            CapabilityId, GearId, ProtectedResourceAccess, ProtectedResourceCommitPolicy,
            ResourceBindingRoleId, ResourceClassId,
        };
        let grant =
            |handle: &str, role: &str, access, policy| conduit_core::ProtectedResourceGrant {
                handle_id: ResourceHandleId::from(handle),
                class_id: ResourceClassId::from(
                    conduit_semantic_catalog::PROTECTED_FILE_RESOURCE_CLASS,
                ),
                gear_id: GearId::from("copy-task/task"),
                role_id: ResourceBindingRoleId::from(role),
                host_id: host.advertisement().host_id.clone(),
                boot_id: host.advertisement().boot_id.clone(),
                capability_id: CapabilityId::from(conduit_std_offers::COPY_FILE_CAPABILITY),
                access,
                maximum_bytes: 64,
                commit_policy: policy,
            };
        [
            grant(
                "counter/source",
                conduit_semantic_catalog::COPY_SOURCE_ROLE,
                ProtectedResourceAccess::ReadExisting,
                ProtectedResourceCommitPolicy::NotApplicable,
            ),
            grant(
                "counter/destination",
                conduit_semantic_catalog::COPY_DESTINATION_ROLE,
                ProtectedResourceAccess::Create,
                ProtectedResourceCommitPolicy::CreateOnly,
            ),
        ]
    }
}
