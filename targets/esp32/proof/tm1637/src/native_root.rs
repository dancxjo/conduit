//! Actual initialized GPIO ownership before any Body or native Plan is installed.
//! No simulated provider, descriptor-derived availability or execution is used.
use conduit_core as c;
use conduit_esp32_pulse_capture::{Esp32PulseCapture, LIMITS};
use conduit_pulse_capture::{installation::*, owner::*, PulseReadContract};

pub struct NativeRoot {
    credentials: conduit_network_join_call::credential_installation::ReadyCredentialBase<
        crate::native_credentials::RootCredentialSource,
    >,
    credential_writer: conduit_environment_http::network_setup::NetworkSetupWriter<'static>,
    station_config: Option<esp_radio::wifi::sta::StationConfig>,
    services: Option<crate::services::Owner>,
    pulse: ReadyPulseBase<Esp32PulseCapture<'static>>,
    contract: PulseReadContract,
    _controller: esp_radio::wifi::WifiController<'static>,
    _stacks: (embassy_net::Stack<'static>, embassy_net::Stack<'static>),
    _ap_address: embassy_net::StaticConfigV4,
    _ap_config: esp_radio::wifi::ap::AccessPointConfig,
    host: c::HostId,
    boot: c::BootId,
}
impl NativeRoot {
    pub fn prepare(
        sensor: Esp32PulseCapture<'static>,
        radio: crate::network::Radio,
        boot: &crate::boot::ObservedBoot,
    ) -> Self {
        let (host, boot) = boot.ids();
        let host: c::HostId = host.into();
        let boot: c::BootId = boot.into();
        // One owned pin, 64 bytes of capture attachment and the reviewed finite
        // cooperative limits. A future installation must separately admit its
        // cadence, capability authority and complete kernel memory/work before Play.
        let identity = PulseNativeIdentity {
            host_id: host.clone(),
            boot_id: boot.clone(),
            base_id: "base/esp32/gpio27".into(),
            provider_instance_id: "instance/esp32/gpio27".into(),
            provider_generation: 1,
            resource_pool_id: "pool/esp32/gpio27".into(),
            resource_generation_id: c::ResourceGenerationId("generation/gpio27/1".into()),
            envelope_id: "envelope/esp32/gpio27".into(),
            artifact_id: "native/esp32-gpio-pulse-root@1".into(),
            implementation_id: "esp32/gpio27-pulse@1".into(),
            base_implementation_id: "esp32/gpio27-pulse-base@1".into(),
        };
        // SAFETY: Root consumes the actual exclusively initialized GPIO27 Flex
        // provider. Its read implementation has no allocation, releases low on
        // drop, and enforces these finite hold/timeout and capture bounds.
        let pulse = unsafe {
            ReadyPulseBase::new(
                identity,
                PulseAttachment {
                    generation: 1,
                    resource_bytes: 64,
                    limits: LIMITS,
                },
                sensor,
            )
        }
        .unwrap();
        let (credentials, credential_writer) = crate::native_credentials::prepare(&host, &boot);
        Self {
            credentials,
            credential_writer,
            station_config: None,
            services: None,
            pulse,
            contract: PulseReadContract::prepare().unwrap(),
            _controller: radio.controller,
            _stacks: radio.stacks,
            _ap_address: radio.ap_address,
            _ap_config: radio.ap_config,
            host,
            boot,
        }
    }
    /// Fixed Root bootstrap service, independent of Body/Face installation. Two
    /// HTTP listeners and one DHCP socket share one finite private submission.
    pub async fn start_preparation_service(
        &mut self,
        spawner: embassy_executor::Spawner,
    ) -> Result<(), ()> {
        use conduit_esp32_network_switch::AccessPointServices;
        spawner.spawn(
            crate::services::run(self._stacks.0, Some(self.credential_writer.clone()))
                .map_err(|_| ())?,
        );
        let mut services = crate::services::control(false);
        embassy_time::with_timeout(embassy_time::Duration::from_secs(5), services.restore())
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
        self.services = Some(services);
        Ok(())
    }
    /// Preparation never consumes the selected source or changes radio mode.
    /// Once configured, subsequent offers reuse the retained SDK configuration.
    pub fn prepare_network_configuration(&mut self) -> &'static [u8] {
        use conduit_environment_http::network_setup::NetworkSetupRefusal as E;
        if self.station_config.is_some() {
            return b"prepared";
        }
        let Ok(mailbox) = self.credential_writer.try_lock() else {
            return b"busy";
        };
        match mailbox
            .prepare_acknowledged(conduit_esp32_network_switch::prepare_station_configuration)
        {
            Ok(Ok(config)) => {
                self.station_config = Some(config);
                b"prepared"
            }
            Ok(Err(_)) => b"refused",
            Err(E::NoSubmission | E::NotAcknowledged) => b"waiting",
            Err(_) => b"refused",
        }
    }
    /// Projection is made from the still-retained provider each time. It is an
    /// offer, not authority, membership, a Body birth or an installed native Play.
    pub fn write_offer(&self, output: &mut [u8]) -> Result<usize, ()> {
        let mut advertisement = c::HostAdvertisement {
            protocol_version: c::PROTOCOL_VERSION,
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            offer_generation: c::OfferGeneration(1),
            profile: "environment/esp32-native-root@1".into(),
            bases: alloc::vec![],
            resources: alloc::vec![],
            capabilities: alloc::vec![],
            planner_capabilities: alloc::vec![],
        };
        self.pulse
            .append_to_advertisement(&mut advertisement, &self.contract)
            .map_err(|_| ())?;
        self.credentials
            .append_to_advertisement(&mut advertisement)
            .map_err(|_| ())?;
        serde_json_core::to_slice(&advertisement, output).map_err(|_| ())
    }
}
