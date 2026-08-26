use std::{collections::BTreeMap, fs, path::Path};

use conduit_body_fabrication::{
    check_body_description, parse_body_description_conduit, CheckedBodyDescription,
};
use conduit_core::{BootId, ConnectionBase, HostId, OfferGeneration};
use conduit_host_fabrication::{parse_host_configuration_conduit, HostConfiguration};
use conduit_std_host::{StdHost, StdHostConfig};

use crate::product_execution::{ProductExecutionContext, ProductRuntime};

pub(crate) struct BodyProduct {
    pub(crate) context: ProductExecutionContext,
}

pub(crate) fn load(path: &Path) -> Result<CheckedBodyDescription, String> {
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".body.conduit"))
    {
        return Err(format!(
            "Body construction source must use the canonical .body.conduit suffix: {}",
            path.display()
        ));
    }
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let description = parse_body_description_conduit(&source)
        .map_err(|diagnostic| format!("Body description decode refused: {diagnostic:?}"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut configurations = BTreeMap::<String, HostConfiguration>::new();
    for host in &description.hosts {
        if configurations.contains_key(&host.configuration) {
            continue;
        }
        let configuration_path = parent.join(&host.configuration);
        if !configuration_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".host.conduit"))
        {
            return Err(format!(
                "referenced Host source must use the canonical .host.conduit suffix: {}",
                configuration_path.display()
            ));
        }
        let configuration_source =
            fs::read_to_string(&configuration_path).map_err(|error| error.to_string())?;
        let configuration =
            parse_host_configuration_conduit(&configuration_source).map_err(|diagnostic| {
                format!(
                    "Host configuration {} decode refused: {diagnostic:?}",
                    configuration_path.display()
                )
            })?;
        configurations.insert(host.configuration.clone(), configuration);
    }
    check_body_description(
        description,
        &configurations,
        &conduit_workspace_fabrication::catalog(),
        &conduit_workspace_fabrication::package_set(),
    )
    .map_err(|diagnostics| format!("Body description refused: {diagnostics:?}"))
}

pub(crate) fn prepare(path: &Path) -> Result<BodyProduct, String> {
    let checked = load(path)?;
    let context = context(&checked)?;
    Ok(BodyProduct { context })
}

fn context(body: &CheckedBodyDescription) -> Result<ProductExecutionContext, String> {
    if body.hosts().len() != 2 {
        return Err(format!(
            "ordinary std Line execution requires exactly two current hosted Hosts; Body has {}",
            body.hosts().len()
        ));
    }
    let mut source = None;
    let mut sink = None;
    for host in body.hosts() {
        let target = &host.configuration.profile().target;
        if target.family != "std" || target.architecture != "x86_64" {
            return Err(format!(
                "Body Host '{}' target {} is not an ordinary hosted std runtime",
                host.description.name,
                target.key()
            ));
        }
        let bases = host.configuration.resolved_bases();
        let source_capable = bases.iter().any(|(kind, _)| kind == "clock/monotonic");
        let sink_capable = bases.iter().any(|(kind, _)| kind == "serial/text");
        if source_capable && !sink_capable && source.is_none() {
            source = Some(runtime(body, host, conduit_signal::PULSE_KIND)?);
        } else if sink_capable && !source_capable && sink.is_none() {
            sink = Some(runtime(body, host, conduit_signal::SHOW_KIND)?);
        } else {
            return Err(format!(
                "Body Host '{}' must select exactly one execution role through clock/monotonic or serial/text",
                host.description.name
            ));
        }
    }
    let source = source.ok_or("Body has no clock/monotonic source Host")?;
    let sink = sink.ok_or("Body has no serial/text sink Host")?;
    let line = crate::std_websocket_line::line_offer(&source, &sink);
    ProductExecutionContext::new(
        vec![source.advertisement().clone(), sink.advertisement().clone()],
        vec![ProductRuntime::std(source), ProductRuntime::std(sink)],
        vec![ConnectionBase::WebSocket],
        vec![line],
    )
}

fn runtime(
    body: &CheckedBodyDescription,
    host: &conduit_body_fabrication::CheckedBodyHost,
    kind: &str,
) -> Result<StdHost, String> {
    let host_id = HostId::from(format!(
        "{}/host/{}",
        body.description().body.id,
        host.description.name
    ));
    let boot_id = BootId::from(format!(
        "{}/boot/{}",
        host_id.as_str(),
        host.configuration.configuration_id()
    ));
    let host = StdHost::new_with_config(StdHostConfig {
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
    });
    let mut advertisement = host.advertisement().clone();
    advertisement
        .capabilities
        .retain(|capability| capability.kind_id.as_str() == kind);
    if advertisement.capabilities.len() != 1 {
        return Err(format!(
            "hosted std runtime does not expose one exact {kind} capability"
        ));
    }
    StdHost::from_advertisement(advertisement)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_body_drives_two_exact_hosts_and_one_line_without_fixture_identity() {
        let path = Path::new("../../profiles/bodies/std-line.body.conduit");
        let checked = load(path).unwrap();
        let product = prepare(path).unwrap();
        assert_eq!(checked.hosts().len(), 2);
        assert_eq!(product.context.advertisements().len(), 2);
        assert_eq!(product.context.line_offers().len(), 1);
        assert!(product
            .context
            .advertisements()
            .iter()
            .all(|host| host.host_id.as_str().starts_with("body:std-line/host/")));
    }
}
