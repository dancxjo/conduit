//! Actual initialized GPIO ownership before any Body or native Plan is installed.
//! No simulated provider, descriptor-derived availability or execution is used.
use conduit_core as c;
use conduit_esp32_pulse_capture::{Esp32PulseCapture, LIMITS};
use conduit_pulse_capture::{installation::*, owner::*, PulseReadContract};

pub struct NativeRoot {
    network_preparation: Option<
        Result<
            crate::network_planning::PreparedNetworkSetup,
            crate::network_planning::NetworkPlanningRefusal,
        >,
    >,
    network_source: conduit_plot::ExpandedCanonicalPlot,
    credentials: conduit_network_join_call::credential_installation::ReadyCredentialBase<
        crate::native_credentials::RootCredentialSource,
    >,
    credential_writer: conduit_environment_http::network_setup::NetworkSetupWriter<'static>,
    network: crate::native_network::NativeNetwork,
    pulse: ReadyPulseBase<Esp32PulseCapture<'static>>,
    contract: PulseReadContract,
    host: c::HostId,
    boot: c::BootId,
}
impl NativeRoot {
    pub fn prepare(
        sensor: Esp32PulseCapture<'static>,
        radio: crate::network::Radio,
        boot: &crate::boot::ObservedBoot,
        network_source: conduit_plot::ExpandedCanonicalPlot,
        sdk_reservation: crate::sdk_memory::NativeSdkReservation,
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
            network_preparation: None,
            network_source,
            credentials,
            credential_writer,
            network: crate::native_network::NativeNetwork::new(radio, sdk_reservation),
            pulse,
            contract: PulseReadContract::prepare().unwrap(),
            host,
            boot,
        }
    }
    pub fn network_source(&self) -> &conduit_plot::ExpandedCanonicalPlot {
        &self.network_source
    }
    /// Use current initialized offers only. No authority or local Base is
    /// inferred from authored meaning, SDK configuration or the UART request.
    /// A prepared Plan remains uninstalled; admission and kernel storage are
    /// separate. Each call replaces preparation against this same actual Boot.
    pub fn prepare_network_plan(&mut self) -> Result<(), ()> {
        self.network_preparation = None;
        let host = self.advertisement()?;
        self.network_preparation =
            Some(crate::network_planning::PreparedNetworkSetup::from_checked(
                &self.network_source,
                &host,
                &[],
                &[],
            ));
        Ok(())
    }
    pub fn network_plan_status(&self) -> &'static str {
        match &self.network_preparation {
            None => "unprepared",
            Some(Err(error)) => error.code(),
            Some(Ok(prepared)) => {
                // Read the retained ordinary result; this is not installation.
                debug_assert_eq!(prepared.plan().fragments.len(), 1);
                debug_assert!(!prepared.lowered().nodes.is_empty());
                "prepared-uninstalled"
            }
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
            crate::services::run(
                self.network.ap_stack().ok_or(())?,
                Some(self.credential_writer.clone()),
            )
            .map_err(|_| ())?,
        );
        let mut services = crate::services::control(false);
        embassy_time::with_timeout(embassy_time::Duration::from_secs(5), services.restore())
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
        self.network.retain_services(services);
        Ok(())
    }
    /// Preparation never consumes the selected source or changes radio mode.
    /// Once configured, subsequent offers reuse the retained SDK configuration.
    pub fn prepare_network_configuration(&mut self) -> &'static [u8] {
        use conduit_environment_http::network_setup::NetworkSetupRefusal as E;
        if self.network.is_ready() {
            return b"prepared";
        }
        if self.network.has_staged_configuration() {
            // A refused preparation still retains its initialized owner. The
            // one-submission diagnostic never overwrites or silently retries it.
            return b"refused";
        }
        let configuration = {
            let Ok(mailbox) = self.credential_writer.try_lock() else {
                return b"busy";
            };
            mailbox
                .prepare_acknowledged(conduit_esp32_network_switch::prepare_station_configuration)
        };
        match configuration {
            Ok(Ok(config)) => {
                self.network.stage(config);
                if self.network.prepare(&self.host, &self.boot).is_err() {
                    return b"refused";
                }
                // The retained provider changes the truthful offer generation.
                // Preparation is fresh; old refusal/Plan material is discarded.
                if self.prepare_network_plan().is_err() {
                    return b"refused";
                }
                b"prepared"
            }
            Ok(Err(_)) => b"refused",
            Err(E::NoSubmission | E::NotAcknowledged) => b"waiting",
            Err(_) => b"refused",
        }
    }
    pub fn network_ready(&self) -> bool {
        self.network.is_ready()
    }
    /// Projection is made from the still-retained provider each time. It is an
    /// offer, not authority, membership, a Body birth or an installed native Play.
    pub fn write_offer(&self, output: &mut [u8]) -> Result<usize, ()> {
        serde_json_core::to_slice(&self.advertisement()?, output).map_err(|_| ())
    }
    fn advertisement(&self) -> Result<c::HostAdvertisement, ()> {
        let mut advertisement = c::HostAdvertisement {
            protocol_version: c::PROTOCOL_VERSION,
            host_id: self.host.clone(),
            boot_id: self.boot.clone(),
            offer_generation: c::OfferGeneration(if self.network.is_ready() { 2 } else { 1 }),
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
        self.network.append_to_advertisement(&mut advertisement)?;
        Ok(advertisement)
    }
}
