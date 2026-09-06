use std::{collections::BTreeMap, fs, path::Path};

use conduit_body_fabrication::{
    check_body_description, parse_body_description_conduit, CheckedBodyDescription,
};
use conduit_core::{BaseImplementationId, BootId, HostId, OfferGeneration};
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
    let mut hosts = Vec::with_capacity(body.hosts().len());
    for host in body.hosts() {
        hosts.push((runtime(body, host)?, host.configuration.profile()));
    }
    let mut line_offers = Vec::new();
    for (source, _) in &hosts {
        for (sink, _) in &hosts {
            if source.advertisement().host_id != sink.advertisement().host_id {
                line_offers.push(crate::std_websocket_line::line_offer(source, sink));
            }
        }
    }
    let mut connection_bases = vec![BaseImplementationId::from("conduit.base/local@1")];
    let mut line_runtimes = Vec::<Box<dyn crate::product_execution::ProductLineRuntime>>::new();
    if !line_offers.is_empty() {
        connection_bases.push(BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1",
        ));
        line_runtimes.push(Box::new(crate::std_websocket_line::ProductWebSocketRuntime));
    }
    let advertisements = hosts
        .iter()
        .map(|(host, _)| host.advertisement().clone())
        .collect();
    let runtimes = hosts
        .into_iter()
        .map(|(host, _)| ProductRuntime::std(host))
        .collect();
    ProductExecutionContext::new(
        advertisements,
        runtimes,
        connection_bases,
        line_offers,
        line_runtimes,
    )
}

fn runtime(
    body: &CheckedBodyDescription,
    host: &conduit_body_fabrication::CheckedBodyHost,
) -> Result<StdHost, String> {
    let target = &host.configuration.profile().target;
    if target.family != "std" || target.architecture != std::env::consts::ARCH {
        return Err(format!(
            "Body Host '{}' target {} has no installed product runtime handle on std/{}",
            host.description.name,
            target.key(),
            std::env::consts::ARCH
        ));
    }
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
    let runtime_host = StdHost::new_with_config(StdHostConfig {
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
    });
    Ok(runtime_host)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_body_drives_current_hosts_and_bounded_lines_without_fixture_identity() {
        let path = Path::new("../../proof/fixtures/bodies/std-line.body.conduit");
        let checked = load(path).unwrap();
        let product = prepare(path).unwrap();
        assert_eq!(checked.hosts().len(), 2);
        assert_eq!(product.context.advertisements().len(), 2);
        assert_eq!(product.context.line_offers().len(), 2);
        assert!(product
            .context
            .advertisements()
            .iter()
            .all(|host| host.host_id.as_str().starts_with("body:std-line/host/")));
    }

    #[test]
    fn structurally_different_body_refuses_the_exact_missing_runtime_class() {
        let error = prepare(Path::new("../../bodies/pete/profiles/pete-r1.body.conduit"))
            .err()
            .expect("the installed product has no Pico runtime handle");
        assert!(error.contains("brainstem"), "{error}");
        assert!(
            error.contains("no installed product runtime handle"),
            "{error}"
        );
        assert!(!error.contains("exactly two"), "{error}");
    }

    #[test]
    fn three_host_body_builds_current_runtime_and_line_truth_without_roles() {
        let product = prepare(Path::new(
            "../../proof/fixtures/bodies/std-three-host.body.conduit",
        ))
        .expect("three installed std Hosts are an ordinary finite product context");
        assert_eq!(product.context.advertisements().len(), 3);
        assert_eq!(product.context.line_offers().len(), 6);
    }
}
