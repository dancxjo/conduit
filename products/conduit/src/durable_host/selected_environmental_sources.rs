//! Explicit checked device attachments; inert declarations confer no native custody.
use super::{bounded_read, digest};
use conduit_environment_host::source_operator::{
    CollectorRoot, DetachedCollectorEquipment, NetworkSourceConfiguration,
};
use conduit_host_make::{CheckedHostConfiguration, ConfigurationDevice, ConfigurationScalar};
use conduit_std_host::{isolated_http_base::IsolatedHttpBaseConfig, StdHost};
use serde::Deserialize;
use std::{collections::BTreeSet, net::SocketAddr, path::PathBuf, time::Duration};
pub(crate) struct AttachedEquipment {
    pub(crate) native: DetachedCollectorEquipment,
    /// Instrument construction/placement context is distinct from the sampling Plot.
    pub(crate) instruments: Vec<SelectedInstrument>,
    pub(crate) authority_grants: Vec<conduit_core::AuthorityGrant>,
}
pub(crate) struct SelectedInstrument {
    pub(crate) attachment_id: String,
    pub(crate) source_configuration: CheckedHostConfiguration,
    pub(crate) firmware: [u8; 32],
    pub(crate) session: conduit_environment_host::source_operator::SourceSessionDescriptor,
}
impl AttachedEquipment {
    /// Check descriptive associations against the actual retained inventory.
    /// These checks confer no ownership, grant, or remote device membership.
    pub(crate) fn validate_owner(&self, host: &StdHost) -> Result<(), String> {
        let mut labels = BTreeSet::new();
        for instrument in &self.instruments {
            if !labels.insert(&instrument.attachment_id)
                || instrument
                    .source_configuration
                    .configuration_id()
                    .is_empty()
                || instrument.firmware == [0; 32]
                || instrument.session.host() != &host.advertisement().host_id
                || instrument.session.boot() != &host.advertisement().boot_id
                || !host
                    .advertisement()
                    .capabilities
                    .iter()
                    .any(|offer| &offer.capability_id == instrument.session.capability())
                || !host.advertisement().bases.iter().any(|base| {
                    &base.provider_instance_id == instrument.session.session()
                        && base
                            .capability_ids
                            .contains(instrument.session.capability())
                })
            {
                return Err(
                    "selected instrument association differs from retained native inventory".into(),
                );
            }
        }
        if self.authority_grants.iter().any(|grant| {
            grant.host_id != host.advertisement().host_id
                || grant.boot_id != host.advertisement().boot_id
        }) {
            return Err("retained native planning grant has a foreign scope".into());
        }
        Ok(())
    }
}
const IMPLEMENTATION: &str = "linux/environmental-sources@1";
const DEVICE: &str = "environment/source@1";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Parameters {
    worker_executable: PathBuf,
    worker_sha256: String,
    timeout_milliseconds: u64,
}
struct Attachment {
    device: ConfigurationDevice,
    checked: CheckedHostConfiguration,
    firmware: [u8; 32],
}
fn hex(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("source selection requires exact lowercase SHA-256".into());
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| "source SHA-256")?;
    }
    Ok(bytes)
}
fn prepare(
    checked: &CheckedHostConfiguration,
) -> Result<Option<(Parameters, Vec<Attachment>)>, String> {
    let bases = checked
        .configuration()
        .bases
        .iter()
        .filter(|base| base.kind == "environment/source")
        .collect::<Vec<_>>();
    let Some(base) = bases.first() else {
        return Ok(None);
    };
    if bases.len() != 1 {
        return Err("select exactly one environmental Source Base".into());
    }
    let initialization = base
        .initialization
        .as_ref()
        .ok_or("environmental Source Base requires explicit initialization")?;
    let catalog = conduit_workspace_make::catalog();
    initialization.checked_values(
        catalog
            .initialization_contracts
            .get(IMPLEMENTATION)
            .ok_or("environmental Source initialization contract absent")?,
    )?;
    let parameters: Parameters =
        serde_json::from_value(serde_json::to_value(initialization).map_err(|e| e.to_string())?)
            .map_err(|e| format!("environmental Source parameters: {e}"))?;
    if !parameters.worker_executable.is_absolute() {
        return Err("selected Source worker requires an absolute executable".into());
    }
    hex(&parameters.worker_sha256)?;
    let mut attachments = Vec::new();
    let mut endpoints = BTreeSet::new();
    for device in &checked.configuration().devices {
        let (implementation, configuration, endpoint) = match device {
            ConfigurationDevice::Serial {
                implementation,
                configuration,
                path,
                ..
            } => {
                if !std::path::Path::new(path).is_absolute() {
                    return Err("selected serial Source requires absolute device path".into());
                }
                (implementation, configuration, format!("serial:{path}"))
            }
            ConfigurationDevice::Network {
                implementation,
                configuration,
                endpoint,
                authority,
                ..
            } => {
                let address = endpoint
                    .parse::<SocketAddr>()
                    .map_err(|_| "network Source requires an exact IP socket endpoint")?;
                if address.port() == 0 || authority.is_empty() {
                    return Err("network Source endpoint is incomplete".into());
                }
                (implementation, configuration, format!("network:{address}"))
            }
            _ => continue,
        };
        if implementation != DEVICE {
            return Err("selected native attachment implementation is unsupported".into());
        }
        if attachments.len() >= 4 || !endpoints.insert(endpoint) {
            return Err(
                "selected Source attachments exceed capacity or duplicate an endpoint".into(),
            );
        }
        if configuration.len() != 2 {
            return Err(
                "Source attachment requires only source_configuration and firmware_sha256".into(),
            );
        }
        let text = |key: &str| match configuration.get(key) {
            Some(ConfigurationScalar::Text(value)) => Ok(value.as_str()),
            _ => Err(format!("Source attachment requires typed {key}")),
        };
        let path = PathBuf::from(text("source_configuration")?);
        if !path.is_absolute()
            || !path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.ends_with(".host.conduit"))
        {
            return Err(
                "Source attachment requires an absolute checked .host.conduit source".into(),
            );
        }
        let bytes = bounded_read(&path, 16 * 1024)?;
        let source =
            std::str::from_utf8(&bytes).map_err(|_| "Source configuration must be UTF-8")?;
        let configuration = conduit_host_make::parse_host_configuration_conduit(source)
            .map_err(|e| format!("Source construction: {e:?}"))?;
        let source = conduit_host_make::check_host_configuration(
            configuration,
            &catalog,
            &conduit_workspace_make::package_set(),
        )
        .map_err(|e| format!("Source construction refused: {e:?}"))?;
        attachments.push(Attachment {
            device: device.clone(),
            checked: source,
            firmware: hex(text("firmware_sha256")?)?,
        });
    }
    if attachments.is_empty() {
        return Err(
            "selected environmental Base needs one to four Serial/Network attachments".into(),
        );
    }
    Ok(Some((parameters, attachments)))
}
pub(super) fn validate(checked: &CheckedHostConfiguration) -> Result<(), String> {
    prepare(checked).map(|_| ())
}
pub(super) fn attach(
    host: StdHost,
    checked: &CheckedHostConfiguration,
) -> Result<(StdHost, Option<AttachedEquipment>), String> {
    let Some((parameters, attachments)) = prepare(checked)? else {
        return Ok((host, None));
    };
    let executable = bounded_read(&parameters.worker_executable, 64 * 1024 * 1024)?;
    if digest(&executable).strip_prefix("sha256:") != Some(parameters.worker_sha256.as_str()) {
        return Err("selected environmental worker identity changed".into());
    }
    let budget = Duration::from_millis(parameters.timeout_milliseconds);
    let mut instruments = Vec::with_capacity(attachments.len());
    let mut collector = CollectorRoot::new_installed(host);
    for attachment in attachments {
        let attachment_id = attachment.device.id().to_owned();
        let source_configuration = attachment.checked.clone();
        let firmware = attachment.firmware;
        let id = match attachment.device {
            ConfigurationDevice::Serial { path, .. } => collector
                .attach_usb(
                    std::path::Path::new(&path),
                    &attachment.checked,
                    attachment.firmware,
                    budget,
                )
                .map_err(|e| format!("initialize selected serial Source: {e:?}"))?,
            ConfigurationDevice::Network {
                id,
                endpoint,
                authority,
                ..
            } => {
                let nonce = crate::birth_identity::fresh_uuid()?;
                collector
                    .attach_network(
                        NetworkSourceConfiguration {
                            http: IsolatedHttpBaseConfig {
                                executable: parameters.worker_executable.clone(),
                                base_instance_id: format!(
                                    "installed/source-acquisition/{id}/{nonce}"
                                )
                                .into(),
                                provider_generation: 1,
                                authority,
                                endpoint: endpoint
                                    .parse()
                                    .map_err(|_| "invalid exact source endpoint")?,
                                resource_generation_id: conduit_core::ResourceGenerationId(
                                    format!("installed/source-acquisition/{nonce}"),
                                ),
                            },
                            firmware: attachment.firmware,
                        },
                        &attachment.checked,
                        budget,
                    )
                    .map_err(|e| format!("initialize selected network Source: {e:?}"))?
            }
            _ => return Err("unsupported selected source transport".into()),
        };
        let session = collector
            .source_descriptor(id)
            .map_err(|reason| format!("retain selected instrument session: {reason:?}"))?;
        instruments.push(SelectedInstrument {
            attachment_id,
            source_configuration,
            firmware,
            session,
        });
    }
    let authority_grants = collector
        .native_authority_grants()
        .map_err(|reason| format!("retain selected native grants: {reason:?}"))?;
    let (host, equipment) = collector
        .detach()
        .map_err(|e| format!("seal selected Source inventory: {:?}", e.reason))?;
    let equipment = AttachedEquipment {
        native: equipment,
        instruments,
        authority_grants,
    };
    equipment.validate_owner(&host)?;
    Ok((host, Some(equipment)))
}
