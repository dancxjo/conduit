//! Environmental interpretation of the existing selected confined HTTP Base.
//! Only this adapter constructs its private GET /environment request. The
//! generic HTTP Base remains method-neutral and grants no Body membership.
use super::*;
use crate::device_carrier_source::{decode, encoding, Encoding};
use conduit_environment_runtime::environmental_reading::source_observation::{
    SourceContinuity, SourceContinuityRefusal, SourceObservation, SourceOutcome,
};
use conduit_environment_runtime::environmental_source::Refusal as SourceRefusal;
use conduit_std_host::isolated_http_base::{IsolatedHttpBaseConfig, IsolatedHttpHost};
use std::time::Instant;
#[derive(Clone)]
pub struct NetworkSourceConfiguration {
    pub http: IsolatedHttpBaseConfig,
    pub firmware: [u8; 32],
}
pub(super) struct NetworkSource {
    installed: SharedHost,
    configuration: NetworkSourceConfiguration,
    expected_configuration: String,
    encoding: Encoding,
    continuity: SourceContinuity,
    budget: Duration,
    failed: bool,
    next_transaction: u64,
    plan: Option<Plan>,
    capability: Option<CapabilityId>,
}
impl NetworkSource {
    pub(super) fn acquire(
        installed: SharedHost,
        configuration: NetworkSourceConfiguration,
        checked: &CheckedHostConfiguration,
        budget: Duration,
    ) -> Result<Self, Refusal> {
        if budget < Duration::from_secs(31)
            || budget > Duration::from_secs(45)
            || configuration.firmware == [0; 32]
        {
            return Err(Refusal::Acquisition);
        }
        let encoding = encoding(checked).map_err(|_| Refusal::Acquisition)?;
        let mut owner = Self {
            installed,
            configuration,
            expected_configuration: checked.configuration_id().to_owned(),
            encoding,
            continuity: SourceContinuity::default(),
            budget,
            failed: false,
            next_transaction: 1,
            plan: None,
            capability: None,
        };
        // The native HTTP provider must complete the existing exact-endpoint selected
        // exchange and independently pinned identity check before Source is offered.
        let bytes = owner.read().map_err(|_| Refusal::Acquisition)?;
        decode(&bytes, owner.encoding).map_err(|_| Refusal::Acquisition)?;
        Ok(owner)
    }
    pub(super) fn register_native(
        &mut self,
        identity: &Identity,
        base: &mut BaseProviderEntry,
    ) -> Result<(), Refusal> {
        self.configuration.http.base_instance_id = identity.provider_instance_id.clone();
        self.configuration.http.provider_generation = identity.provider_generation;
        self.configuration.http.resource_generation_id =
            ResourceGenerationId(format!("{}/http", identity.resource_generation_id.0));
        let mut offer = conduit_std_host::isolated_http_base::isolated_http_client_offer();
        offer.capability_id = format!("{}/http", identity.capability.as_str()).into();
        self.capability = Some(offer.capability_id.clone());
        self.plan = None;
        base.capabilities.push(offer);
        base.resources.push(resource_offer(
            &format!("{}/http", identity.resource_pool_id.as_str()),
            conduit_std_host::isolated_http_base::HTTP_CLIENT_RESOURCE,
            1,
        ));
        Ok(())
    }
    pub(super) fn matches_capability(&self, capability: &CapabilityId) -> bool {
        self.capability.as_ref() == Some(capability)
    }
    pub(super) fn exchange_admitted(
        &self,
        host: &conduit_std_host::StdHost,
        work: &mut conduit_std_host::PreparedNativeWork,
    ) -> Result<Vec<u8>, Refusal> {
        if self.failed {
            return Err(Refusal::Possession);
        }
        let response = conduit_std_host::isolated_http_base::exchange_native_work_response(
            host,
            work,
            &self.configuration.http,
        )
        .map_err(|_| Refusal::Acquisition)?;
        conduit_web::encode_response(&response).map_err(|_| Refusal::Capacity)
    }
    pub(super) fn authority_grants(&self, host: &HostAdvertisement) -> Vec<AuthorityGrant> {
        self.capability
            .as_ref()
            .and_then(|id| {
                host.capabilities
                    .iter()
                    .find(|offer| &offer.capability_id == id)
            })
            .map(|offer| {
                offer
                    .authority_requirements
                    .iter()
                    .enumerate()
                    .map(|(index, requirement)| AuthorityGrant {
                        grant_id: format!(
                            "grant/environment-http/{}/{index}",
                            offer.capability_id.as_str()
                        )
                        .into(),
                        contract_id: requirement.contract_id.clone(),
                        host_call_contract_id: requirement.host_call_contract_id.clone(),
                        subject_kind: requirement.subject_kind.clone(),
                        host_id: host.host_id.clone(),
                        boot_id: host.boot_id.clone(),
                        capability_id: offer.capability_id.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    fn read(&mut self) -> Result<Vec<u8>, SourceRefusal> {
        let transaction = self.next_transaction;
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or(SourceRefusal::Capacity)?;
        let bytes = if self.capability.is_some() {
            let mut installed = self
                .installed
                .lock()
                .expect("collector Host mutex poisoned");
            let host = installed.as_mut().ok_or(SourceRefusal::Provider)?;
            perform(
                host,
                &self.configuration.http,
                self.plan.as_ref().ok_or(SourceRefusal::Possession)?,
                transaction,
            )?
        } else {
            let installed = self
                .installed
                .lock()
                .expect("collector Host mutex poisoned")
                .take()
                .ok_or(SourceRefusal::Provider)?;
            let mut http =
                match IsolatedHttpHost::from_host(installed, self.configuration.http.clone()) {
                    Ok(http) => http,
                    Err((host, _)) => {
                        *self
                            .installed
                            .lock()
                            .expect("collector Host mutex poisoned") = Some(host);
                        return Err(SourceRefusal::Provider);
                    }
                };
            let result = (|| {
                let plan = http_plan(http.host().advertisement(), None)
                    .map_err(|_| SourceRefusal::Provider)?;
                perform(
                    http.host_mut(),
                    &self.configuration.http,
                    &plan,
                    transaction,
                )
            })();
            *self
                .installed
                .lock()
                .expect("collector Host mutex poisoned") = Some(http.into_host());
            result?
        };
        verify_identity(
            &bytes,
            &self.configuration.firmware,
            &self.expected_configuration,
        )?;
        Ok(bytes)
    }
}
fn perform(
    host: &mut conduit_std_host::StdHost,
    configuration: &IsolatedHttpBaseConfig,
    plan: &Plan,
    transaction: u64,
) -> Result<Vec<u8>, SourceRefusal> {
    if plan.fragments.len() != 1 {
        return Err(SourceRefusal::Possession);
    }
    let fragment = &plan.fragments[0];
    let play = host
        .issue_kernel_play(fragment)
        .map_err(|_| SourceRefusal::Provider)?;
    let request = conduit_web::HttpRequest {
        transaction_id: conduit_web::HttpTransactionId::new(transaction)
            .map_err(|_| SourceRefusal::Request)?,
        method: conduit_web::HttpMethod::Get,
        target: conduit_web::HttpTarget::new(
            configuration.authority.clone(),
            "/environment".into(),
            conduit_web::HttpScheme::Http,
        )
        .map_err(|_| SourceRefusal::Request)?,
        headers: Default::default(),
        body: conduit_web::HttpBody::inline([]),
    };
    let (_, response) = conduit_std_host::isolated_http_base::exchange_selected_response(
        host,
        configuration,
        play,
        fragment,
        &request,
    )
    .map_err(|_| SourceRefusal::Provider)?;
    if response.status != 200 {
        return Err(SourceRefusal::Provider);
    }
    response
        .body
        .as_inline()
        .filter(|b| b.len() <= 1536)
        .map(|b| b.to_vec())
        .ok_or(SourceRefusal::Provider)
}

impl EnvironmentalSourceProvider for NetworkSource {
    fn prepare_for_host(&mut self, host: &HostAdvertisement) -> Result<(), SourceRefusal> {
        self.plan =
            Some(http_plan(host, self.capability.as_ref()).map_err(|_| SourceRefusal::Possession)?);
        Ok(())
    }

    async fn observe(&mut self) -> Result<SourceObservation, SourceRefusal> {
        if self.failed {
            return Err(SourceRefusal::Provider);
        }
        let deadline = Instant::now()
            .checked_add(self.budget)
            .ok_or(SourceRefusal::Provider)?;
        loop {
            let bytes = match self.read() {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.failed = true;
                    return Err(error);
                }
            };
            let at = boot_ms()?;
            let outcome = decode(&bytes, self.encoding).map_err(|_| {
                self.failed = true;
                SourceRefusal::Provider
            })?;
            match outcome {
                SourceOutcome::Unavailable(_) => {
                    return Ok(SourceObservation {
                        observed_at_host_boot_ms: at,
                        outcome,
                    })
                }
                SourceOutcome::Available(sample) => match self.continuity.observe(&sample, at) {
                    Ok(()) => {
                        return Ok(SourceObservation {
                            observed_at_host_boot_ms: at,
                            outcome,
                        })
                    }
                    Err(SourceContinuityRefusal::NoNewObservation) => {}
                    Err(_) => {
                        self.failed = true;
                        return Err(SourceRefusal::Provider);
                    }
                },
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Ok(SourceObservation {observed_at_host_boot_ms:at,outcome:SourceOutcome::Unavailable(conduit_environment_runtime::environmental_publication::SourceFailure::CaptureTimeout)});
            };
            std::thread::sleep(remaining.min(Duration::from_millis(250)));
        }
    }
}
fn verify_identity(
    bytes: &[u8],
    firmware: &[u8; 32],
    configuration: &str,
) -> Result<(), SourceRefusal> {
    #[derive(serde::Deserialize)]
    struct Identity<'a> {
        #[serde(borrow)]
        role: &'a str,
        #[serde(borrow)]
        build: &'a str,
        #[serde(borrow)]
        configuration: &'a str,
    }
    let identity: Identity<'_> =
        serde_json::from_slice(bytes).map_err(|_| SourceRefusal::Provider)?;
    let expected = firmware
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>();
    if identity.role != "conduit.environment-wifi-carrier@1"
        || identity.build != expected
        || identity.configuration != configuration
    {
        return Err(SourceRefusal::Possession);
    }
    Ok(())
}
fn boot_ms() -> Result<u64, SourceRefusal> {
    let mut clock = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut clock) } != 0
        || clock.tv_sec < 0
        || clock.tv_nsec < 0
    {
        return Err(SourceRefusal::Provider);
    }
    (clock.tv_sec as u64)
        .checked_mul(1000)
        .and_then(|s| s.checked_add(clock.tv_nsec as u64 / 1_000_000))
        .ok_or(SourceRefusal::Provider)
}
fn http_plan(host: &HostAdvertisement, selected: Option<&CapabilityId>) -> Result<Plan, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut catalog = conduit_plot::ProfileCatalog::new();
    conduit_web::install_http_catalogs(&mut startup, &mut catalog)?;
    let plot = conduit_plot::parse(
        "plot environmental-native-read {\n client: http/client\n}\n",
        &catalog,
    )
    .map_err(|e| format!("{e:?}"))?;
    let capability = host
        .capabilities
        .iter()
        .find(|c| {
            c.implementation.implementation_id.as_str()
                == conduit_std_host::isolated_http_base::ISOLATED_HTTP_IMPLEMENTATION
                && selected.map_or(
                    c.capability_id.as_str() == "std-isolated-http-client-http1",
                    |id| &c.capability_id == id,
                )
        })
        .ok_or("selected HTTP capability missing")?;
    let grants = capability
        .authority_requirements
        .iter()
        .enumerate()
        .map(|(i, r)| AuthorityGrant {
            grant_id: format!(
                "grant/environment-http/{}/{i}",
                capability.capability_id.as_str()
            )
            .into(),
            contract_id: r.contract_id.clone(),
            host_call_contract_id: r.host_call_contract_id.clone(),
            subject_kind: r.subject_kind.clone(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            capability_id: capability.capability_id.clone(),
        })
        .collect::<Vec<_>>();
    let hosts = [host.clone()];
    let placements = conduit_planner::PlacementChoices {
        by_gear: plot
            .gears
            .iter()
            .map(|gear| {
                (
                    gear.gear_id.clone(),
                    conduit_planner::PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: capability.capability_id.clone(),
                    },
                )
            })
            .collect(),
    };
    conduit_planner::plan_with_options(
        &plot,
        &hosts,
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|e| format!("{e:?}"))
}
