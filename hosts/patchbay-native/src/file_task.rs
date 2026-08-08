//! Optional native file-choice provider and ordinary protected copy task adapter.

use conduit_core::{
    BootId, CapabilityId, HostId, OfferGeneration, OperationId, ProtectedResourceAccess,
    ProtectedResourceCommitPolicy, ProtectedResourceGrant, ResourceBindingRoleId, ResourceHandleId,
};
use conduit_std_host::{
    prepare_copy_task, CopyRequestId, CopyRunReceipt, CopyStopToken, PreparedCopyTask,
    ProtectedFileAvailability, ProtectedFileRegistry, StdHost, StdHostComposition, StdHostConfig,
};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, TryRecvError};

const MAXIMUM_COPY_BYTES: u64 = 16 * 1024 * 1024;
const SOURCE_HANDLE: &str = "patchbay-native/file/source";
const DESTINATION_HANDLE: &str = "patchbay-native/file/destination";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationPolicy {
    Create,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChoiceDisposition {
    Selected(ResourceHandleId),
    Cancelled,
}

enum DialogBackend {
    Command(PathBuf),
    #[cfg(test)]
    Scripted(VecDeque<Result<Option<PathBuf>, String>>),
}

pub struct NativeFileProvider {
    backend: DialogBackend,
}

impl NativeFileProvider {
    pub fn probe(program: impl AsRef<Path>) -> Result<Self, String> {
        let program = program.as_ref();
        if std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_none() {
            return Err("native file provider has no display connection".into());
        }
        let output = Command::new(program)
            .arg("--version")
            .output()
            .map_err(|error| format!("native file provider unavailable: {error}"))?;
        if !output.status.success() {
            return Err("native file provider failed its usability probe".into());
        }
        Ok(Self {
            backend: DialogBackend::Command(program.to_path_buf()),
        })
    }

    fn choose(&mut self, destination: bool) -> Result<Option<PathBuf>, String> {
        match &mut self.backend {
            DialogBackend::Command(program) => {
                let mut command = Command::new(program);
                command.arg("--file-selection");
                command.arg(if destination {
                    "--title=Choose protected copy destination"
                } else {
                    "--title=Choose protected copy source"
                });
                if destination {
                    command.arg("--save");
                }
                let output = command
                    .output()
                    .map_err(|error| format!("native file choice failed: {error}"))?;
                if output.status.code() == Some(1) {
                    return Ok(None);
                }
                if !output.status.success() {
                    return Err(format!(
                        "native file provider failed with status {:?}",
                        output.status.code()
                    ));
                }
                let selected = String::from_utf8(output.stdout)
                    .map_err(|_| "native file provider returned a non-UTF-8 locator")?;
                let selected = selected.trim_end_matches(['\r', '\n']);
                if selected.is_empty() || selected.len() > 4096 {
                    return Err("native file provider returned an invalid bounded locator".into());
                }
                Ok(Some(PathBuf::from(selected)))
            }
            #[cfg(test)]
            DialogBackend::Scripted(choices) => choices
                .pop_front()
                .ok_or_else(|| "scripted native provider exhausted".to_string())?,
        }
    }
}

type CopyWorkerResult = Result<CopyRunReceipt, String>;

pub struct NativeFileTask {
    provider: Option<NativeFileProvider>,
    config: StdHostConfig,
    host: StdHost,
    registry: ProtectedFileRegistry,
    source: Option<ProtectedResourceGrant>,
    destination: Option<ProtectedResourceGrant>,
    prepared: Option<PreparedCopyTask>,
    receipt: Option<CopyRunReceipt>,
    active: Option<(CopyStopToken, Receiver<CopyWorkerResult>)>,
    request_sequence: u64,
    events: VecDeque<String>,
}

pub fn probe_native_file_provider() -> Option<NativeFileProvider> {
    NativeFileProvider::probe("zenity").ok()
}

impl NativeFileTask {
    #[cfg(test)]
    pub fn new(provider: Option<NativeFileProvider>) -> Self {
        let composition = if provider.is_some() {
            StdHostComposition::minimal().with_files()
        } else {
            StdHostComposition::minimal()
        };
        Self::for_host(
            provider,
            HostId::from("patchbay-native/file-host"),
            BootId::from("patchbay-native/file-boot-1"),
            composition,
        )
    }

    pub fn for_host(
        provider: Option<NativeFileProvider>,
        host_id: HostId,
        boot_id: BootId,
        composition: StdHostComposition,
    ) -> Self {
        let config = StdHostConfig {
            host_id,
            boot_id,
            offer_generation: OfferGeneration(1),
        };
        let host = StdHost::new_with_composition(config.clone(), composition);
        Self {
            provider,
            config,
            host,
            registry: ProtectedFileRegistry::default(),
            source: None,
            destination: None,
            prepared: None,
            receipt: None,
            active: None,
            request_sequence: 0,
            events: VecDeque::with_capacity(24),
        }
    }

    pub fn choose_source(&mut self) -> Result<ChoiceDisposition, String> {
        let path = match self.provider_mut()?.choose(false)? {
            Some(path) => path,
            None => {
                self.record("FILE-CHOICE role=source disposition=Cancelled".into());
                return Ok(ChoiceDisposition::Cancelled);
            }
        };
        let handle = ResourceHandleId::from(SOURCE_HANDLE);
        self.registry.revoke(&handle);
        let grant = self.registry.register(
            handle.clone(),
            path,
            OperationId::from("copy"),
            ResourceBindingRoleId::from(conduit_std_catalog::COPY_SOURCE_ROLE),
            self.config.host_id.clone(),
            self.config.boot_id.clone(),
            CapabilityId::from(conduit_std_catalog::COPY_FILE_CAPABILITY),
            ProtectedResourceAccess::ReadExisting,
            MAXIMUM_COPY_BYTES,
            ProtectedResourceCommitPolicy::NotApplicable,
            ProtectedFileAvailability::Available,
        )?;
        self.source = Some(grant);
        self.invalidate_plan();
        self.record(format!("FILE-CHOICE role=source handle={SOURCE_HANDLE}"));
        Ok(ChoiceDisposition::Selected(handle))
    }

    pub fn run_choice_demo(&mut self) -> Result<(), String> {
        if self.choose_source()? == ChoiceDisposition::Cancelled {
            return Err("native copy demonstration source choice was cancelled".into());
        }
        if self.choose_destination(DestinationPolicy::Create)? == ChoiceDisposition::Cancelled {
            return Err("native copy demonstration destination choice was cancelled".into());
        }
        self.plan()?;
        self.run()
    }

    pub fn choose_destination(
        &mut self,
        policy: DestinationPolicy,
    ) -> Result<ChoiceDisposition, String> {
        let path = match self.provider_mut()?.choose(true)? {
            Some(path) => path,
            None => {
                self.record("FILE-CHOICE role=destination disposition=Cancelled".into());
                return Ok(ChoiceDisposition::Cancelled);
            }
        };
        let (access, commit_policy) = match policy {
            DestinationPolicy::Create => (
                ProtectedResourceAccess::Create,
                ProtectedResourceCommitPolicy::CreateOnly,
            ),
            DestinationPolicy::Replace => (
                ProtectedResourceAccess::Replace,
                ProtectedResourceCommitPolicy::ReplaceExisting,
            ),
        };
        let handle = ResourceHandleId::from(DESTINATION_HANDLE);
        self.registry.revoke(&handle);
        let grant = self.registry.register(
            handle.clone(),
            path,
            OperationId::from("copy"),
            ResourceBindingRoleId::from(conduit_std_catalog::COPY_DESTINATION_ROLE),
            self.config.host_id.clone(),
            self.config.boot_id.clone(),
            CapabilityId::from(conduit_std_catalog::COPY_FILE_CAPABILITY),
            access,
            MAXIMUM_COPY_BYTES,
            commit_policy,
            ProtectedFileAvailability::Available,
        )?;
        self.destination = Some(grant);
        self.invalidate_plan();
        self.record(format!(
            "FILE-CHOICE role=destination handle={DESTINATION_HANDLE} policy={policy:?}"
        ));
        Ok(ChoiceDisposition::Selected(handle))
    }

    pub fn plan(&mut self) -> Result<(), String> {
        if self.active.is_some() {
            return Err("cannot replace a copy Plan while its Play is active".into());
        }
        let grants = [
            self.source
                .clone()
                .ok_or("copy source has not been chosen")?,
            self.destination
                .clone()
                .ok_or("copy destination has not been chosen")?,
        ];
        let prepared = prepare_copy_task(&self.host, &grants)?;
        self.record(format!(
            "FILE-PLAN checked={} plan={} source={} destination={}",
            prepared.form.checked_form_id.as_str(),
            prepared.plan.plan_id.as_str(),
            SOURCE_HANDLE,
            DESTINATION_HANDLE
        ));
        self.prepared = Some(prepared);
        self.receipt = None;
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), String> {
        if self.active.is_some() {
            return Err("a copy Play is already active".into());
        }
        let prepared = self
            .prepared
            .as_ref()
            .ok_or("copy requires an exact Plan")?;
        let fragment = prepared.fragment.clone();
        let plan_id = prepared.plan.plan_id.clone();
        let registry = std::mem::take(&mut self.registry);
        let stop = CopyStopToken::default();
        let worker_stop = stop.clone();
        let config = self.config.clone();
        let request = CopyRequestId::new(format!("patchbay/file-copy/{}", self.request_sequence))?;
        self.request_sequence = self.request_sequence.saturating_add(1);
        let request_text = request.as_str().to_string();
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut host =
                StdHost::new_with_composition(config, StdHostComposition::minimal().with_files());
            let result = host.run_copy_fragment(request, fragment, &registry, &worker_stop);
            let _ = sender.send(result);
        });
        self.record(format!(
            "FILE-RUN request={request_text} plan={} terminal=pending",
            plan_id.as_str()
        ));
        self.active = Some((stop, receiver));
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        let request = format!("patchbay/file-stop/{}", self.request_sequence);
        self.request_sequence = self.request_sequence.saturating_add(1);
        let result = match self.active.as_ref() {
            Some((stop, _)) => {
                stop.request_stop();
                Ok(())
            }
            None => Err("no copy Play is active".to_string()),
        };
        match &result {
            Ok(()) => self.record(format!(
                "FILE-STOP request={request} disposition=Accepted terminal=pending"
            )),
            Err(error) => self.record(format!(
                "FILE-STOP request={request} disposition=Rejected reason={error}"
            )),
        }
        result
    }

    pub fn poll(&mut self) -> Result<bool, String> {
        let Some((_, receiver)) = self.active.as_ref() else {
            return Ok(false);
        };
        match receiver.try_recv() {
            Ok(result) => {
                let receipt = result?;
                self.record(format!(
                    "FILE-RECEIPT request={} play={} plan={} source={} destination={} result={:?} kernel-events={}",
                    receipt.request_id.as_str(),
                    receipt.run_id.as_str(),
                    receipt.plan_id.as_str(),
                    receipt.source_binding_id.as_str(),
                    receipt.destination_binding_id.as_str(),
                    receipt.result,
                    receipt.kernel_events
                ));
                self.receipt = Some(receipt);
                self.active = None;
                Ok(true)
            }
            Err(TryRecvError::Empty) => Ok(false),
            Err(TryRecvError::Disconnected) => {
                self.active = None;
                Err("copy worker ended without a receipt".into())
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.active.is_some()
    }

    #[cfg(test)]
    pub fn host_identity(&self) -> (&HostId, &BootId) {
        (&self.config.host_id, &self.config.boot_id)
    }

    #[cfg(test)]
    pub fn provider_available(&self) -> bool {
        self.provider.is_some()
    }

    pub fn lines(&self) -> Vec<String> {
        let advertised =
            self.host.advertisement().capabilities.iter().any(|offer| {
                offer.capability_id.as_str() == conduit_std_catalog::COPY_FILE_CAPABILITY
            });
        let mut lines = vec![format!(
            "NATIVE-FILE-PROVIDER usable={} capability-advertised={advertised}",
            self.provider.is_some()
        )];
        lines.extend(self.events.iter().cloned());
        lines
    }

    fn provider_mut(&mut self) -> Result<&mut NativeFileProvider, String> {
        self.provider
            .as_mut()
            .ok_or_else(|| "native file provider is unavailable; no grant was created".into())
    }

    fn invalidate_plan(&mut self) {
        self.prepared = None;
        self.receipt = None;
    }

    fn record(&mut self, event: String) {
        if self.events.len() == 24 {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

#[cfg(test)]
#[path = "file_task_tests.rs"]
mod tests;
