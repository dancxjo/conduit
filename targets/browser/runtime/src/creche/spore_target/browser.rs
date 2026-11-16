//! Reviewed browser target construction, independent of Body preparation.
use super::*;

pub(crate) const BROWSER_RELEASE_SOURCE_ID: &str = "conduit/reviewed-browser-host-release@1";

pub(super) fn browser_target() -> Result<TargetFacts, String> {
    let package = BrowserMakePackage;
    let conduit_host_make::MakeContribution::Anchor(anchor) = package.contribution() else {
        return Err("browser make package is not an anchor".into());
    };
    let descriptor = anchor
        .targets
        .into_iter()
        .next()
        .ok_or_else(|| "browser make package omitted its page target".to_string())?;
    Ok(TargetFacts {
        configuration_name: "creche-browser-page",
        host_name: "creche-browser-page",
        source_identity: BROWSER_RELEASE_SOURCE_ID,
        deployment_destination: Some("browser/local-sandbox"),
        output: SporeOutputKind::BrowserBundle,
        configuration: HostConfiguration {
            schema: 1,
            name: "creche-browser-page".into(),
            target: ConfigurationTarget {
                architecture: descriptor.architecture,
                machine: descriptor.machine,
                board: descriptor.board,
                os: descriptor.os,
                make_descriptor: None,
            },
            bases: conduit_host_browser_make::default_configuration_bases(),
            capabilities: Vec::new(),
            preparation: None,
            execution: None,
            resources: Vec::new(),
            limits: descriptor.maxima,
        },
        packages: MakePackageSet::compose(&[&BrowserMakePackage])
            .map_err(|error| format!("compose browser make package: {error:?}"))?,
    })
}
