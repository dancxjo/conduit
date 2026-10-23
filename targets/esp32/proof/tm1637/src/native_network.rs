//! Root-owned native preparation. No radio effect can be called here: the
//! prepared provider is exposed only through its exact-selection Base owner.
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
}
impl NativeNetwork {
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
        }
    }
    pub fn ap_stack(&self) -> Option<embassy_net::Stack<'static>> {
        self.radio.as_ref().map(|radio| radio.stacks.0)
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
    pub fn stage(&mut self, configuration: esp_radio::wifi::sta::StationConfig) {
        assert!(self.configuration.is_none() && self.adapter.is_none() && !self.is_ready());
        self.configuration = Some(configuration);
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
            provider_generation: 1,
            resource: RESOURCE.into(),
            resource_generation: c::ResourceGenerationId("generation/wifi-station/1".into()),
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
        let provider = unsafe {
            Esp32NetworkJoinProvider::prepare(
                &mut self.adapter,
                Duration::from_secs(ATTEMPT_SECONDS),
                Duration::from_secs(CLEANUP_SECONDS),
                conduit_net::NetworkAttachmentInfo {
                    attachment_id: "attachment/esp32/wifi-station/1",
                    host_id: host.as_str(),
                    boot_id: boot.as_str(),
                    resource_pool_id: RESOURCE,
                    generation: 1,
                },
            )
        }
        .map_err(|_| ())?;
        // SAFETY: the actual provider and its SDK backing remain retained here.
        // The initialized ownership is distinct from successful association and
        // from the opaque authority/selection still required before invocation.
        self.ready = Some(
            unsafe { ReadyNetworkJoinBase::new(identity, 1, provider) }
                .expect("validated nonempty current native identity"),
        );
        Ok(())
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
