//! Root-owned native preparation. No radio effect can be called here: the
//! prepared provider is exposed only through its exact-selection Base owner.
#[path = "native_network_recovery.rs"]
pub(crate) mod recovery;
use conduit_core as c;
use conduit_esp32_network_switch::{Esp32ApStationSwitch, Esp32NetworkJoinProvider};
use conduit_network_join_call::installation::{NetworkNativeIdentity, ReadyNetworkJoinBase};
use embassy_time::Duration;

type Adapter = Esp32ApStationSwitch<'static, 'static, crate::services::Owner>;
pub(crate) type Provider = Esp32NetworkJoinProvider<'static, 'static, crate::services::Owner>;
const ATTEMPT_SECONDS: u64 = 15;
const CLEANUP_SECONDS: u64 = 5;
const RESOURCE: &str = "pool/esp32/wifi-station";

pub struct NativeNetwork {
    radio: Option<crate::network::Radio>,
    services: Option<crate::services::Owner>,
    configuration: Option<esp_radio::wifi::sta::StationConfig>,
    adapter: Option<Adapter>,
    ready: Option<ReadyNetworkJoinBase<Provider>>,
    sdk: crate::sdk_memory::NativeSdkReservation,
    selected: bool,
    generation: u64,
    pub(crate) recovery: recovery::Recovery,
    pub(crate) retirement: Option<alloc::boxed::Box<crate::native_play::RetiredNetworkControl>>,
}
/// Observed driver/runner state, separate from service readiness or RF proof.
#[derive(serde::Serialize)]
pub struct Inspection {
    pub runner_polls: [u32; 2],
    pub ap_link: Option<bool>,
    pub station_link: Option<bool>,
    pub ap_configured: Option<bool>,
    pub station_configured: Option<bool>,
    pub ap_mac: Option<[u8; 6]>,
    pub channel: Option<u8>,
}
impl NativeNetwork {
    pub fn inspect(&self) -> Inspection {
        let stacks = self.stacks();
        Inspection {
            runner_polls: crate::network::runner_polls(),
            ap_link: stacks.map(|(ap, _)| ap.is_link_up()),
            station_link: stacks.map(|(_, station)| station.is_link_up()),
            ap_configured: stacks.map(|(ap, _)| ap.is_config_up()),
            station_configured: stacks.map(|(_, station)| station.is_config_up()),
            ap_mac: stacks.and_then(|(ap, _)| match ap.hardware_address() {
                embassy_net::HardwareAddress::Ethernet(mac) => Some(mac),
                _ => None,
            }),
            channel: self
                .radio
                .as_ref()
                .and_then(|radio| radio.controller.channel().ok().map(|(primary, _)| primary)),
        }
    }
    pub fn new(radio: crate::network::Radio, sdk: crate::sdk_memory::NativeSdkReservation) -> Self {
        assert_eq!(sdk.bytes(), crate::sdk_memory::SDK_BYTES);
        Self {
            radio: Some(radio),
            services: None,
            configuration: None,
            adapter: None,
            ready: None,
            sdk,
            selected: false,
            generation: 1,
            recovery: recovery::Recovery::new(),
            retirement: None,
        }
    }
    pub fn stacks(&self) -> Option<(embassy_net::Stack<'static>, embassy_net::Stack<'static>)> {
        self.radio.as_ref().map(|radio| radio.stacks)
    }
    pub fn retain_services(&mut self, services: crate::services::Owner) {
        assert!(self.services.is_none() && self.radio.is_some());
        self.services = Some(services);
    }
    pub fn is_ready(&self) -> bool {
        self.ready.is_some()
    }
    pub fn has_staged_configuration(&self) -> bool {
        self.selected
            || self.configuration.is_some()
            || self.adapter.is_some()
            || self.ready.is_some()
    }
    pub fn stage(&mut self, configuration: esp_radio::wifi::sta::StationConfig) -> Result<(), ()> {
        if self.configuration.is_some() || self.is_ready() || self.selected {
            return Err(());
        }
        if let Some(adapter) = &mut self.adapter {
            adapter
                .stage_station_configuration(configuration)
                .map_err(|_| ())
        } else {
            self.configuration = Some(configuration);
            Ok(())
        }
    }
    pub fn has_reclaimed_access_point(&self) -> bool {
        self.adapter.is_some()
            && !self.selected
            && !self.is_ready()
            && self.radio.is_none()
            && self.configuration.is_none()
    }
    /// Root's cooperative profile admits one attempt, no SDK retry, and one
    /// separately bounded restoration. It is not a hard real-time SDK WCET claim.
    /// SDK C allocation is confined to this retained fixed reservation, with
    /// null/Pressure on exhaustion and no Root-heap fallback. Configuration strings
    /// were prepared before this transfer; no credential is cloned or consumed.
    pub fn prepare(&mut self, host: &c::HostId, boot: &c::BootId) -> Result<(), ()> {
        if self.is_ready() {
            return Ok(());
        }
        if host.as_str().is_empty()
            || boot.as_str().is_empty()
            || self.sdk.bytes() != crate::sdk_memory::SDK_BYTES
        {
            return Err(());
        }
        // Identity allocation precedes transferring any initialized owner.
        let identity = NetworkNativeIdentity {
            host: host.clone(),
            boot: boot.clone(),
            base: "base/esp32/wifi-station".into(),
            instance: "instance/esp32/wifi-station".into(),
            provider_generation: self.generation,
            resource: RESOURCE.into(),
            resource_generation: c::ResourceGenerationId(alloc::format!(
                "generation/wifi-station/{}",
                self.generation
            )),
            envelope: "envelope/esp32/wifi-station".into(),
            artifact: "native/esp32-wifi-station-root@1".into(),
            implementation: "esp32/wifi-station@1".into(),
            base_implementation: "esp32/wifi-station-base@1".into(),
        };
        if self.adapter.is_none() {
            if self.radio.is_none() || self.services.is_none() || self.configuration.is_none() {
                return Err(());
            }
            let radio = self.radio.take().expect("retained controller");
            let configuration = self.configuration.take().expect("prepared configuration");
            let services = self.services.take().expect("acknowledged socket services");
            // SAFETY: actual exclusive controller, two independently running
            // interfaces and acknowledged service owner are transferred together.
            // Root retains their real isolated SDK reservation and the finite
            // cooperative budgets below. No adapter operation escapes selection.
            self.adapter = Some(unsafe {
                Esp32ApStationSwitch::new(
                    radio.controller,
                    radio.stacks,
                    radio.ap_address,
                    (radio.ap_config, configuration),
                    services,
                )
            });
        }
        // SAFETY: the retained adapter above owns the actual resources. Root
        // reserves one attachment generation in this observed Boot and admits
        // the fixed native storage and separate attempt/cleanup durations.
        // The SDK configuration validator requires failure_retry_cnt == 0.
        let attachment = alloc::format!("attachment/esp32/wifi-station/{}", self.generation);
        let provider = unsafe {
            Esp32NetworkJoinProvider::prepare(
                &mut self.adapter,
                Duration::from_secs(ATTEMPT_SECONDS),
                Duration::from_secs(CLEANUP_SECONDS),
                conduit_net::NetworkAttachmentInfo {
                    attachment_id: &attachment,
                    host_id: host.as_str(),
                    boot_id: boot.as_str(),
                    resource_pool_id: RESOURCE,
                    generation: self.generation,
                },
            )
        }
        .map_err(|_| ())?;
        // SAFETY: the actual provider and its SDK backing remain retained here.
        // The initialized ownership is distinct from successful association and
        // from the opaque authority/selection still required before invocation.
        self.ready = Some(
            unsafe { ReadyNetworkJoinBase::new(identity, self.generation, provider) }
                .expect("validated nonempty current native identity"),
        );
        Ok(())
    }
    /// Reclaim the actual spent provider after selected possession retirement.
    /// AP restoration must already be acknowledged; no readiness flag is reset
    /// without returned ownership. Fresh preparation will use a new generation.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn can_reclaim_restored(&self) -> bool {
        self.selected
            && self.ready.is_none()
            && self.adapter.is_none()
            && self.radio.is_none()
            && self.services.is_none()
            && self.configuration.is_none()
            && self.generation < u64::MAX
    }
    pub fn reclaim_restored(&mut self, provider: Provider) -> Result<(), Provider> {
        if !self.selected
            || self.ready.is_some()
            || self.adapter.is_some()
            || self.radio.is_some()
            || self.services.is_some()
            || self.configuration.is_some()
        {
            return Err(provider);
        }
        let Some(next) = self.generation.checked_add(1) else {
            return Err(provider);
        };
        match provider.into_restored_radio() {
            Ok(adapter) => {
                self.adapter = Some(adapter);
                self.generation = next;
                self.selected = false;
                Ok(())
            }
            Err(provider) => Err(provider),
        }
    }
    pub fn ready(&self) -> Option<&ReadyNetworkJoinBase<Provider>> {
        self.ready.as_ref()
    }
    pub fn ready_slot(&mut self) -> &mut Option<ReadyNetworkJoinBase<Provider>> {
        &mut self.ready
    }
    pub fn mark_selected(&mut self) {
        if self.ready.is_none() {
            self.selected = true;
        }
    }
    pub fn append_to_advertisement(&self, host: &mut c::HostAdvertisement) -> Result<(), ()> {
        if let Some(ready) = &self.ready {
            ready.append_to_advertisement(host).map_err(|_| ())?;
        }
        Ok(())
    }
}
